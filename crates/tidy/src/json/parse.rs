//! What Prettier's JSON parsers accept: an expression as Babel's `parseExpression` reads it in a classic script, that
//! `assertJsonNode` then allows. That is JSON5 and more: comments, single quotes, unquoted keys, holes in arrays,
//! signs, `Infinity`, `NaN`, `undefined`, numbers in every JavaScript form, and template literals without expressions.

use super::Abort;
use super::ast::{Comment, Items, Kind, NodeId, Span, Tree};
use super::cook;
use crate::stack::Stack;
use crate::strings;
use crate::syntax::{Message, Rejection};

/// What the parser builds from what it reads: the tree that formats, or the value of a config.
pub(crate) trait Build {
  type Node;
  type Key;
  /// A string, number, template literal, identifier, `null`, `true` or `false`.
  fn leaf(&mut self, kind: Kind, span: Span) -> Self::Node;
  fn key(&mut self, kind: Kind, span: Span) -> Self::Key;
  fn sign(&mut self, minus: bool, argument: Self::Node, span: Span) -> Self::Node;
  /// Where the items of an object or array that opens start.
  fn open(&self, object: bool) -> usize;
  fn property(&mut self, key: Self::Key, value: Self::Node, span: Span);
  fn element(&mut self, value: Self::Node);
  /// Adds a hole to an array, if it can have one.
  fn hole(&mut self) -> bool;
  fn object(&mut self, open: usize, span: Span) -> Self::Node;
  fn array(&mut self, open: usize, span: Span) -> Self::Node;
  fn comment(&mut self, comment: Comment);
}

/// Builds the tree that formats.
pub(crate) struct Trees<'t> {
  pub(crate) tree: &'t mut Tree,
  /// The items of the objects and arrays that are open, innermost last.
  pub(crate) pending: &'t mut Vec<Option<NodeId>>,
}

impl Trees<'_> {
  fn items(&mut self, open: usize) -> Items {
    let start = self.tree.items.len() as u32;
    self.tree.items.extend(self.pending.drain(open..));
    Items { start, end: self.tree.items.len() as u32 }
  }
}

impl Build for Trees<'_> {
  type Node = NodeId;
  type Key = NodeId;

  fn leaf(&mut self, kind: Kind, span: Span) -> NodeId {
    self.tree.push(kind, span)
  }

  fn key(&mut self, kind: Kind, span: Span) -> NodeId {
    self.tree.push(kind, span)
  }

  fn sign(&mut self, minus: bool, argument: NodeId, span: Span) -> NodeId {
    self.tree.push(Kind::Sign { minus, argument }, span)
  }

  fn open(&self, _: bool) -> usize {
    self.pending.len()
  }

  fn property(&mut self, key: NodeId, value: NodeId, span: Span) {
    let property = self.tree.push(Kind::Property { key, value }, span);
    self.pending.push(Some(property));
  }

  fn element(&mut self, value: NodeId) {
    self.pending.push(Some(value));
  }

  fn hole(&mut self) -> bool {
    self.pending.push(None);
    true
  }

  fn object(&mut self, open: usize, span: Span) -> NodeId {
    let items = self.items(open);
    self.tree.push(Kind::Object(items), span)
  }

  fn array(&mut self, open: usize, span: Span) -> NodeId {
    let items = self.items(open);
    self.tree.push(Kind::Array(items), span)
  }

  fn comment(&mut self, comment: Comment) {
    self.tree.comments.push(comment);
  }
}

pub(crate) struct Parser<'p, B: Build> {
  pub(crate) text: &'p [u8],
  pub(crate) build: &'p mut B,
  pub(crate) rejection: &'p Rejection,
  pub(crate) stack: &'p Stack,
  pub(crate) at: usize,
}

