//! tidy compiled for the browser, behind the one call the playground makes.

use tidy::config::Config;
use tidy::embed::Verbatim;
use tidy::{Error, Formatter, Language, Options, css, handlebars, ini, js, json};
use wasm_bindgen::prelude::*;

/// Prettier's parser names, as `--parser` and `tidy.json` use them.
fn language(name: &str) -> Option<(Language, Option<handlebars::Dialect>)> {
  Some(match name {
    "html" => (Language::Html, None),
    "vue" => (Language::Vue, None),
    "mjml" => (Language::Mjml, None),
    "handlebars" => (Language::Handlebars, None),
    "tempura" => (Language::Handlebars, Some(handlebars::Dialect::Tempura)),
    "css" => (Language::Css(css::Dialect::Css), None),
    "scss" => (Language::Css(css::Dialect::Scss), None),
    "less" => (Language::Css(css::Dialect::Less), None),
    "json" => (Language::Json(json::Dialect::Json), None),
    "jsonc" => (Language::Json(json::Dialect::Jsonc), None),
    "json5" => (Language::Json(json::Dialect::Json5), None),
    "json-stringify" => (Language::Json(json::Dialect::Stringify), None),
    "yaml" => (Language::Yaml, None),
    "toml" => (Language::Toml, None),
    "dockerfile" => (Language::Dockerfile, None),
    "ini" => (Language::Ini(ini::Dialect::Ini), None),
    "gitconfig" => (Language::Ini(ini::Dialect::Git), None),
    "graphql" => (Language::Graphql, None),
    "babel" => (Language::Js(js::Dialect::Js), None),
    "typescript" | "babel-ts" => (Language::Js(js::Dialect::Ts), None),
    "xml" => (Language::Xml, None),
    "markdown" => (Language::Markdown, None),
    _ => return None,
  })
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

/// Format `source` as `parser`, with the options of a `tidy.json` or `tidy.toml` text.
///
/// `config` may be empty, for the defaults. `filepath` decides the details that
/// a file name settles, such as whether a `.hbs` template is HTML.
#[wasm_bindgen]
pub fn format(
  source: &str,
  parser: &str,
  config: &str,
  filepath: Option<String>,
) -> Result<String, JsError> {
  let (language, dialect) =
    language(parser).ok_or_else(|| JsError::new(&format!("unknown parser `{parser}`")))?;

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
