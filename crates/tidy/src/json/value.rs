//! The value of a JSON text, as `JSON5.parse` reads it, for config files.

use std::borrow::Cow;

use serde_json::{Map, Number, Value};

use super::ast::{Kind, NodeId, Tree};
use super::{cook, number};
use crate::Error;

/// The value of `text`, or null if it only has comments. `Infinity` and `NaN` are null, as `JSON.stringify` writes them.
pub fn value(text: &str) -> Result<Value, Error> {
  let tree = super::parse(text)?;
  let Some(root) = tree.root() else { return Ok(Value::Null) };
  Reader { text, tree: &tree }.value(root)
}

struct Reader<'r> {
  text: &'r str,
  tree: &'r Tree,
}

impl Reader<'_> {
  fn value(&self, id: NodeId) -> Result<Value, Error> {
    let node = self.tree.node(id);
    let raw = node.span.of(self.text);
    Ok(match node.kind {
      Kind::Object(items) => {
        let mut map = Map::new();
        for property in self.tree.items(items).iter().flatten() {
          let Kind::Property { key, value } = self.tree.node(*property).kind else { continue };
          map.insert(self.key(key), self.value(value)?);
        }
        Value::Object(map)
      }
      Kind::Array(items) => {
        let mut values = Vec::with_capacity(items.len());
        for item in self.tree.items(items) {
          let Some(item) = item else {
            return Err(error("An array can't have holes", node.span.start));
          };
          values.push(self.value(*item)?);
        }
        Value::Array(values)
      }
      Kind::String => Value::String(string(raw, cook::string)),
      Kind::Template => Value::String(string(raw, cook::template)),
      Kind::Number => number(raw, false),
      Kind::Sign { minus, argument } => {
        let argument = self.tree.node(argument);
        match argument.kind {
          Kind::Number => number(argument.span.of(self.text), minus),
          _ => Value::Null,
        }
      }
      Kind::Identifier | Kind::Null => Value::Null,
      Kind::True => Value::Bool(true),
      Kind::False => Value::Bool(false),
      Kind::Property { .. } => unreachable!("a property is only in an object"),
    })
  }

  fn key(&self, id: NodeId) -> String {
    let node = self.tree.node(id);
    let raw = node.span.of(self.text);
    match node.kind {
      Kind::String => string(raw, cook::string),
      Kind::Number => number::string(float(&raw.replace('_', ""))),
      _ => cook::identifier(raw.as_bytes()).into_owned(),
    }
  }
}

/// A whole number as an integer, any other as a float, and an infinite one as null, as `JSON.stringify` writes it.
fn number(raw: &str, minus: bool) -> Value {
  let digits = match raw.contains('_') {
    true => Cow::Owned(raw.replace('_', "")),
    false => Cow::Borrowed(raw),
  };
  let whole = split(&digits).and_then(|(radix, digits)| u64::from_str_radix(digits, radix).ok());
  let number = match whole {
    Some(whole) if !minus => Some(Number::from(whole)),
    Some(0) => None,
    Some(whole) => i64::try_from(whole).ok().map(|whole| Number::from(-whole)),
    None => None,
  };
  let float = || Number::from_f64(if minus { -float(&digits) } else { float(&digits) });
  number.or_else(float).map_or(Value::Null, Value::Number)
}

/// The radix and digits of a whole number, without `_`: `0x1F`, `017`, `42`. None for a fraction or an exponent.
fn split(digits: &str) -> Option<(u32, &str)> {
  let legacy = digits.len() > 1 && digits.bytes().all(|byte| matches!(byte, b'0'..=b'7'));
  match digits.get(..2).map(str::to_ascii_lowercase).as_deref() {
    Some("0x") => Some((16, &digits[2..])),
    Some("0o") => Some((8, &digits[2..])),
    Some("0b") => Some((2, &digits[2..])),
    _ if legacy && digits.starts_with('0') => Some((8, &digits[1..])),
    _ if digits.bytes().all(|byte| byte.is_ascii_digit()) => Some((10, digits)),
    _ => None,
  }
}

fn float(digits: &str) -> f64 {
  match split(digits) {
    Some((radix, digits)) if radix != 10 => digits
      .chars()
      .fold(0.0, |value, digit| value * radix as f64 + digit.to_digit(radix).unwrap_or(0) as f64),
    _ => digits.parse().unwrap_or(f64::NAN),
  }
}

/// The value of a string or a template literal, from its text with its quotes.
fn string(raw: &str, cook: fn(&[u8]) -> Vec<u16>) -> String {
  let content = &raw[1..raw.len() - 1];
  match memchr::memchr2(b'\\', b'\r', content.as_bytes()) {
    Some(_) => String::from_utf16_lossy(&cook(content.as_bytes())),
    None => content.to_owned(),
  }
}

fn error(message: &str, at: u32) -> Error {
  Error::Syntax { message: message.to_owned(), at: at as usize }
}
