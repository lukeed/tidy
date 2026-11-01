use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Condvar, Mutex, PoisonError};

use tidyrs::Whitespace;
use tidyrs::css;
use tidyrs::embed::Verbatim;
use tidyrs::handlebars::{self, Dialect};
use tidyrs::html::{self, Parser};
use tidyrs::xml;

const HELP: &str = "\
tidyrs: a fast markup formatter

Usage
  $ tidyrs [paths...] [options]

  Without paths, reads stdin and prints the result.
  With one file, prints the result unless --check or --write is set.
  Folders are searched for .html, .htm, .vue, .hbs, .handlebars, .css, .scss, and .less files, and for XML
  files: .xml, .svg, .plist, .csproj, pom.xml, and the other files that @prettier/plugin-xml formats.
  Style sheets in <style> and style=\"\" are formatted. Scripts are left as they are written.

Options
  --check                         List files that aren't formatted, and exit 1 if any
  -w, --write                     Rewrite files in place
  --parser <name>                 html, vue, handlebars, tempura, css, scss, less, or xml (default: by extension)
  --width <n>                     Line width (default: 80)
  --tab-width <n>                 Indent width (default: 2)
  --use-tabs                      Indent with tabs
  --whitespace <how>              css (HTML's default), strict (XML's default), preserve (XML), or ignore
  --single-attribute-per-line     Put each attribute on its own line
  --bracket-same-line             Keep the > of a multi-line tag on the last line
  --vue-indent-script-and-style   Indent the code in Vue's <script> and <style>
  --single-quote                  Prefer single quotes in Handlebars and style sheets
  --threads <n>                   How many files to format at once (default: all cores)
  -v, --version                   Print the version
  -h, --help                      Print this help

Examples
  $ tidyrs index.html
  $ tidyrs src --check
  $ tidyrs . --write --width 100
";

fn main() -> ExitCode {
  match run() {
    Ok(code) => code,
    Err(error) => {
      eprintln!("tidyrs: {error}");
      ExitCode::from(2)
    }
  }
}

