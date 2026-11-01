use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use mdtidy::{Options, Preset};

const HELP: &str = "\
mdtidy: a fast Markdown formatter

Usage
  $ mdtidy [paths...] [options]

  Without paths, reads stdin and prints the result.
  With one file, prints the result unless --check or --write is set.
  Folders are searched for .md and .markdown files.

Options
  --check              List files that aren't formatted, and exit 1 if any
  -w, --write          Rewrite files in place
  --preset <name>      prettier (default) or preserve
  --width <n>          Line width (default: 80)
  --wrap <how>         preserve, always, or never
  --bullet <marker>    keep, dash, asterisk, or plus
  --emphasis <mark>    keep, underscore, or asterisk
  --strong <mark>      keep, asterisk, or underscore
  --numbering <how>    keep or auto
  --rule <marker>      keep, dash, asterisk, or underscore
  --fence <marker>     keep, backtick, or tilde
  --heading <style>    keep or atx
  --ignore <name>      The ignore directive (default: prettier-ignore,
                       or mdtidy-ignore with --preset preserve)
  -v, --version        Print the version
  -h, --help           Print this help

Examples
  $ mdtidy readme.md
  $ mdtidy docs --check
  $ mdtidy . --write --preset preserve --wrap always
";

fn main() -> ExitCode {
  match run() {
    Ok(code) => code,
    Err(error) => {
      eprintln!("mdtidy: {error}");
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
    println!("mdtidy {}", env!("CARGO_PKG_VERSION"));
    return Ok(ExitCode::SUCCESS);
  }
  let check = args.contains("--check");
  let write = args.contains(["-w", "--write"]);
  let options = options(&mut args)?;
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
    let output = mdtidy::format(&source, &options);
    if check {
      return Ok(if output == source { ExitCode::SUCCESS } else { ExitCode::FAILURE });
    }
    std::io::stdout().write_all(output.as_bytes()).map_err(|error| error.to_string())?;
    return Ok(ExitCode::SUCCESS);
  }

  if !check && !write && paths.len() == 1 && paths[0].is_file() {
    let source = std::fs::read_to_string(&paths[0])
      .map_err(|error| format!("{}: {error}", paths[0].display()))?;
    let output = mdtidy::format(&source, &options);
    std::io::stdout().write_all(output.as_bytes()).map_err(|error| error.to_string())?;
    return Ok(ExitCode::SUCCESS);
  }
  if !check && !write {
    return Err("pass --check or --write to format more than one file".into());
  }

  let mut files = Vec::new();
  let mut errors = Vec::new();
  for path in &paths {
    if path.is_dir() {
      walk(path, &mut files, &mut errors);
    } else {
      files.push(path.clone());
    }
  }
  files.sort();

  let next = AtomicUsize::new(0);
  let threads = std::thread::available_parallelism().map_or(1, |count| count.get());
  let mut outcomes: Vec<(usize, Outcome)> = std::thread::scope(|scope| {
    let workers: Vec<_> = (0..threads.min(files.len().max(1)))
      .map(|_| scope.spawn(|| work(&files, &next, &options, write)))
      .collect();
    workers.into_iter().flat_map(|worker| worker.join().unwrap_or_default()).collect()
  });
  outcomes.sort_by_key(|(index, _)| *index);

  let mut changed = 0;
  errors.sort();
  let mut failed = !errors.is_empty();
  for (path, error) in &errors {
    eprintln!("mdtidy: {}: {error}", path.display());
  }
  for (index, outcome) in outcomes {
    let path = files[index].display();
    match outcome {
      Outcome::Same => {}
      Outcome::Changed => {
        changed += 1;
        println!("{path}");
      }
      Outcome::Failed(error) => {
        failed = true;
        eprintln!("mdtidy: {path}: {error}");
      }
    }
  }
  if failed {
    return Ok(ExitCode::from(2));
  }
  Ok(if check && changed > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

fn options(args: &mut pico_args::Arguments) -> Result<Options, String> {
  let preset: Option<Preset> = choice(args, "--preset")?;
  let mut options = Options::from(preset.unwrap_or_default());
  if let Some(width) = args.opt_value_from_str("--width").map_err(|error| error.to_string())? {
    options.width = width;
  }
  if let Some(wrap) = choice(args, "--wrap")? {
    options.wrap = wrap;
  }
  if let Some(bullet) = choice(args, "--bullet")? {
    options.bullet = bullet;
  }
  if let Some(emphasis) = choice(args, "--emphasis")? {
    options.emphasis = emphasis;
  }
  if let Some(strong) = choice(args, "--strong")? {
    options.strong = strong;
  }
  if let Some(numbering) = choice(args, "--numbering")? {
    options.numbering = numbering;
  }
  if let Some(rule) = choice(args, "--rule")? {
    options.rule = rule;
  }
  if let Some(fence) = choice(args, "--fence")? {
    options.fence = fence;
  }
  if let Some(heading) = choice(args, "--heading")? {
    options.heading = heading;
  }
  if let Some(ignore) = args.opt_value_from_str("--ignore").map_err(|error| error.to_string())? {
    options.ignore = ignore;
  }
  Ok(options)
}

fn choice<T: FromStr<Err = String>>(
  args: &mut pico_args::Arguments,
  name: &'static str,
) -> Result<Option<T>, String> {
  args.opt_value_from_fn(name, T::from_str).map_err(|error| error.to_string())
}

enum Outcome {
  Same,
  Changed,
  Failed(String),
}

fn work(
  files: &[PathBuf],
  next: &AtomicUsize,
  options: &Options,
  write: bool,
) -> Vec<(usize, Outcome)> {
  let mut outcomes = Vec::new();
  loop {
    let index = next.fetch_add(1, Ordering::Relaxed);
    let Some(path) = files.get(index) else { return outcomes };
    let outcome = match std::fs::read_to_string(path) {
      Err(error) => Outcome::Failed(error.to_string()),
      Ok(source) => match std::panic::catch_unwind(|| mdtidy::format(&source, options)) {
        Err(panic) => Outcome::Failed(format!("formatting panicked: {}", message(&*panic))),
        Ok(output) if output == source => Outcome::Same,
        Ok(output) => match write.then(|| std::fs::write(path, output)) {
          Some(Err(error)) => Outcome::Failed(error.to_string()),
          _ => Outcome::Changed,
        },
      },
    };
    outcomes.push((index, outcome));
  }
}

/// The text of a panic's payload.
fn message(panic: &(dyn std::any::Any + Send)) -> &str {
  match (panic.downcast_ref::<&str>(), panic.downcast_ref::<String>()) {
    (Some(text), _) => text,
    (_, Some(text)) => text,
    _ => "no message",
  }
}

/// Collect Markdown files under `dir`, skipping hidden folders and `node_modules`. Folders and
/// entries that can't be read are collected as errors, and the walk goes on.
fn walk(dir: &Path, files: &mut Vec<PathBuf>, errors: &mut Vec<(PathBuf, String)>) {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(error) => return errors.push((dir.to_path_buf(), error.to_string())),
  };
  for entry in entries {
    let entry = match entry {
      Ok(entry) => entry,
      Err(error) => {
        errors.push((dir.to_path_buf(), error.to_string()));
        continue;
      }
    };
    let name = entry.file_name();
    let kind = match entry.file_type() {
      Ok(kind) => kind,
      Err(error) => {
        errors.push((entry.path(), error.to_string()));
        continue;
      }
    };
    if kind.is_dir() {
      let name = name.to_string_lossy();
      if !name.starts_with('.') && name != "node_modules" {
        walk(&entry.path(), files, errors);
      }
    } else if kind.is_file()
      && Path::new(&name).extension().is_some_and(|ext| ext == "md" || ext == "markdown")
    {
      files.push(entry.path());
    }
  }
}
