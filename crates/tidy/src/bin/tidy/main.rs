use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use tidy::embed::Verbatim;
use tidy::{Language, Options, json};

use files::Mode;

mod files;
mod flags;
mod git;
mod help;
mod init;
mod migrate;
mod sources;

const STDIN_NEEDS_COMMAND: &str =
  "--stdin needs a command: `tidy up --stdin` prints the result, `tidy check --stdin` checks it";

fn main() -> ExitCode {
  // A main thread has 1 MiB on Windows and 8 MiB elsewhere, so run on a thread with the walk threads' stack.
  let thread = std::thread::Builder::new().stack_size(tidy::walk::STACK).spawn(run);
  let result = match thread.map(std::thread::JoinHandle::join) {
    Ok(Ok(result)) => result,
    Ok(Err(panic)) => std::panic::resume_unwind(panic),
    Err(error) => Err(format!("can't start: {error}")),
  };
  match result {
    Ok(code) => code,
    Err(error) => {
      eprintln!("tidy: {error}");
      ExitCode::from(2)
    }
  }
}

fn run() -> Result<ExitCode, String> {
  let mut raw: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
  let asks_help =
    |args: &[std::ffi::OsString]| args.iter().any(|arg| arg == "-h" || arg == "--help");
  let first = raw.first().map(|arg| arg.to_string_lossy().into_owned());
  let mode = match first.as_deref() {
    None | Some("-h" | "--help") => {
      return print(help::global().as_bytes()).map(|()| ExitCode::SUCCESS);
    }
    Some("-v" | "--version") => {
      println!("tidy {}", env!("CARGO_PKG_VERSION"));
      return Ok(ExitCode::SUCCESS);
    }
    Some("up") => Mode::Up,
    Some("check") => Mode::Check,
    Some(command @ ("init" | "schema")) => {
      let rest = &raw[1..];
      if asks_help(rest) {
        let text = if command == "init" { help::init() } else { help::schema() };
        return print(text.as_bytes()).map(|()| ExitCode::SUCCESS);
      }
      if command == "init" {
        return init::run(init::Args::of(rest)?).map(|()| ExitCode::SUCCESS);
      }
      if let Some(arg) = rest.first() {
        return Err(format!(
          "`tidy schema` takes no arguments, but got `{}`",
          arg.to_string_lossy()
        ));
      }
      return schema();
    }
    Some(flag) if flag.starts_with('-') => {
      let stdin = raw
        .iter()
        .any(|arg| arg.to_str().is_some_and(|it| it == "--stdin" || it.starts_with("--stdin=")));
      return Err(match stdin {
        true => STDIN_NEEDS_COMMAND.into(),
        false => "a command is missing: `tidy up` or `tidy check`, see `tidy --help`".into(),
      });
    }
    Some(other) => return Err(format!("unknown command `{other}`, see `tidy --help`")),
  };
  raw.remove(0);
  if asks_help(&raw) {
    let text = if mode == Mode::Up { help::up() } else { help::check() };
    return print(text.as_bytes()).map(|()| ExitCode::SUCCESS);
  }
  files::run(mode, raw)
}

/// A path as tidy prints it: with `/` between its parts on every system, as Prettier and git print it.
pub(crate) fn shown(path: &Path) -> String {
  let text = path.to_string_lossy();
  match std::path::MAIN_SEPARATOR {
    '/' => text.into_owned(),
    separator => text.replace(separator, "/"),
  }
}

/// Writes to stdout. A closed pipe, as in `tidy schema | head`, is not an error worth reporting.
pub(crate) fn print(bytes: &[u8]) -> Result<(), String> {
  match std::io::stdout().write_all(bytes) {
    Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
    other => other.map_err(|error| error.to_string()),
  }
}

/// `tidy schema`, printed through tidy, so the file it writes is one tidy already accepts.
fn schema() -> Result<ExitCode, String> {
  let raw = tidy::schema::schema();
  let json = Language::Json(json::Dialect::Json);
  let text = tidy::format(&raw, json, &Options::default(), None, &mut Verbatim).unwrap_or(raw);
  print(text.as_bytes())?;
  Ok(ExitCode::SUCCESS)
}
