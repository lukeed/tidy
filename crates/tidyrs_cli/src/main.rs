use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Condvar, Mutex, PoisonError};

use tidyrs::config::{Config, Resolver};
use tidyrs::embed::Verbatim;
use tidyrs::{Error, Formatter, Language, Settings, Shared, Whitespace, css, doc, handlebars};

const HELP: &str = "\
tidyrs: a fast markup formatter

Usage
  $ tidyrs [paths...] [options]

  Without paths, reads stdin and prints the result.
  With one file, prints the result unless --check or --write is set.
  Folders are searched for .html, .htm, .vue, .hbs, .handlebars, .css, .scss, and .less files, and for XML
  files: .xml, .svg, .plist, .csproj, pom.xml, and the other files that @prettier/plugin-xml formats.
  Style sheets in <style> and style=\"\" are formatted. Scripts are left as they are written.

  Each file takes its settings from the nearest tidy.json, tidy.jsonc or tidy.toml, and from .editorconfig.
  Options given here override them.

Options
  --check                         List files that aren't formatted, and exit 1 if any
  -w, --write                     Rewrite files in place
  --parser <name>                 html, vue, handlebars, tempura, css, scss, less, or xml (default: by file name)
  --stdin-filepath <path>         The path of the text on stdin, for its language and its config files
  --config <path>                 Use this config for every file instead of the nearest one
  --no-config                     Read no config files and no .editorconfig
  --width <n>                     Line width (default: 80)
  --indent <n>                    Indent width (default: 2)
  --tabs                          Indent with tabs
  --eol <eol>                     lf (default), crlf, or cr
  --whitespace <how>              strict, auto, or ignore (default: auto, and strict for XML)
  --quote <quote>                 double (default) or single, in Handlebars and style sheets
  --single-attribute-per-line     Put each attribute on its own line
  --bracket-same-line             Keep the > of a multi-line tag on the last line
  --ignore <name>                 The ignore directive (default: prettier-ignore)
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
  let error = |error: pico_args::Error| error.to_string();
  let check = args.contains("--check");
  let write = args.contains(["-w", "--write"]);
  let threads: Option<usize> = args.opt_value_from_str("--threads").map_err(error)?;
  let mut flags = Config::default();
  let parser: Option<String> = args.opt_value_from_str("--parser").map_err(error)?;
  let language = match parser.as_deref() {
    None => None,
    Some(name) => {
      let (language, dialect) = match name {
        "html" => (Language::Html, None),
        "vue" => (Language::Vue, None),
        "handlebars" => (Language::Handlebars, None),
        "tempura" => (Language::Handlebars, Some(handlebars::Dialect::Tempura)),
        "css" => (Language::Css(css::Dialect::Css), None),
        "scss" => (Language::Css(css::Dialect::Scss), None),
        "less" => (Language::Css(css::Dialect::Less), None),
        "xml" => (Language::Xml, None),
        other => return Err(format!("unknown parser `{other}`")),
      };
      flags.handlebars.dialect = dialect;
      Some(language)
    }
  };
  let stdin: Option<PathBuf> = args.opt_value_from_str("--stdin-filepath").map_err(error)?;
  let fixed: Option<PathBuf> = args.opt_value_from_str("--config").map_err(error)?;
  let no_config = args.contains("--no-config");
  flags.shared = shared(&mut args)?;
  let mut paths = Vec::new();
  for arg in args.finish() {
    let text = arg.to_string_lossy();
    if text.starts_with('-') && text != "-" {
      return Err(format!("unknown flag `{text}`, see --help"));
    }
    paths.push(PathBuf::from(arg));
  }
  let resolver = match (no_config, fixed) {
    (true, _) => None,
    (false, Some(path)) => Some(Resolver::fixed(Config::read(&path)?)),
    (false, None) => Some(Resolver::default()),
  };
  let sources = Sources { resolver, flags };

  if paths.is_empty() || paths == [Path::new("-")] {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).map_err(|error| error.to_string())?;
    let language = language
      .or_else(|| stdin.as_deref().and_then(|path| Language::of_path(&path.to_string_lossy())))
      .unwrap_or(Language::Html);
    let settings = sources.settings(stdin.as_deref())?;
    let path = stdin.as_deref().map(|path| path.to_string_lossy());
    let mut formatter = Formatter::default();
    let output = formatter
      .format(&source, language, &settings, path.as_deref(), &mut Verbatim)
      .map_err(|error| describe(&error, &source, path.as_deref().unwrap_or("stdin")))?;
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
    let mut formatter = Formatter::default();
    let output = sources.format(&mut formatter, &source, language, path)?;
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
      (0..threads).map(|_| scope.spawn(|| work(&queue, &sources, language, write))).collect();
    workers.into_iter().flat_map(|worker| worker.join().unwrap_or_default()).collect()
  });
  outcomes.sort_by(|(left, _), (right, _)| left.cmp(right));

  let (mut listed, mut errors) = (String::new(), String::new());
  for (path, outcome) in &outcomes {
    match outcome {
      Outcome::Changed => listed.push_str(&format!("{}\n", path.display())),
      Outcome::Failed(error) => errors.push_str(&format!("tidyrs: {error}\n")),
    }
  }
  std::io::stdout().write_all(listed.as_bytes()).map_err(|error| error.to_string())?;
  std::io::stderr().write_all(errors.as_bytes()).map_err(|error| error.to_string())?;
  if !errors.is_empty() {
    return Ok(ExitCode::from(2));
  }
  Ok(if check && !outcomes.is_empty() { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

/// The shared choices that the flags make.
fn shared(args: &mut pico_args::Arguments) -> Result<Shared, String> {
  let error = |error: pico_args::Error| error.to_string();
  let mut choice = |name: &'static str, options: &[&str]| -> Result<Option<String>, String> {
    let value: Option<String> = args.opt_value_from_str(name).map_err(error)?;
    match value {
      Some(value) if !options.contains(&value.as_str()) => {
        Err(format!("unknown {name} `{value}`, expected one of: {}", options.join(", ")))
      }
      value => Ok(value),
    }
  };
  let eol = choice("--eol", &["lf", "crlf", "cr"])?;
  let whitespace = choice("--whitespace", &["strict", "auto", "ignore"])?;
  let quote = choice("--quote", &["double", "single"])?;
  let flag =
    |args: &mut pico_args::Arguments, name: &'static str| args.contains(name).then_some(true);
  Ok(Shared {
    width: args.opt_value_from_str("--width").map_err(error)?,
    indent: args.opt_value_from_str("--indent").map_err(error)?,
    tabs: flag(args, "--tabs"),
    eol: eol.map(|eol| match eol.as_str() {
      "crlf" => doc::Eol::Crlf,
      "cr" => doc::Eol::Cr,
      _ => doc::Eol::Lf,
    }),
    whitespace: whitespace.map(|how| match how.as_str() {
      "strict" => Whitespace::Strict,
      "ignore" => Whitespace::Ignore,
      _ => Whitespace::Auto,
    }),
    single_quote: quote.map(|quote| quote == "single"),
    bracket_same_line: flag(args, "--bracket-same-line"),
    single_attribute_per_line: flag(args, "--single-attribute-per-line"),
    ignore: args.opt_value_from_str("--ignore").map_err(error)?,
    format_embedded: None,
  })
}

/// Where the settings of each file come from: Prettier's, then the config files around it, then the flags.
struct Sources {
  /// `None` with `--no-config`.
  resolver: Option<Resolver>,
  flags: Config,
}

impl Sources {
  fn settings(&self, path: Option<&Path>) -> Result<Settings, String> {
    let mut settings = match (&self.resolver, path) {
      (Some(resolver), Some(path)) => resolver.settings(path)?,
      _ => Settings::default(),
    };
    self.flags.apply(&mut settings);
    Ok(settings)
  }

  /// Formats the file at `path`, in `language` or the language of its name.
  fn format<'f>(
    &self,
    formatter: &'f mut Formatter,
    source: &str,
    language: Option<Language>,
    path: &Path,
  ) -> Result<&'f str, String> {
    let name = path.to_string_lossy();
    let language = language
      .or_else(|| Language::of_path(&name))
      .ok_or_else(|| format!("{name}: can't tell the language of this file, pass --parser"))?;
    let settings = self.settings(Some(path)).map_err(|error| format!("{name}: {error}"))?;
    formatter
      .format(source, language, &settings, Some(&name), &mut Verbatim)
      .map_err(|error| describe(&error, source, &name))
  }
}