impl<B: Build> Parser<'_, B> {
  /// The value of the text, or `None` if it only has comments.
  pub(crate) fn document(&mut self) -> Result<Option<B::Node>, Abort> {
    self.trivia()?;
    if self.at == self.text.len() {
      return Ok(None);
    }
    let root = self.value()?;
    self.trivia()?;
    match self.at == self.text.len() {
      true => Ok(Some(root)),
      false => Err(self.error(Message::UnexpectedToken)),
    }
  }

  fn value(&mut self) -> Result<B::Node, Abort> {
    if !self.stack.has_room() {
      return Err(Abort::TooDeep);
    }
    let start = self.at;
    match self.peek() {
      Some(b'{') => self.object(),
      Some(b'[') => self.array(),
      Some(b'"' | b'\'') => self.string(),
      Some(b'`') => self.template(),
      Some(b'+' | b'-') => self.sign(),
      Some(b'0'..=b'9') => self.number(),
      Some(b'.') if self.text.get(start + 1).is_some_and(u8::is_ascii_digit) => self.number(),
      Some(_) if self.identifier_length(start).is_some() => {
        let end = self.identifier()?;
        let raw = &self.text[start..end];
        let kind = match raw {
          b"true" => Kind::True,
          b"false" => Kind::False,
          b"null" => Kind::Null,
          _ if matches!(&*cook::identifier(raw), "Infinity" | "NaN" | "undefined") => {
            Kind::Identifier
          }
          _ => return Err(self.error_at(Message::NotJson, start)),
        };
        Ok(self.build.leaf(kind, span(start, end)))
      }
      Some(_) => Err(self.error(Message::UnexpectedToken)),
      None => Err(self.error(Message::UnexpectedEnd)),
    }
  }

  fn object(&mut self) -> Result<B::Node, Abort> {
    let start = self.at;
    self.at += 1;
    let open = self.build.open(true);
    let mut proto = None;
    loop {
      self.trivia()?;
      if self.peek() == Some(b'}') {
        break;
      }
      let key_start = self.at;
      let key = self.key()?;
      // Babel's `checkProto`: `__proto__` can only be a key once.
      if self.is_proto(key_start, self.at) {
        proto = match proto {
          None => Some(None),
          Some(None) => Some(Some(key_start)),
          twice => twice,
        };
      }
      self.trivia()?;
      if self.peek() != Some(b':') {
        return Err(self.error(Message::ExpectedColon));
      }
      self.at += 1;
      self.trivia()?;
      let value = self.value()?;
      self.build.property(key, value, span(key_start, self.at));
      self.trivia()?;
      match self.peek() {
        Some(b',') => self.at += 1,
        Some(b'}') => break,
        Some(_) => return Err(self.error(Message::UnexpectedToken)),
        None => return Err(self.error(Message::UnexpectedEnd)),
      }
    }
    self.at += 1;
    if let Some(Some(at)) = proto {
      return Err(self.error_at(Message::DuplicateProto, at));
    }
    Ok(self.build.object(open, span(start, self.at)))
  }

  fn array(&mut self) -> Result<B::Node, Abort> {
    let start = self.at;
    self.at += 1;
    let open = self.build.open(false);
    loop {
      self.trivia()?;
      match self.peek() {
        Some(b']') => break,
        Some(b',') => {
          if !self.build.hole() {
            return Err(self.error_at(Message::ArrayHole, start));
          }
          self.at += 1;
          continue;
        }
        _ => {}
      }
      let value = self.value()?;
      self.build.element(value);
      self.trivia()?;
      match self.peek() {
        Some(b',') => self.at += 1,
        Some(b']') => break,
        Some(_) => return Err(self.error(Message::UnexpectedToken)),
        None => return Err(self.error(Message::UnexpectedEnd)),
      }
    }
    self.at += 1;
    Ok(self.build.array(open, span(start, self.at)))
  }

  /// The key from `start` to `end` is `__proto__`. Only a key with an escape needs reading to tell.
  fn is_proto(&self, start: usize, end: usize) -> bool {
    let raw = &self.text[start..end];
    let (content, string) = match raw[0] {
      b'"' | b'\'' => (&raw[1..raw.len() - 1], true),
      b'0'..=b'9' | b'.' => return false,
      _ => (raw, false),
    };
    match (content.contains(&b'\\'), string) {
      (false, _) => content == b"__proto__",
      (true, false) => cook::identifier(content) == "__proto__",
      (true, true) => char::decode_utf16(cook::string(content))
        .map(|it| it.ok())
        .eq("__proto__".chars().map(Some)),
    }
  }

  fn key(&mut self) -> Result<B::Key, Abort> {
    let start = self.at;
    let kind = match self.peek() {
      Some(b'"' | b'\'') => {
        self.string_end()?;
        Kind::String
      }
      Some(b'0'..=b'9') => {
        self.number_end()?;
        Kind::Number
      }
      Some(b'.') if self.text.get(start + 1).is_some_and(u8::is_ascii_digit) => {
        self.number_end()?;
        Kind::Number
      }
      Some(_) if self.identifier_length(start).is_some() => {
        self.identifier()?;
        Kind::Identifier
      }
      Some(_) => return Err(self.error(Message::UnexpectedToken)),
      None => return Err(self.error(Message::UnexpectedEnd)),
    };
    Ok(self.build.key(kind, span(start, self.at)))
  }

  fn sign(&mut self) -> Result<B::Node, Abort> {
    let start = self.at;
    let minus = self.text[start] == b'-';
    self.at += 1;
    if self.peek() == self.text.get(start).copied() {
      return Err(self.error_at(Message::NotJson, start));
    }
    self.trivia()?;
    let at = self.at;
    let argument = match self.peek() {
      Some(b'0'..=b'9') => self.number()?,
      Some(b'.') if self.text.get(at + 1).is_some_and(u8::is_ascii_digit) => self.number()?,
      Some(_) if self.identifier_length(at).is_some() => {
        let end = self.identifier()?;
        let raw = &self.text[at..end];
        if !matches!(&*cook::identifier(raw), "Infinity" | "NaN") {
          return Err(self.error_at(Message::NotJson, at));
        }
        self.build.leaf(Kind::Identifier, span(at, end))
      }
      Some(_) => return Err(self.error_at(Message::NotJson, at)),
      None => return Err(self.error(Message::UnexpectedEnd)),
    };
    Ok(self.build.sign(minus, argument, span(start, self.at)))
  }

  fn string(&mut self) -> Result<B::Node, Abort> {
    let start = self.at;
    self.string_end()?;
    Ok(self.build.leaf(Kind::String, span(start, self.at)))
  }

  /// Reads a string, from its opening quote to after its closing one.
  fn string_end(&mut self) -> Result<(), Abort> {
    let start = self.at;
    let quote = self.text[start];
    self.at += 1;
    loop {
      let rest = &self.text[self.at..];
      let Some(length) = memchr::memchr3(quote, b'\\', b'\n', rest) else {
        return Err(self.error_at(Message::UnclosedString, start));
      };
      if memchr::memchr(b'\r', &rest[..length]).is_some() {
        return Err(self.error_at(Message::UnclosedString, start));
      }
      self.at += length;
      match rest[length] {
        b'\\' => self.escape(false)?,
        b'\n' => return Err(self.error_at(Message::UnclosedString, start)),
        _ => break,
      }
    }
    self.at += 1;
    Ok(())
  }

  fn template(&mut self) -> Result<B::Node, Abort> {
    let start = self.at;
    self.at += 1;
    loop {
      match self.text.get(self.at..).unwrap_or_default() {
        [] => return Err(self.error_at(Message::UnclosedString, start)),
        [b'`', ..] => break,
        [b'$', b'{', ..] => return Err(self.error(Message::NotJson)),
        [b'\\', ..] => self.escape(true)?,
        _ => self.at += 1,
      }
    }
    self.at += 1;
    Ok(self.build.leaf(Kind::Template, span(start, self.at)))
  }

  /// Babel's `readEscapedChar`, from the backslash. A template can't have octal escapes.
  fn escape(&mut self, template: bool) -> Result<(), Abort> {
    let start = self.at;
    self.at += 1;
    let Some(&byte) = self.text.get(self.at) else {
      return Err(self.error(Message::UnexpectedEnd));
    };
    match byte {
      b'x' => {
        self.at += 1;
        if !self.hex_digits(2) {
          return Err(self.error_at(Message::InvalidEscape, start));
        }
      }
      b'u' => {
        self.at += 1;
        if !self.code_point() {
          return Err(self.error_at(Message::InvalidEscape, start));
        }
      }
      b'0' if template && self.text.get(self.at + 1).is_some_and(u8::is_ascii_digit) => {
        return Err(self.error_at(Message::InvalidEscape, start));
      }
      b'1'..=b'9' if template => return Err(self.error_at(Message::InvalidEscape, start)),
      _ => self.at += utf8_length(byte),
    }
    Ok(())
  }

  /// After `\u`: four hex digits, or hex digits in braces up to U+10FFFF.
  fn code_point(&mut self) -> bool {
    if self.peek() != Some(b'{') {
      return self.hex_digits(4);
    }
    self.at += 1;
    let digits = self.text[self.at..].iter().take_while(|byte| byte.is_ascii_hexdigit()).count();
    let value = std::str::from_utf8(&self.text[self.at..self.at + digits])
      .ok()
      .and_then(|digits| u32::from_str_radix(digits, 16).ok());
    self.at += digits;
    if digits == 0 || self.peek() != Some(b'}') || !value.is_some_and(|value| value <= 0x10FFFF) {
      return false;
    }
    self.at += 1;
    true
  }

  fn hex_digits(&mut self, count: usize) -> bool {
    let digits = self.text.get(self.at..self.at + count);
    if !digits.is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit)) {
      return false;
    }
    self.at += count;
    true
  }

  fn number(&mut self) -> Result<B::Node, Abort> {
    let start = self.at;
    self.number_end()?;
    Ok(self.build.leaf(Kind::Number, span(start, self.at)))
  }

  /// Babel's `readNumber` and `readRadixNumber`. A BigInt isn't JSON.
  fn number_end(&mut self) -> Result<(), Abort> {
    let start = self.at;
    let text = self.text;
    let radix = match text.get(start..start + 2) {
      Some([b'0', b'x' | b'X']) => 16,
      Some([b'0', b'o' | b'O']) => 8,
      Some([b'0', b'b' | b'B']) => 2,
      _ => 10,
    };
    if radix != 10 {
      self.at += 2;
      if !self.digits(radix)? {
        return Err(self.error_at(Message::InvalidNumber, start));
      }
    } else {
      let dot = text[start] == b'.';
      if !dot {
        self.digits(10)?;
      }
      let integer = &text[start..self.at];
      let legacy = integer.len() >= 2 && integer[0] == b'0';
      if legacy && integer.contains(&b'_') {
        return Err(self.error_at(Message::InvalidNumber, start));
      }
      let octal = legacy && !integer.iter().any(|byte| matches!(byte, b'8' | b'9'));
      if !octal && self.peek() == Some(b'.') {
        self.at += 1;
        self.digits(10)?;
      }
      if !octal && matches!(self.peek(), Some(b'e' | b'E')) {
        self.at += 1;
        if matches!(self.peek(), Some(b'+' | b'-')) {
          self.at += 1;
        }
        if !self.digits(10)? {
          return Err(self.error_at(Message::InvalidNumber, start));
        }
      }
    }
    if self.peek() == Some(b'n') || self.identifier_length(self.at).is_some() {
      return Err(self.error_at(Message::InvalidNumber, start));
    }
    Ok(())
  }

  /// Babel's `readInt`: digits of `radix`, with single `_` between them. False if there are none.
  fn digits(&mut self, radix: u32) -> Result<bool, Abort> {
    let start = self.at;
    while let Some(&byte) = self.text.get(self.at) {
      if byte == b'_' {
        let before = self.at > start && is_digit(self.text[self.at - 1], radix);
        let after = self.text.get(self.at + 1).is_some_and(|byte| is_digit(*byte, radix));
        if !before || !after {
          return Err(self.error(Message::InvalidNumber));
        }
      } else if !is_digit(byte, radix) {
        break;
      }
      self.at += 1;
    }
    Ok(self.at > start)
  }

  /// An identifier name, which may have `\u` escapes. Returns where it ends.
  fn identifier(&mut self) -> Result<usize, Abort> {
    let start = self.at;
    let mut first = true;
    loop {
      let at = self.at;
      if self.text.get(at) == Some(&b'\\') {
        let escaped = self.text.get(at + 1) == Some(&b'u') && {
          self.at += 2;
          self.code_point()
        };
        let character = cook::identifier(&self.text[at..self.at]).chars().next();
        let valid = character.is_some_and(|it| if first { is_start(it) } else { is_part(it) });
        if !escaped || !valid {
          return Err(self.error_at(Message::InvalidEscape, at));
        }
      } else {
        match self.identifier_character(at, first) {
          Some(length) => self.at += length,
          None => break,
        }
      }
      first = false;
    }
    debug_assert!(self.at > start);
    Ok(self.at)
  }

  /// The length of the identifier that starts at `at`, if one does: of its first character only.
  fn identifier_length(&self, at: usize) -> Option<usize> {
    match self.text.get(at) {
      Some(b'\\') => Some(1),
      _ => self.identifier_character(at, true),
    }
  }

  fn identifier_character(&self, at: usize, first: bool) -> Option<usize> {
    let byte = *self.text.get(at)?;
    if byte.is_ascii() {
      let valid = byte.is_ascii_alphabetic()
        || matches!(byte, b'$' | b'_')
        || (!first && byte.is_ascii_digit());
      return valid.then_some(1);
    }
    let character = std::str::from_utf8(&self.text[at..(at + 4).min(self.text.len())])
      .or_else(|error| std::str::from_utf8(&self.text[at..at + error.valid_up_to()]))
      .ok()?
      .chars()
      .next()?;
    let valid = if first { is_start(character) } else { is_part(character) };
    valid.then_some(character.len_utf8())
  }

  /// Whitespace and comments, which it records.
  fn trivia(&mut self) -> Result<(), Abort> {
    loop {
      let rest = &self.text[self.at..];
      match rest {
        [b'/', b'/', ..] => {
          let end = line_end(&rest[2..]) + 2;
          self.comment(end, false);
        }
        [b'/', b'*', ..] => match strings::index_of(&rest[2..], b"*/") {
          Some(end) => self.comment(end + 4, true),
          None => return Err(self.error(Message::UnclosedComment)),
        },
        [b' ' | b'\t' | b'\n' | b'\r', ..] => {
          let length = rest.iter().position(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r'));
          self.at += length.unwrap_or(rest.len());
        }
        [] | [0x0E..=0x1F | 0x21..=0x7F, ..] => return Ok(()),
        _ => match strings::js_whitespace_len(rest) {
          0 => return Ok(()),
          length => self.at += length,
        },
      }
    }
  }

  fn comment(&mut self, length: usize, block: bool) {
    let span = span(self.at, self.at + length);
    self.build.comment(Comment { span, block });
    self.at += length;
  }

  fn peek(&self) -> Option<u8> {
    self.text.get(self.at).copied()
  }

  fn error(&self, message: Message) -> Abort {
    self.error_at(message, self.at)
  }

  fn error_at(&self, message: Message, at: usize) -> Abort {
    Abort::Syntax(self.rejection.note(message, at as u32))
  }
}

fn span(start: usize, end: usize) -> Span {
  Span { start: start as u32, end: end as u32 }
}

/// Where the line ends: at `\n`, `\r`, U+2028 or U+2029.
fn line_end(text: &[u8]) -> usize {
  let mut at = 0;
  while at < text.len() {
    if strings::js_line_break_len(&text[at..]) > 0 {
      return at;
    }
    at += 1;
  }
  at
}

fn utf8_length(byte: u8) -> usize {
  match byte {
    0xF0.. => 4,
    0xE0.. => 3,
    0xC0.. => 2,
    _ => 1,
  }
}

fn is_digit(byte: u8, radix: u32) -> bool {
  (byte as char).is_digit(radix)
}

fn is_start(character: char) -> bool {
  matches!(character, '$' | '_') || unicode_ident::is_xid_start(character)
}

fn is_part(character: char) -> bool {
  matches!(character, '$' | '\u{200C}' | '\u{200D}') || unicode_ident::is_xid_continue(character)
}
