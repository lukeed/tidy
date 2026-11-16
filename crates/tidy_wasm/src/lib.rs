//! tidy compiled for the browser, behind the one call the playground makes.

use tidy::config::Config;
use tidy::embed::Verbatim;
use tidy::{Error, Formatter, Language, Options, handlebars};
use wasm_bindgen::prelude::*;

/// The language of `lang`, as `--lang` reads it, and the Handlebars dialect it asks for.
fn language(lang: &str) -> Option<(Language, Option<handlebars::Dialect>)> {
  match lang {
    // A dialect of Handlebars, which tidy otherwise tells from the template.
    "tempura" => Some((Language::Handlebars, Some(handlebars::Dialect::Tempura))),
    name => Language::hint(name).map(|it| (it, None)),
  }
}

/// The line and column of a syntax error, as the command line prints them.
fn describe(error: &Error, source: &str) -> String {
  match error {
    Error::Syntax { message, at } => {
      let before = &source[..(*at).min(source.len())];
      let line = before.matches('\n').count() + 1;
      let column = before.rsplit('\n').next().unwrap_or_default().chars().count() + 1;
      format!("{line}:{column}: {message}")
    }
    Error::TooDeep => error.to_string(),
  }
}

/// Format `source` as `lang`, with the options of a `tidy.json` or `tidy.toml` text.
///
/// `config` may be empty, for the defaults. `filepath` decides the details that
/// a file name settles, such as whether a `.hbs` template is HTML.
#[wasm_bindgen]
pub fn format(
  source: &str,
  lang: &str,
  config: &str,
  filepath: Option<String>,
) -> Result<String, JsError> {
  let (language, dialect) =
    language(lang).ok_or_else(|| JsError::new(&format!("unknown language `{lang}`")))?;

  let mut options = Options::default();
  let text = config.trim();
  if !text.is_empty() {
    let parsed = match text.starts_with('{') {
      true => Config::json(text),
      false => Config::toml(text),
    };
    parsed.map_err(|error| JsError::new(&error))?.apply(&mut options);
  }
  if let Some(dialect) = dialect {
    options.handlebars.dialect = dialect;
  }

  Formatter::default()
    .format(source, language, &options, filepath.as_deref(), &mut Verbatim)
    .map(str::to_owned)
    .map_err(|error| JsError::new(&describe(&error, source)))
}

/// The version of tidy this build came from.
#[wasm_bindgen]
pub fn version() -> String {
  env!("CARGO_PKG_VERSION").to_owned()
}
