//! The value of a TOML document, by the rules of TOML 1.0, for config files.

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde_json::{Map, Number, Value};

use super::lexer::Token;
use super::tree::{Id, Kind, Tree};
use crate::Error;

/// The value of `text`: a table.
pub fn value(text: &str) -> Result<Value, Error> {
  let tree = super::parse(text)?;
  let reader = Reader { text, tree: &tree };
  let mut root = Table::new(Made::Header);
  let (mut current, mut keys) = (Vec::new(), Vec::new());
  for &child in tree.children(tree.root()) {
    match tree.kind(child) {
      Kind::Entry => reader.entry(child, open(&mut root, &current), &mut keys)?,
      Kind::TableHeader | Kind::TableArrayHeader => {
        reader.keys(reader.child(child, Kind::Key), &mut current);
        header(&mut root, &current, tree.kind(child) == Kind::TableArrayHeader)?;
      }
      _ => {}
    }
  }
  Ok(root.into_value())
}

/// A part of a key, and where it is.
type Key<'t> = (Cow<'t, str>, u32);

enum Item {
  Value(Value),
  Table(Table),
  /// An array of tables.
  Tables(Vec<Table>),
}

/// How a table came to be, which decides what can add to it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Made {
  /// By a header for a table in it.
  Implicit,
  Header,
  /// By a dotted key.
  Dotted,
}

struct Table {
  made: Made,
  items: BTreeMap<String, Item>,
}

impl Table {
  fn new(made: Made) -> Table {
    Table { made, items: BTreeMap::new() }
  }

  fn into_value(self) -> Value {
    let items = self.items.into_iter().map(|(key, item)| {
      let value = match item {
        Item::Value(value) => value,
        Item::Table(table) => table.into_value(),
        Item::Tables(tables) => Value::Array(tables.into_iter().map(Table::into_value).collect()),
      };
      (key, value)
    });
    Value::Object(items.collect::<Map<_, _>>())
  }
}

/// Defines the table of a `[header]` or `[[header]]`.
fn header(root: &mut Table, keys: &[Key], array: bool) -> Result<(), Error> {
  let (last, parents) = keys.split_last().expect("a header has a key");
  let mut table = root;
  for (index, (key, at)) in parents.iter().enumerate() {
    table = match slot(&mut table.items, key, Made::Implicit) {
      Item::Table(table) => table,
      Item::Tables(tables) => tables.last_mut().expect("an array of tables has a table"),
      Item::Value(_) => return Err(defined(&keys[..=index], *at)),
    };
  }
  match (table.items.get_mut(&*last.0), array) {
    (None, false) => {
      _ = table.items.insert(last.0.to_string(), Item::Table(Table::new(Made::Header)))
    }
    (None, true) => {
      _ = table.items.insert(last.0.to_string(), Item::Tables(vec![Table::new(Made::Header)]))
    }
    (Some(Item::Table(table)), false) if table.made == Made::Implicit => table.made = Made::Header,
    (Some(Item::Tables(tables)), true) => tables.push(Table::new(Made::Header)),
    _ => return Err(defined(keys, last.1)),
  }
  Ok(())
}

/// The table that a header defined, where its entries go.
fn open<'t>(root: &'t mut Table, keys: &[Key]) -> &'t mut Table {
  keys.iter().fold(root, |table, (key, _)| match table.items.get_mut(&**key) {
    Some(Item::Table(table)) => table,
    Some(Item::Tables(tables)) => tables.last_mut().expect("an array of tables has a table"),
    _ => unreachable!("a header defines its tables"),
  })
}

/// Adds an entry, and the tables of its dotted key.
fn insert(table: &mut Table, keys: &[Key], value: Value) -> Result<(), Error> {
  let (last, parents) = keys.split_last().expect("an entry has a key");
  let mut table = table;
  for (index, (key, at)) in parents.iter().enumerate() {
    table = match slot(&mut table.items, key, Made::Dotted) {
      Item::Table(table) if table.made == Made::Dotted => table,
      _ => return Err(defined(&keys[..=index], *at)),
    };
  }
  if table.items.contains_key(&*last.0) {
    return Err(defined(keys, last.1));
  }
  table.items.insert(last.0.to_string(), Item::Value(value));
  Ok(())
}

/// The item at `key`, which is a new table if there was none.
fn slot<'i>(items: &'i mut BTreeMap<String, Item>, key: &str, made: Made) -> &'i mut Item {
  if !items.contains_key(key) {
    items.insert(key.to_owned(), Item::Table(Table::new(made)));
  }
  items.get_mut(key).expect("the item is there")
}

fn defined(keys: &[Key], at: u32) -> Error {
  let path = keys.iter().map(|(key, _)| &**key).collect::<Vec<_>>().join(".");
  error(&format!("`{path}` is already defined"), at)
}

struct Reader<'r> {
  text: &'r str,
  tree: &'r Tree,
}

impl<'r> Reader<'r> {
  /// Adds an entry to `table`, with `keys` as a buffer for its key.
  fn entry(&self, id: Id, table: &mut Table, keys: &mut Vec<Key<'r>>) -> Result<(), Error> {
    let value = self.value(self.child(id, Kind::Value))?;
    self.keys(self.child(id, Kind::Key), keys);
    insert(table, keys, value)
  }