fn run() -> Result<ExitCode, String> {
  let mut args = pico_args::Arguments::from_env();
  if args.contains(["-h", "--help"]) {
    print!("{HELP}");
    return Ok(ExitCode::SUCCESS);
  }
  if args.contains(["-v", "--version"]) {
    println!("tidyrs {}", env!("CARGO_PKG_VERSION"));
    return Ok(ExitCode::SUCCESS);
  }
  let check = args.contains("--check");
  let write = args.contains(["-w", "--write"]);
  let threads: Option<usize> =
    args.opt_value_from_str("--threads").map_err(|error| error.to_string())?;
  let language = args
    .opt_value_from_fn("--parser", |name: &str| match name {
      "html" => Ok(Language::Markup(Parser::Html)),
      "vue" => Ok(Language::Markup(Parser::Vue)),
      "handlebars" | "glimmer" => Ok(Language::Handlebars(Dialect::Handlebars)),
      "tempura" => Ok(Language::Handlebars(Dialect::Tempura)),
      "css" => Ok(Language::Css(css::Dialect::Css)),
      "scss" => Ok(Language::Css(css::Dialect::Scss)),
      "less" => Ok(Language::Css(css::Dialect::Less)),
      "xml" => Ok(Language::Xml),
      other => Err(format!("unknown parser `{other}`")),
    })
    .map_err(|error| error.to_string())?;
  let settings = settings(&mut args)?;
  let mut paths = Vec::new();
  for arg in args.finish() {
    let text = arg.to_string_lossy();
    if text.starts_with('-') && text != "-" {
      return Err(format!("unknown flag `{text}`, see --help"));
    }
    paths.push(PathBuf::from(arg));
  }

  if paths.is_empty() || paths == [Path::new("-")] {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).map_err(|error| error.to_string())?;
    let language = language.unwrap_or(Language::Markup(Parser::Html));
    let mut formatters = Formatters::default();
    let output = formatters.format(&source, &settings, language, None)?;
    if check {
      return Ok(if output == source { ExitCode::SUCCESS } else { ExitCode::FAILURE });
    }
    std::io::stdout().write_all(output.as_bytes()).map_err(|error| error.to_string())?;
    return Ok(ExitCode::SUCCESS);
  }

  if !check && !write && paths.len() == 1 && paths[0].is_file() {
    let path = &paths[0];
    let source =
      std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut formatters = Formatters::default();
    let language = language.unwrap_or_else(|| language_of(path));
    let output = formatters.format(&source, &settings, language, Some(path))?;
    std::io::stdout().write_all(output.as_bytes()).map_err(|error| error.to_string())?;
    return Ok(ExitCode::SUCCESS);
  }
  if !check && !write {
    return Err("pass --check or --write to format more than one file".into());
  }

  let queue = Queue::new(&paths);
  let cores = std::thread::available_parallelism().map_or(1, |count| count.get());
  let threads = threads.unwrap_or(cores).max(1);
  let mut outcomes: Vec<(PathBuf, Outcome)> = std::thread::scope(|scope| {
    let workers: Vec<_> =
      (0..threads).map(|_| scope.spawn(|| work(&queue, &settings, language, write))).collect();
    workers.into_iter().flat_map(|worker| worker.join().unwrap_or_default()).collect()
  });
  outcomes.sort_by(|(left, _), (right, _)| left.cmp(right));

  let (mut listed, mut errors) = (String::new(), String::new());
  for (path, outcome) in &outcomes {
    let path = path.display();
    match outcome {
      Outcome::Changed => listed.push_str(&format!("{path}\n")),
      Outcome::Failed(error) => errors.push_str(&format!("tidyrs: {path}: {error}\n")),
    }
  }
  std::io::stdout().write_all(listed.as_bytes()).map_err(|error| error.to_string())?;
  std::io::stderr().write_all(errors.as_bytes()).map_err(|error| error.to_string())?;
  if !errors.is_empty() {
    return Ok(ExitCode::from(2));
  }
  Ok(if check && !outcomes.is_empty() { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

/// What a file is written in.
#[derive(Clone, Copy)]
enum Language {
  Markup(Parser),
  Handlebars(Dialect),
  Css(css::Dialect),
  Xml,
}

/// The language of the file at `path`, by its extension.
fn language_of(path: &Path) -> Language {
  let name = path.to_string_lossy();
  if let Some(dialect) = css::Dialect::of_path(&name) {
    return Language::Css(dialect);
  }
  if xml::is_xml_path(&name) {
    return Language::Xml;
  }
  match path.extension().and_then(|it| it.to_str()) {
    Some("vue") => Language::Markup(Parser::Vue),
    Some("hbs" | "handlebars") => Language::Handlebars(Dialect::Auto),
    _ => Language::Markup(Parser::Html),
  }
}

/// The settings of each formatter.
#[derive(Clone)]
struct Settings {
  html: html::Settings,
  handlebars: handlebars::Settings,
  css: css::Settings,
  xml: xml::Settings,
}

fn settings(args: &mut pico_args::Arguments) -> Result<Settings, String> {
  let error = |error: pico_args::Error| error.to_string();
  let mut settings = html::Settings::default();
  if let Some(width) = args.opt_value_from_str("--width").map_err(error)? {
    settings.width = width;
  }
  if let Some(width) = args.opt_value_from_str("--tab-width").map_err(error)? {
    settings.tab_width = width;
  }
  settings.tabs = args.contains("--use-tabs");
  let whitespace = args
    .opt_value_from_fn("--whitespace", |name: &str| match name {
      "css" | "strict" | "preserve" | "ignore" => Ok(name.to_owned()),
      other => Err(format!("unknown whitespace `{other}`")),
    })
    .map_err(error)?;
  settings.whitespace = match whitespace.as_deref() {
    Some("strict") => Whitespace::Strict,
    Some("ignore") => Whitespace::Ignore,
    _ => Whitespace::Auto,
  };
  settings.single_attribute_per_line = args.contains("--single-attribute-per-line");
  settings.bracket_same_line = args.contains("--bracket-same-line");
  settings.vue_indent_script_and_style = args.contains("--vue-indent-script-and-style");
  let handlebars = handlebars::Settings {
    width: settings.width,
    tab_width: settings.tab_width,
    tabs: settings.tabs,
    whitespace: settings.whitespace,
    single_quote: args.contains("--single-quote"),
    ..handlebars::Settings::default()
  };
  let css = css::Settings {
    width: settings.width,
    tab_width: settings.tab_width,
    tabs: settings.tabs,
    single_quote: handlebars.single_quote,
    ..css::Settings::default()
  };
  let xml = xml::Settings {
    width: settings.width,
    tab_width: settings.tab_width,
    tabs: settings.tabs,
    whitespace: match whitespace.as_deref() {
      Some("preserve") => tidyrs::Whitespace::Auto,
      Some("ignore") => tidyrs::Whitespace::Ignore,
      _ => tidyrs::Whitespace::Strict,
    },
    bracket_same_line: settings.bracket_same_line,
    single_attribute_per_line: settings.single_attribute_per_line,
    ..xml::Settings::default()
  };
  Ok(Settings { html: settings, handlebars, css, xml })
}

/// A formatter for each language, each reusing its buffers.
#[derive(Default)]
struct Formatters {
  html: html::Formatter,
  handlebars: handlebars::Formatter,
  css: css::Formatter,
  xml: xml::Formatter,
}

impl Formatters {
  fn format(
    &mut self,
    source: &str,
    settings: &Settings,
    language: Language,
    path: Option<&Path>,
  ) -> Result<&str, String> {
    let error = |error: &dyn std::fmt::Display| error.to_string();
    match language {
      Language::Markup(parser) => {
        let filepath = path.map(|path| path.to_string_lossy().into_owned());
        let settings = html::Settings { parser, filepath, ..settings.html.clone() };
        self.html.format(source, &settings, &mut Verbatim).map_err(|it| error(&it))
      }
      Language::Handlebars(dialect) => {
        let filepath = path.map(|path| path.to_string_lossy().into_owned());
        let settings = handlebars::Settings { filepath, dialect, ..settings.handlebars.clone() };
        self.handlebars.format(source, &settings, &mut Verbatim).map_err(|it| error(&it))
      }
      Language::Css(dialect) => {
        let settings = css::Settings { dialect, ..settings.css.clone() };
        self.css.format(source, &settings, &mut Verbatim).map_err(|it| error(&it))
      }
      Language::Xml => {
        self.xml.format(source, &settings.xml, &mut Verbatim).map_err(|it| error(&it))
      }
    }
  }
}

/// What happened to a file that isn't formatted already.
enum Outcome {
  Changed,
  Failed(String),
}

fn work(
  queue: &Queue,
  settings: &Settings,
  language: Option<Language>,
  write: bool,
) -> Vec<(PathBuf, Outcome)> {
  let mut outcomes = Vec::new();
  let mut formatters = Formatters::default();
  let mut source = String::new();
  while let Some(task) = queue.next() {
    let path = match task {
      Task::Folder(dir) => {
        queue.done(read(&dir));
        continue;
      }
      Task::File(path) => path,
    };
    source.clear();
    let read = std::fs::File::open(&path).and_then(|mut file| file.read_to_string(&mut source));
    let outcome = match read {
      Err(error) => Outcome::Failed(error.to_string()),
      Ok(_) => match formatters.format(
        &source,
        settings,
        language.unwrap_or_else(|| language_of(&path)),
        Some(&path),
      ) {
        Err(error) => Outcome::Failed(error),
        Ok(output) if output == source => {
          queue.done(Vec::new());
          continue;
        }
        Ok(output) => match write.then(|| std::fs::write(&path, output)) {
          Some(Err(error)) => Outcome::Failed(error.to_string()),
          _ => Outcome::Changed,
        },
      },
    };
    outcomes.push((path, outcome));
    queue.done(Vec::new());
  }
  outcomes
}

enum Task {
  Folder(PathBuf),
  File(PathBuf),
}

/// The folders to read and the files to format, which the workers share. Reading a folder adds what it holds, so
/// formatting starts before the walk ends.
struct Queue {
  /// The tasks, and how many workers are busy with one.
  state: Mutex<(Vec<Task>, usize)>,
  ready: Condvar,
}

impl Queue {
  fn new(paths: &[PathBuf]) -> Queue {
    let tasks = paths
      .iter()
      .map(|path| match path.is_dir() {
        true => Task::Folder(path.clone()),
        false => Task::File(path.clone()),
      })
      .collect();
    Queue { state: Mutex::new((tasks, 0)), ready: Condvar::new() }
  }

  /// The next task, or `None` once there's none left and no busy worker can add more.
  fn next(&self) -> Option<Task> {
    let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
      if let Some(task) = state.0.pop() {
        state.1 += 1;
        return Some(task);
      }
      if state.1 == 0 {
        return None;
      }
      state = self.ready.wait(state).unwrap_or_else(PoisonError::into_inner);
    }
  }

  /// Ends a task, adding the tasks that it found.
  fn done(&self, found: Vec<Task>) {
    let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
    let wakes = !found.is_empty() || state.1 == 1;
    state.0.extend(found);
    state.1 -= 1;
    drop(state);
    if wakes {
      self.ready.notify_all();
    }
  }
}

/// The markup, style sheet and XML files in `dir`, and the folders to read next, skipping hidden folders and
/// `node_modules`.
fn read(dir: &Path) -> Vec<Task> {
  let mut found = Vec::new();
  let Ok(entries) = std::fs::read_dir(dir) else { return found };
  for entry in entries.flatten() {
    let name = entry.file_name();
    let Ok(kind) = entry.file_type() else { continue };
    let path = entry.path();
    if kind.is_dir() {
      let name = name.to_string_lossy();
      if !name.starts_with('.') && name != "node_modules" {
        found.push(Task::Folder(path));
      }
    } else if matches!(
      path.extension().and_then(|it| it.to_str()),
      Some("html" | "htm" | "vue" | "hbs" | "handlebars" | "css" | "scss" | "less")
    ) || xml::is_xml_path(&path.to_string_lossy())
    {
      found.push(Task::File(path));
    }
  }
  found
}
