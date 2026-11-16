//! `tidy up` and `tidy check`: the files they take, from paths, git or stdin, and what they do with them.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tidy::config::{Config, Resolver};
use tidy::walk::Output;
use tidy::{Formatter, Language};

use crate::sources::Sources;
use crate::{flags, git, shown};

/// What to do with the files: rewrite them, or just name the ones that need it.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Mode {
  Up,
  Check,
}

/// Which files to take from git, instead of walking the paths given.
enum Scope {
  All,
  /// Staged in the index. `up` stages what it rewrote.
  Staged,
  /// Different from a branch or commit; `None` is the default branch.
  Changed(Option<String>),
}

/// What the command line asked for.
struct Request {
  mode: Mode,
  scope: Scope,
  /// `--stdin`, with its path if it has one.
  stdin: Option<Option<PathBuf>>,
  paths: Vec<PathBuf>,
  threads: Option<usize>,
  gitignore: bool,
  sources: Sources,
}

pub(crate) fn run(mode: Mode, raw: Vec<OsString>) -> Result<ExitCode, String> {
  let request = parse(mode, raw)?;
  // `up` prints what it formats from stdin, which has no file to rewrite; `check` only exits with it.
  if let Some(path) = &request.stdin {
    return request.sources.stdin(path.as_deref(), mode == Mode::Check);
  }
  match pick(&request.scope, &request.paths)? {
    // Nothing to do is not a failure: a commit that touches no formattable file is fine.
    Some(listed) if listed.is_empty() => Ok(ExitCode::SUCCESS),
    Some(listed) => walk(&request, listed, false),
    None if request.paths.is_empty() => walk(&request, vec![PathBuf::from(".")], true),
    None => walk(&request, request.paths.clone(), false),
  }
}

fn parse(mode: Mode, raw: Vec<OsString>) -> Result<Request, String> {
  // `--changed` and `--stdin` take their values with `=`, because `--changed main` cannot be
  // told apart from `--changed` followed by the path `main`.
  let (mut staged, mut changed, mut stdin, mut rest) =
    (false, None, None, Vec::with_capacity(raw.len()));
  for arg in raw {
    match arg.to_str() {
      Some("--staged") => staged = true,
      Some("--changed") => changed = Some(None),
      Some(text) if text.starts_with("--changed=") => {
        changed = Some(Some(text["--changed=".len()..].to_owned()));
      }
      Some("--stdin") => stdin = Some(None),
      Some("--stdin=") => return Err("--stdin= needs a path".into()),
      Some(text) if text.starts_with("--stdin=") => {
        stdin = Some(Some(PathBuf::from(&text["--stdin=".len()..])));
      }
      _ => rest.push(arg),
    }
  }
  let scope = match (staged, changed) {
    (true, Some(_)) => return Err("--staged and --changed can't be used together".into()),
    (true, None) => Scope::Staged,
    (false, Some(base)) => Scope::Changed(base),
    (false, None) => Scope::All,
  };

  let mut args = pico_args::Arguments::from_vec(rest);
  let error = |error: pico_args::Error| error.to_string();
  let threads: Option<usize> = args.opt_value_from_str("--threads").map_err(error)?;
  let gitignore = !args.contains("--no-gitignore");
  let sources = sources(&mut args, mode)?;
  let mut paths = Vec::new();
  for arg in args.finish() {
    let text = arg.to_string_lossy();
    if text.starts_with('-') {
      let command = if mode == Mode::Up { "up" } else { "check" };
      return Err(format!("unknown flag `{text}`, see `tidy {command} --help`"));
    }
    paths.push(PathBuf::from(arg));
  }
  if stdin.is_some() && !paths.is_empty() {
    return Err("--stdin takes no paths, name the text with --stdin=<path>".into());
  }
  if stdin.is_some() && !matches!(scope, Scope::All) {
    return Err("--stdin can't be used with --staged or --changed".into());
  }
  Ok(Request { mode, scope, stdin, paths, threads, gitignore, sources })
}

/// The options and languages the flags choose, on top of the config files.
fn sources(args: &mut pico_args::Arguments, mode: Mode) -> Result<Sources, String> {
  let error = |error: pico_args::Error| error.to_string();
  let mut flags = Config::default();
  let lang: Option<String> = args.opt_value_from_str("--lang").map_err(error)?;
  let language = match lang {
    Some(name) => Some(Language::of_name(&name).ok_or_else(|| {
      let command = if mode == Mode::Up { "up" } else { "check" };
      format!("unknown language `{name}`, see `tidy {command} --help`")
    })?),
    None => None,
  };
  let skip_unknown = args.contains("--skip-unknown");
  let fixed: Option<PathBuf> = args.opt_value_from_str("--config").map_err(error)?;
  let resolver = match (args.contains("--no-config"), fixed) {
    (true, _) => None,
    (false, Some(path)) => Some(Resolver::fixed(Config::read(&path)?)),
    (false, None) => Some(Resolver::default()),
  };
  flags.shared = flags::shared(args)?;
  Ok(Sources { resolver, flags, language, skip_unknown })
}