  fn child(&self, id: Id, kind: Kind) -> Id {
    let mut children = self.tree.children(id).iter().copied();
    children.find(|child| self.tree.kind(*child) == kind).expect("the parser made the child")
  }

  fn keys(&self, id: Id, keys: &mut Vec<Key<'r>>) {
    keys.clear();
    let idents = self.tree.children(id).iter().filter(|child| self.tree.is(**child, Token::Ident));
    keys.extend(
      idents.map(|ident| (key(self.tree.text(*ident, self.text)), self.tree.start(*ident))),
    );
  }

  fn value(&self, id: Id) -> Result<Value, Error> {
    let mut children = self.tree.children(id).iter().copied();
    let trivia =
      |kind| matches!(kind, Kind::Token(Token::Whitespace | Token::Newline | Token::Comment));
    let child =
      children.find(|child| !trivia(self.tree.kind(*child))).expect("a value has a child");
    let raw = self.tree.text(child, self.text);
    let at = self.tree.start(child);
    Ok(match self.tree.kind(child) {
      Kind::Array => {
        let values = self.tree.nodes(child).map(|value| self.value(value));
        Value::Array(values.collect::<Result<_, _>>()?)
      }
      Kind::InlineTable => {
        let (mut table, mut keys) = (Table::new(Made::Header), Vec::new());
        for entry in self.tree.nodes(child) {
          self.entry(entry, &mut table, &mut keys)?;
        }
        table.into_value()
      }
      Kind::Token(Token::String) => Value::String(basic(&raw[1..raw.len() - 1]).into_owned()),
      Kind::Token(Token::MultiLineString) => {
        Value::String(basic(trim(&raw[3..raw.len() - 3])).into_owned())
      }
      Kind::Token(Token::StringLiteral) => Value::String(raw[1..raw.len() - 1].to_owned()),
      Kind::Token(Token::MultiLineStringLiteral) => {
        Value::String(trim(&raw[3..raw.len() - 3]).to_owned())
      }
      Kind::Token(Token::Integer | Token::IntegerHex | Token::IntegerOct | Token::IntegerBin) => {
        match integer(raw) {
          Some(value) => value,
          // logos lexes some dates as integers, and taplo keeps their kind.
          None if raw[1..].contains(['-', ':']) => date(raw),
          None => return Err(error("The integer is out of range", at)),
        }
      }
      Kind::Token(Token::Float) => {
        let value = digits(raw).parse().ok().and_then(Number::from_f64);
        value.map_or(Value::Null, Value::Number)
      }
      Kind::Token(Token::Bool) => Value::Bool(raw == "true"),
      _ => date(raw),
    })
  }
}

/// A part of a key: bare, or quoted.
fn key(raw: &str) -> Cow<'_, str> {
  match raw.as_bytes()[0] {
    b'"' => basic(&raw[1..raw.len() - 1]),
    b'\'' => Cow::Borrowed(&raw[1..raw.len() - 1]),
    _ => Cow::Borrowed(raw),
  }
}

fn integer(raw: &str) -> Option<Value> {
  let digits = digits(raw);
  let value = match digits.get(..2) {
    Some("0x") => i64::from_str_radix(&digits[2..], 16),
    Some("0o") => i64::from_str_radix(&digits[2..], 8),
    Some("0b") => i64::from_str_radix(&digits[2..], 2),
    _ => digits.parse(),
  };
  value.ok().map(Value::from)
}

/// A number without its `_`.
fn digits(raw: &str) -> Cow<'_, str> {
  match raw.contains('_') {
    true => Cow::Owned(raw.replace('_', "")),
    false => Cow::Borrowed(raw),
  }
}

/// A date or a time, in RFC 3339's form.
fn date(raw: &str) -> Value {
  Value::String(raw.replace([' ', 't'], "T").replace('z', "Z"))
}

/// A multi-line string without the line break right after its opening quotes.
fn trim(content: &str) -> &str {
  content.strip_prefix('\n').or_else(|| content.strip_prefix("\r\n")).unwrap_or(content)
}

/// The value of a basic string, with its escapes read. The parser checked them.
fn basic(content: &str) -> Cow<'_, str> {
  if memchr::memchr(b'\\', content.as_bytes()).is_none() {
    return Cow::Borrowed(content);
  }
  let mut value = String::with_capacity(content.len());
  let mut characters = content.chars().peekable();
  while let Some(character) = characters.next() {
    if character != '\\' {
      value.push(character);
      continue;
    }
    match characters.next() {
      Some('b') => value.push('\u{8}'),
      Some('t') => value.push('\t'),
      Some('n') => value.push('\n'),
      Some('f') => value.push('\u{c}'),
      Some('r') => value.push('\r'),
      Some(escape @ ('u' | 'U')) => {
        let count = if escape == 'u' { 4 } else { 8 };
        let digits: String = characters.by_ref().take(count).collect();
        value.extend(u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32));
      }
      Some(character) if character.is_whitespace() => {
        while characters.next_if(|character| character.is_whitespace()).is_some() {}
      }
      Some(character) => value.push(character),
      None => {}
    }
  }
  Cow::Owned(value)
}

fn error(message: &str, at: u32) -> Error {
  Error::Syntax { message: message.to_owned(), at: at as usize }
}