/// An error, with the line and column of a syntax error.
fn describe(error: &Error, source: &str, name: &str) -> String {
  match error {
    Error::Syntax { message, at } => {
      let before = &source[..(*at).min(source.len())];
      let line = before.matches('\n').count() + 1;
      let column = before.rsplit('\n').next().unwrap_or_default().chars().count() + 1;
      format!("{name}:{line}:{column}: {message}")
    }
    Error::TooDeep => format!("{name}: {error}"),
  }
}

/// What happened to a file that isn't formatted already.
enum Outcome {
  Changed,
  Failed(String),
}

fn work(
  queue: &Queue,
  sources: &Sources,
  language: Option<Language>,
  write: bool,
) -> Vec<(PathBuf, Outcome)> {
  let mut outcomes = Vec::new();
  let mut formatter = Formatter::default();
  let mut source = String::new();
  while let Some(task) = queue.next() {
    let path = match task {
      Task::Folder(dir) => {
        queue.done(read(&dir, sources.resolver.as_ref()));
        continue;
      }
      Task::File(path) => path,
    };
    source.clear();
    let read = std::fs::File::open(&path).and_then(|mut file| file.read_to_string(&mut source));
    let outcome = match read {
      Err(error) => Outcome::Failed(format!("{}: {error}", path.display())),
      Ok(_) => match sources.format(&mut formatter, &source, language, &path) {
        Err(error) => Outcome::Failed(error),
        Ok(output) if output == source => {
          queue.done(Vec::new());
          continue;
        }
        Ok(output) => match write.then(|| std::fs::write(&path, output)) {
          Some(Err(error)) => Outcome::Failed(format!("{}: {error}", path.display())),
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

/// The files in `dir` that tidyrs formats, and the folders to read next, skipping hidden folders and
/// `node_modules`. Tells `resolver` which config files are there.
fn read(dir: &Path, resolver: Option<&Resolver>) -> Vec<Task> {
  let mut found = Vec::new();
  let mut configs = Vec::new();
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
    } else if Language::of_path(&path.to_string_lossy()).is_some() {
      found.push(Task::File(path));
    } else if name == ".editorconfig" || tidyrs::config::NAMES.iter().any(|it| name == *it) {
      configs.push(name);
    }
  }
  if let Some(resolver) = resolver {
    resolver.listing(dir, configs.iter().map(|name| name.as_os_str()));
  }
  found
}
