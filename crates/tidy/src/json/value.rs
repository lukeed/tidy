//! The value of a JSON text, as `JSON5.parse` reads it, for config files.

use std::borrow::Cow;

use serde_json::{Map, Number, Value};

use super::ast::{Comment, Kind, Span};
use super::parse::Build;
use super::{cook, number};
use crate::Error;

/// The value of `text`, or null if it only has comments. `Infinity` and `NaN` are null, as `JSON.stringify` writes them.
pub fn value(text: &str) -> Result<Value, Error> {
  crate::read_without_bom(text, document)
}

fn document(text: &str) -> Result<Value, Error> {
  let mut values = Values { text, pairs: Vec::new(), elements: Vec::new() };
  Ok(super::read(text, &mut values)?.unwrap_or(Value::Null))
}

/// Builds the value as it parses, with the pairs and elements of the objects and arrays that are open, innermost last.
struct Values<'v> {
  text: &'v str,
  pairs: Vec<(String, Value)>,
  elements: Vec<Value>,
}

impl Build for Values<'_> {
  type Node = Value;
  type Key = String;

  fn leaf(&mut self, kind: Kind, span: Span) -> Value {
    let raw = span.of(self.text);
    match kind {
      Kind::String => Value::String(string(raw, cook::string)),
      Kind::Template => Value::String(string(raw, cook::template)),
      Kind::Number => number(raw),
      Kind::True => Value::Bool(true),
      Kind::False => Value::Bool(false),
      _ => Value::Null,
    }
  }

  fn key(&mut self, kind: Kind, span: Span) -> String {
    let raw = span.of(self.text);
    match kind {
      Kind::String => string(raw, cook::string),
      Kind::Number => number::string(float(&raw.replace('_', ""))),
      _ => cook::identifier(raw.as_bytes()).into_owned(),
    }
  }

  fn sign(&mut self, minus: bool, argument: Value, _: Span) -> Value {
    match (minus, argument) {
      (true, Value::Number(number)) => negate(&number),
      (_, argument) => argument,
    }
  }

  fn open(&self, object: bool) -> usize {
    if object { self.pairs.len() } else { self.elements.len() }
  }

  fn property(&mut self, key: String, value: Value, _: Span) {
    self.pairs.push((key, value));
  }

  fn element(&mut self, value: Value) {
    self.elements.push(value);
  }

  fn hole(&mut self) -> bool {
    false
  }

  fn object(&mut self, open: usize, _: Span) -> Value {
    let mut map = Map::new();
    for (key, value) in self.pairs.drain(open..) {
      map.insert(key, value);
    }
    Value::Object(map)
  }

  fn array(&mut self, open: usize, _: Span) -> Value {
    Value::Array(self.elements.drain(open..).collect())
  }

  fn comment(&mut self, _: Comment) {}
}

/// `-number`, where zero is `-0` and a whole number past `i64` a float.
fn negate(number: &Number) -> Value {
  let negated = match number.as_u64() {
    Some(0) => Number::from_f64(-0.0),
    Some(whole) => i64::try_from(whole)
      .ok()
      .map(|whole| Number::from(-whole))
      .or_else(|| Number::from_f64(-(whole as f64))),
    None => number.as_f64().and_then(|value| Number::from_f64(-value)),
  };
  negated.map_or(Value::Null, Value::Number)
}

/// A whole number as an integer, any other as a float, and an infinite one as null, as `JSON.stringify` writes it.
fn number(raw: &str) -> Value {
  let digits = match raw.contains('_') {
    true => Cow::Owned(raw.replace('_', "")),
    false => Cow::Borrowed(raw),
  };
  let whole = split(&digits).and_then(|(radix, digits)| u64::from_str_radix(digits, radix).ok());
  let number = whole.map(Number::from).or_else(|| Number::from_f64(float(&digits)));
  number.map_or(Value::Null, Value::Number)
}

/// The radix and digits of a whole number, without `_`: `0x1F`, `017`, `42`. None for a fraction or an exponent.
fn split(digits: &str) -> Option<(u32, &str)> {
  let radix = match digits.as_bytes() {
    [b'0', b'x' | b'X', ..] => 16,
    [b'0', b'o' | b'O', ..] => 8,
    [b'0', b'b' | b'B', ..] => 2,
    [b'0', rest @ ..]
      if !rest.is_empty() && rest.iter().all(|byte| matches!(byte, b'0'..=b'7')) =>
    {
      return Some((8, &digits[1..]));
    }
    bytes if bytes.iter().all(u8::is_ascii_digit) => return Some((10, digits)),
    _ => return None,
  };
  Some((radix, &digits[2..]))
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
