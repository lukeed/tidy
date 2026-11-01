use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use mdtidy::{Options, Preset};
use tidy_walk::Output;

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

  let cores = std::thread::available_parallelism().map_or(1, |count| count.get());
  let walk = tidy_walk::Options {
    wanted: &markdown,
    io: cores,
    workers: 0,
    ..tidy_walk::Options::default()
  };
  let report =
    tidy_walk::run(&paths, &walk, |_, _| {}, || (), |(), file| work(file, &options, write));

  let mut changed = 0;
  let mut failed = !report.errors.is_empty();
  for (path, error) in &report.errors {
    eprintln!("mdtidy: {}: {error}", path.display());
  }
  for (path, outcome) in &report.results {
    match outcome {
      Outcome::Changed => {
        changed += 1;
        println!("{}", path.display());
      }
      Outcome::Failed(error) => {
        failed = true;
        eprintln!("mdtidy: {}: {error}", path.display());
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
  Changed,
  Failed(String),
}

/// Whether a file found in a folder is Markdown.
fn markdown(_: &Path, name: &OsStr) -> bool {
  Path::new(name).extension().is_some_and(|ext| ext == "md" || ext == "markdown")
}

/// Format one file: nothing when it is already formatted, or the new text to write under `--write`.
fn work(file: &tidy_walk::File, options: &Options, write: bool) -> Option<Output<Outcome>> {
  let Ok(source) = std::str::from_utf8(&file.bytes) else {
    let value = Outcome::Failed("stream did not contain valid UTF-8".into());
    return Some(Output { value, write: None });
  };
  let output = mdtidy::format(source, options);
  (output != source)
    .then(|| Output { value: Outcome::Changed, write: write.then(|| output.into()) })
}
