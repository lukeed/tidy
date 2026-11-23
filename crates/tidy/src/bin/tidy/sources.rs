//! Where each file's options come from, and how one text is formatted.

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use tidy::config::{Config, Resolver};
use tidy::embed::Verbatim;
use tidy::{Error, Formatter, Language, Options};

use crate::print;

/// Where the options of each file come from: Prettier's, then the config files around it, then the flags.
pub(crate) struct Sources {
  /// `None` with `--no-config`.
  pub(crate) resolver: Option<Resolver>,
  pub(crate) flags: Config,
  /// The language `--lang` forces, instead of the one of each name.
  pub(crate) language: Option<Language>,
  pub(crate) skip_unknown: bool,
}

impl Sources {
  fn options(&self, path: Option<&Path>) -> Result<Options, String> {
    let mut options = match (&self.resolver, path) {
      (Some(resolver), Some(path)) => resolver.options(path)?,
      (Some(resolver), None) => resolver.without_path(),
      (None, _) => Options::default(),
    };
    self.flags.apply(&mut options);
    Ok(options)
  }

  /// The language of the text at `path`, or of text with no path. `None` when tidy can't tell
  /// and `--skip-unknown` says to leave the text as it is.
  fn language(&self, path: Option<&str>) -> Result<Option<Language>, String> {
    match self.language.or_else(|| path.and_then(Language::file)) {
      Some(language) => Ok(Some(language)),
      None if self.skip_unknown => Ok(None),
      None => Err(match path {
        Some(name) => format!("{name}: can't tell the language of this file, pass --lang"),
        None => "can't tell the language of stdin, pass --lang or --stdin=<path>".into(),
      }),
    }
  }

  /// Formats the file at `path`, in the language `--lang` forces or the one of its name.
  /// `None` when `--skip-unknown` leaves it as it is.
  pub(crate) fn format<'f>(
    &self,
    formatter: &'f mut Formatter,
    source: &str,
    path: &Path,
  ) -> Result<Option<&'f str>, String> {
    let name = path.to_string_lossy();
    let Some(language) = self.language(Some(&name))? else {
      return Ok(None);
    };
    let options = self.options(Some(path)).map_err(|error| format!("{name}: {error}"))?;
    formatter
      .format(source, language, &options, Some(&name), &mut Verbatim)
      .map(Some)
      .map_err(|error| describe(&error, source, &name))
  }

  /// Formats stdin as if it were the file at `path`, and prints it, or with `check` only tells
  /// whether it is formatted.
  pub(crate) fn stdin(&self, path: Option<&Path>, check: bool) -> Result<ExitCode, String> {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).map_err(|error| format!("stdin: {error}"))?;
    let name = path.map(Path::to_string_lossy);
    let mut formatter = Formatter::default();
    let output = match self.language(name.as_deref())? {
      None => source.as_str(),
      Some(language) => {
        let options = match &name {
          Some(name) => self.options(path).map_err(|error| format!("{name}: {error}"))?,
          None => self.options(None)?,
        };
        formatter
          .format(&source, language, &options, name.as_deref(), &mut Verbatim)
          .map_err(|error| describe(&error, &source, name.as_deref().unwrap_or("stdin")))?
      }
    };
    match check {
      true if output == source => Ok(ExitCode::SUCCESS),
      true => Ok(ExitCode::FAILURE),
      false => print(output.as_bytes()).map(|()| ExitCode::SUCCESS),
    }
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
    _ => format!("{name}: {error}"),
  }
}