/// The files git names for `--staged` or `--changed`, below the paths given if any. `None` without either flag.
fn pick(scope: &Scope, paths: &[PathBuf]) -> Result<Option<Vec<PathBuf>>, String> {
  let listed = match scope {
    Scope::All => return Ok(None),
    Scope::Staged => {
      git::git_listed(&[&["diff", "--name-only", "--cached", "--diff-filter=ACMR"]])?
    }
    Scope::Changed(base) => {
      let base = match base {
        Some(base) => base.clone(),
        None => git::default_branch()?,
      };
      let from = git::git(&["merge-base", &base, "HEAD"])
        .ok()
        .map(|text| text.trim().to_owned())
        .unwrap_or(base);
      // A file that is new and not yet added still differs from the branch.
      git::git_listed(&[
        &["diff", "--name-only", "--diff-filter=ACMR", &from],
        &["ls-files", "--others", "--exclude-standard"],
      ])?
    }
  };
  // A path git named is relative when it sits below this folder, and
  // absolute when it sits above it, so `.` is the relative ones.
  let under = |path: &Path| {
    paths.is_empty()
      || paths.iter().any(|root| {
        let root = root.strip_prefix(".").unwrap_or(root);
        match root.as_os_str().is_empty() {
          true => path.is_relative(),
          false => path.starts_with(root),
        }
      })
  };
  Ok(Some(listed.into_iter().filter(|path| under(path)).collect()))
}

/// Whether to format a file found in a folder: whether its language is known.
fn wanted(_: &Path, name: &std::ffi::OsStr) -> bool {
  Language::of_path(&name.to_string_lossy()).is_some()
}

/// Formats `paths`, rewriting them for `up`, and reports what needed it. `implicit` is the folder tidy took when
/// given none, which goes unnamed in what it prints, as in ripgrep.
fn walk(request: &Request, paths: Vec<PathBuf>, implicit: bool) -> Result<ExitCode, String> {
  let named = |path: &Path| -> PathBuf {
    match implicit {
      true => path.strip_prefix(".").unwrap_or(path).to_path_buf(),
      false => path.to_path_buf(),
    }
  };
  let write = request.mode == Mode::Up;
  let gitignore = request.gitignore;
  let mut options =
    tidy::walk::Options { wanted: &wanted, gitignore, ..tidy::walk::Options::default() };
  match request.threads.map(|threads| threads.max(1)) {
    Some(1) => (options.io, options.workers) = (1, 0),
    Some(threads) if options.workers == 0 => options.io = threads,
    Some(threads) => (options.io, options.workers) = (options.io.min(threads), threads),
    None => {}
  }
  let sources = &request.sources;
  let listing = |dir: &Path, entries: &[tidy::walk::Entry]| {
    if let Some(resolver) = &sources.resolver {
      let configs = entries.iter().filter(|entry| entry.kind == tidy::walk::Kind::File);
      resolver.listing(dir, configs.map(|entry| entry.name));
    }
  };
  let report = tidy::walk::run(&paths, &options, listing, Formatter::default, |formatter, file| {
    let path = named(&file.path);
    let source = match std::str::from_utf8(&file.bytes) {
      Ok(source) => source,
      Err(error) => {
        return Some(Output { value: Err(format!("{}: {error}", shown(&path))), write: None });
      }
    };
    match sources.format(formatter, source, &path) {
      Ok(None) => None,
      Ok(Some(output)) if output == source => None,
      Ok(Some(output)) => {
        Some(Output { value: Ok(()), write: write.then(|| output.as_bytes().to_vec()) })
      }
      Err(error) => Some(Output { value: Err(error), write: None }),
    }
  });
  if !print(&report, named)? {
    return Ok(ExitCode::from(2));
  }
  // What was staged is what should be committed, so stage the rewrite too.
  if write && matches!(request.scope, Scope::Staged) {
    let rewritten: Vec<&Path> = report
      .results
      .iter()
      .filter(|(_, outcome)| outcome.is_ok())
      .map(|(path, _)| path.as_path())
      .collect();
    if !rewritten.is_empty() {
      git::stage(&rewritten)?;
    }
  }
  let check = request.mode == Mode::Check;
  Ok(if check && !report.results.is_empty() { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

/// Prints the files that needed formatting, and the errors. Returns whether there were none of the latter.
fn print(
  report: &tidy::walk::Report<Result<(), String>>,
  named: impl Fn(&Path) -> PathBuf,
) -> Result<bool, String> {
  let (mut listed, mut errors) = (String::new(), String::new());
  for (path, outcome) in &report.results {
    match outcome {
      Ok(()) => listed.push_str(&format!("{}\n", shown(&named(path)))),
      Err(error) => errors.push_str(&format!("tidy: {error}\n")),
    }
  }
  for (path, error) in &report.errors {
    errors.push_str(&format!("tidy: {}: {error}\n", shown(&named(path))));
  }
  std::io::stdout().write_all(listed.as_bytes()).map_err(|error| error.to_string())?;
  std::io::stderr().write_all(errors.as_bytes()).map_err(|error| error.to_string())?;
  Ok(errors.is_empty())
}
