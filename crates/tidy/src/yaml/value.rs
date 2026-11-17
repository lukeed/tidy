//! The value of a YAML text, for config files: mappings, sequences and scalars, with plain scalars typed by YAML 1.2's
//! core schema. Aliases are an error.

use serde_json::{Map, Number, Value};

use super::{Id, Kind, Root};
use crate::Error;

/// The value of the first document of `text`, or null if it has none.
pub fn value(text: &str) -> Result<Value, Error> {
  let root = super::parse(text)?;
  let document = root.node(root.root).children().first().copied();
  document.map_or(Ok(Value::Null), |id| node(&root, id))
}

fn node(root: &Root, id: Id) -> Result<Value, Error> {
  let node = root.node(id);
  let content = || {
    let child = node.children().iter().copied().find(|&it| root.node(it).kind != Kind::Comment);
    child.map_or(Ok(Value::Null), |it| self::node(root, it))
  };
  match node.kind {
    Kind::Document => {
      let body =
        node.children().iter().copied().find(|&it| root.node(it).kind == Kind::DocumentBody);
      body.map_or(Ok(Value::Null), |it| self::node(root, it))
    }
    Kind::DocumentBody
    | Kind::MappingKey
    | Kind::MappingValue
    | Kind::SequenceItem
    | Kind::FlowSequenceItem => content(),
    Kind::Mapping | Kind::FlowMapping => {
      let mut map = Map::new();
      for &item in node.children() {
        let (mut key, mut value) = (Value::Null, Value::Null);
        for &part in root.node(item).children() {
          match root.node(part).kind {
            Kind::MappingKey => key = self::node(root, part)?,
            Kind::MappingValue => value = self::node(root, part)?,
            _ => {}
          }
        }
        let key = match key {
          Value::String(text) => text,
          Value::Null => String::new(),
          other => other.to_string(),
        };
        map.insert(key, value);
      }
      Ok(Value::Object(map))
    }
    Kind::Sequence | Kind::FlowSequence => node
      .children()
      .iter()
      .map(|&it| self::node(root, it))
      .collect::<Result<_, _>>()
      .map(Value::Array),
    Kind::Plain => Ok(plain(node.value())),
    Kind::QuoteSingle | Kind::QuoteDouble | Kind::BlockLiteral | Kind::BlockFolded => {
      Ok(Value::String(node.value().to_owned()))
    }
    Kind::Alias => Err(Error::Syntax {
      message: "aliases are not supported here".into(),
      at: node.position.start.offset as usize,
    }),
    _ => Ok(Value::Null),
  }
}

/// A plain scalar as YAML 1.2's core schema types it.
fn plain(text: &str) -> Value {
  match text {
    "" | "~" | "null" | "Null" | "NULL" => return Value::Null,
    "true" | "True" | "TRUE" => return Value::Bool(true),
    "false" | "False" | "FALSE" => return Value::Bool(false),
    _ => {}
  }
  let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
  if !digits.is_empty()
    && digits.bytes().all(|it| it.is_ascii_digit())
    && let Ok(number) = text.parse::<i64>()
  {
    return Value::from(number);
  }
  let radix = |prefix: &str, radix: u32| {
    text.strip_prefix(prefix).and_then(|it| i64::from_str_radix(it, radix).ok())
  };
  if let Some(number) = radix("0x", 16).or_else(|| radix("0o", 8)) {
    return Value::from(number);
  }
  let is_float = digits.bytes().any(|it| it.is_ascii_digit())
    && digits
      .bytes()
      .all(|it| it.is_ascii_digit() || matches!(it, b'.' | b'e' | b'E' | b'-' | b'+'));
  match text.parse::<f64>().ok().filter(|_| is_float).and_then(Number::from_f64) {
    Some(number) => Value::Number(number),
    None => Value::String(text.to_owned()),
  }
}

#[cfg(test)]
mod tests {
  use super::value;
  use serde_json::json;

  #[test]
  fn reads_a_prettier_config() {
    let text = "# comment\nprintWidth: 100\nsemi: false\nsingleQuote: true\ntrailingComma: \"none\"\nplugins:\n  - a\n  - 'b'\noverrides:\n  - files: \"*.md\"\n    options: { proseWrap: always, tabWidth: 4 }\nempty:\nratio: 1.5\nhex: 0x10\n";
    let expected = json!({
      "printWidth": 100, "semi": false, "singleQuote": true, "trailingComma": "none", "plugins": ["a", "b"],
      "overrides": [{ "files": "*.md", "options": { "proseWrap": "always", "tabWidth": 4 } }],
      "empty": null, "ratio": 1.5, "hex": 16,
    });
    assert_eq!(value(text).unwrap(), expected);
  }

  #[test]
  fn is_null_without_a_document_and_rejects_aliases() {
    assert_eq!(value("# only a comment\n").unwrap(), serde_json::Value::Null);
    assert!(value("a: &x 1\nb: *x\n").is_err());
  }
}
