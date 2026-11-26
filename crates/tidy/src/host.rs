//! What tidy's bindings for JavaScript share, so that `tidyrs` and `tidyrs-wasm` read a call the same way: its
//! language, its options, the request a hook gets, and the text of an error.

use serde_json::{Map, Value, json};

use crate::config::Config;
use crate::embed::Request;
use crate::{Error, Language, Options, handlebars};

/// The language of `lang`, or of `filepath`, and the Handlebars dialect that `lang` asks for, as `--lang` reads it.
pub fn language(
  lang: Option<&str>,
  filepath: Option<&str>,
) -> Result<(Language, Option<handlebars::Dialect>), String> {
  match (lang, filepath) {
    // A dialect of Handlebars, which tidy otherwise tells from the template.
    (Some("tempura"), _) => Ok((Language::Handlebars, Some(handlebars::Dialect::Tempura))),
    (Some(name), _) => {
      Language::hint(name).map(|it| (it, None)).ok_or_else(|| format!("unknown language: {name}"))
    }
    (None, Some(path)) => Language::file(path)
      .map(|it| (it, None))
      .ok_or_else(|| format!("{path}: can't tell the language of this file, pass `lang`")),
    (None, None) => Err("can't tell the language: pass `lang` or `filepath`".into()),
  }
}

/// The options of `config`, the keys of a `tidy.json`, with the Handlebars dialect that `lang` asked for.
pub fn options(
  config: Map<String, Value>,
  dialect: Option<handlebars::Dialect>,
) -> Result<Options, String> {
  let mut config = Config::from_value(&Value::Object(config))?;
  if dialect.is_some() {
    config.handlebars.dialect = dialect;
  }
  let mut options = Options::default();
  config.apply(&mut options);
  Ok(options)
}

/// The request as the hook gets it, an `Embed.Request`, without `host` where there is none.
pub fn request(request: &Request<'_>) -> Value {
  let mut value = json!({
    "code": request.code,
    "lang": request.lang,
    "kind": request.kind.name(),
    "attribute": request.attribute,
    "indent": request.indent,
    "prefix": request.prefix,
    "suffix": request.suffix,
    "width": request.width,
    "module": request.module,
  });
  if let (Some(host), Value::Object(map)) = (request.host, &mut value) {
    map.insert("host".into(), host.into());
  }
  value
}

/// An error formatting `source`, with the line and column of a syntax error before its message, as the command line
/// prints it.
pub fn describe(error: &Error, source: &str) -> String {
  match error {
    Error::Syntax { message, at } => {
      let before = &source[..(*at).min(source.len())];
      let line = before.matches('\n').count() + 1;
      let column = before.rsplit('\n').next().unwrap_or_default().chars().count() + 1;
      format!("{line}:{column}: {message}")
    }
    _ => error.to_string(),
  }
}

/// The options in the text of a `tidy.json`, `tidy.jsonc` or `tidy.toml`, as a JSON object. JSON starts with `{` or
/// a comment; anything else is TOML.
pub fn config(text: &str) -> Result<Value, String> {
  let trimmed = text.trim_start();
  let is_json = ["{", "//", "/*"].iter().any(|start| trimmed.starts_with(start));
  let value = match is_json {
    true => crate::json::value(text),
    false => crate::toml::value(text),
  }
  .map_err(|error| describe(&error, text))?;
  let value = match value {
    Value::Null => Value::Object(Map::new()),
    value => value,
  };
  Config::from_value(&value)?;
  Ok(value)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_the_language_as_lang_does() {
    assert_eq!(language(Some("ts"), Some("a.css")).unwrap().0, Language::file("a.ts").unwrap());
    assert_eq!(language(None, Some("a.css")).unwrap().0, Language::file("a.css").unwrap());
    assert!(language(Some("tempura"), None).unwrap().1.is_some());
    assert_eq!(language(Some("nope"), None).unwrap_err(), "unknown language: nope");
    assert!(language(None, None).is_err());
  }

  #[test]
  fn reads_a_config_of_either_format() {
    let json = config("// a comment\n{ \"width\": 100, \"js\": { \"semi\": false } }").unwrap();
    assert_eq!(json, json!({ "width": 100, "js": { "semi": false } }));
    let toml = config("width = 100\n\n[js]\nsemi = false\n").unwrap();
    assert_eq!(toml, json);
    assert_eq!(config("").unwrap(), json!({}));
    assert_eq!(config("{ \"width\": \"wide\" }").unwrap_err(), "`width` must be a whole number");
    assert!(config("{ \"width\": ").unwrap_err().starts_with("1:"));
  }

  #[test]
  fn puts_the_line_and_column_before_a_syntax_error() {
    let error = Error::Syntax { message: "Unexpected token".into(), at: 4 };
    assert_eq!(describe(&error, "a\nbc d"), "2:3: Unexpected token");
  }
}
