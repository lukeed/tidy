//! What tidy's bindings for JavaScript share, so that `tidyrs` and `tidyrs-wasm` read a call the same way: its
//! language, its options, the request a hook gets, and the errors it throws.

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

/// What a binding throws: the message of an error, and for a syntax error, the line and column where it is, from 1.
/// The column counts characters.
#[derive(Debug, PartialEq, Eq)]
pub struct Problem {
  pub message: String,
  pub position: Option<(usize, usize)>,
}

impl Problem {
  /// The problem of an error in `source`.
  pub fn of(error: &Error, source: &str) -> Problem {
    let position = match error {
      Error::Syntax { at, .. } => {
        let before = &source[..(*at).min(source.len())];
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap_or_default().chars().count() + 1;
        Some((line, column))
      }
      _ => None,
    };
    Problem { message: error.to_string(), position }
  }
}

impl Problem {
  /// Where the problem is, as Prettier puts it on a syntax error: `{ start: { line, column } }`.
  pub fn loc(&self) -> Option<Value> {
    let (line, column) = self.position?;
    Some(json!({ "start": { "line": line, "column": column } }))
  }
}

impl From<String> for Problem {
  fn from(message: String) -> Problem {
    Problem { message, position: None }
  }
}

/// The options in the text of a `tidy.json`, `tidy.jsonc` or `tidy.toml`, as a JSON object. JSON starts with `{` or
/// a comment; anything else is TOML.
pub fn config(text: &str) -> Result<Value, Problem> {
  let trimmed = text.trim_start();
  let is_json = ["{", "//", "/*"].iter().any(|start| trimmed.starts_with(start));
  let value = match is_json {
    true => crate::json::value(text),
    false => crate::toml::value(text),
  }
  .map_err(|error| Problem::of(&error, text))?;
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
    let wide = config("{ \"width\": \"wide\" }").unwrap_err();
    assert_eq!(wide, Problem::from("`width` must be a whole number".to_owned()));
    assert_eq!(config("{\n  \"width\": ").unwrap_err().position, Some((2, 12)));
  }

  #[test]
  fn says_where_a_syntax_error_is() {
    let error = Error::Syntax { message: "Unexpected token".into(), at: 4 };
    let problem = Problem::of(&error, "a\nbc d");
    assert_eq!(problem, Problem { message: "Unexpected token".into(), position: Some((2, 3)) });
    assert_eq!(Problem::of(&Error::TooDeep, "").position, None);
  }
}
