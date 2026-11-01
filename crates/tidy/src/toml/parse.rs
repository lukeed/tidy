//! taplo's parser (`parser/mod.rs`), which builds the tree with the same nodes, in the same places, as rowan does. taplo
//! keeps going after a syntax error, but `taplo fmt` won't format a document with one, so this stops at the first.

use super::Abort;
use super::lexer::{self, Token};
use super::tree::{Element, Id, Kind, NONE, Tree};
use crate::stack::Stack;
use crate::syntax::{Message, Rejection};

/// Why a step of the parser failed: taplo's `Err(())`, with or without a syntax error.
enum Fail {
  /// taplo returns `Err(())` without an error, and the caller may go on.
  Silent,
  Abort(Abort),
}

type Step = Result<(), Fail>;

pub(crate) struct Parser<'p> {
  pub(crate) text: &'p [u8],
  pub(crate) tree: &'p mut Tree,
  /// The children of the nodes that are open, innermost last.
  pub(crate) pending: &'p mut Vec<Id>,
  /// The nodes that are open: their kind, and where their children start in `pending`.
  pub(crate) open: &'p mut Vec<(Kind, usize)>,
  pub(crate) rejection: &'p Rejection,
  pub(crate) stack: &'p Stack,
  /// Where the current token starts and ends: logos's `span`.
  pub(crate) span: (usize, usize),
  pub(crate) current: Option<Token>,
  pub(crate) skip_whitespace: bool,
}

impl Parser<'_> {
  pub(crate) fn document(&mut self) -> Result<(), Abort> {
    self.start(Kind::Root);
    let result = self.root();
    self.finish();
    match result {
      Ok(()) | Err(Fail::Silent) => Ok(()),
      Err(Fail::Abort(abort)) => Err(abort),
    }
  }

  // ───────────────────────────── the builder ─────────────────────────────

  fn start(&mut self, kind: Kind) {
    self.open.push((kind, self.pending.len()));
  }

  fn finish(&mut self) {
    let (kind, first) = self.open.pop().expect("an open node");
    let id = self.tree.elements.len() as Id;
    let start = self.tree.children.len() as u32;
    let (mut from, mut to) = (self.span.0 as u32, self.span.0 as u32);
    for (index, child) in self.pending.drain(first..).enumerate() {
      let element = &mut self.tree.elements[child as usize];
      element.parent = id;
      element.index = index as u32;
      if index == 0 {
        from = element.start;
      }
      to = element.end;
      self.tree.children.push(child);
    }
    let children = (start, self.tree.children.len() as u32);
    self.tree.elements.push(Element {
      kind,
      start: from,
      end: to,
      parent: NONE,
      index: 0,
      children,
    });
    self.pending.push(id);
  }

  fn insert(&mut self, token: Token, (start, end): (usize, usize)) {
    let id = self.tree.elements.len() as Id;
    let (start, end) = (start as u32, end as u32);
    let kind = Kind::Token(token);
    self.tree.elements.push(Element { kind, start, end, parent: NONE, index: 0, children: (0, 0) });
    self.pending.push(id);
  }

  /// Runs `parse` inside a node of `kind`: taplo's `with_node!`.
  fn node(&mut self, kind: Kind, parse: impl FnOnce(&mut Self) -> Step) -> Step {
    self.start(kind);
    let result = parse(self);
    self.finish();
    result
  }

  // ───────────────────────────── tokens ─────────────────────────────

  fn error(&self, message: Message) -> Fail {
    Fail::Abort(Abort::Syntax(self.rejection.note(message, self.span.0 as u32)))
  }

  /// taplo's `let _ = ...`: going on after `Err(())`, unless there was a syntax error.
  fn ignore(result: Step) -> Step {
    match result {
      Err(Fail::Silent) => Ok(()),
      result => result,
    }
  }

  fn slice(&self) -> &[u8] {
    &self.text[self.span.0..self.span.1]
  }

  fn remainder(&self) -> &[u8] {
    &self.text[self.span.1..]
  }

  /// `step`: lexes up to the next token that isn't whitespace or a comment, and adds those to the open node.
  fn step(&mut self) -> Step {
    self.current = None;
    while self.span.1 < self.text.len() {
      let start = self.span.1;
      let (token, length) = lexer::next(&self.text[start..]);
      self.span = (start, start + length);
      match token {
        Token::Comment if !is_valid_comment(self.slice()) => {
          return Err(self.error(Message::InvalidCharacter));
        }
        Token::Comment => self.insert(token, self.span),
        Token::Whitespace if self.skip_whitespace => self.insert(token, self.span),
        Token::Error => return Err(self.error(Message::UnexpectedToken)),
        _ => {
          self.current = Some(token);
          break;
        }
      }
    }
    Ok(())
  }

  fn token(&mut self) -> Result<Token, Fail> {
    if self.current.is_none() {
      self.step()?;
    }
    self.current.ok_or(Fail::Silent)
  }

  /// `token`: adds the current token, and steps.
  fn bump(&mut self) -> Step {
    let token = self.token()?;
    self.bump_as(token)
  }

  /// `token_as`
  fn bump_as(&mut self, token: Token) -> Step {
    self.token()?;
    self.insert(token, self.span);
    self.step()
  }

  /// `add_token`: adds the current token, without stepping.
  fn add(&mut self) -> Step {
    let token = self.token()?;
    self.insert(token, self.span);
    self.current = None;
    Ok(())
  }

  /// `must_token_or`
  fn expect(&mut self, token: Token, message: Message) -> Step {
    match self.token() {
      Ok(current) if current == token => self.bump(),
      Ok(_) => Err(self.error(message)),
      Err(Fail::Silent) => Err(self.error(Message::UnexpectedEnd)),
      Err(fail) => Err(fail),
    }
  }

  // ───────────────────────────── the grammar ─────────────────────────────

  fn root(&mut self) -> Step {
    let mut not_newline = false;
    let mut entry = false;
    loop {
      let token = match self.token() {
        Ok(token) => token,
        Err(Fail::Silent) => break,
        Err(fail) => return Err(fail),
      };
      match token {
        Token::BracketStart => {
          if entry {
            self.finish();
            entry = false;
          }
          if not_newline {
            return Err(self.error(Message::ExpectedLineBreak));
          }
          not_newline = true;
          let result = match self.remainder().starts_with(b"[") {
            true => self.node(Kind::TableArrayHeader, Self::table_array_header),
            false => self.node(Kind::TableHeader, Self::table_header),
          };
          Self::ignore(result)?;
        }
        Token::Newline => {
          not_newline = false;
          if entry {
            self.finish();
            entry = false;
          }
          Self::ignore(self.bump())?;
        }
        _ => {
          if not_newline {
            return Err(self.error(Message::ExpectedLineBreak));
          }
          if entry {
            self.finish();
          }
          not_newline = true;
          self.start(Kind::Entry);
          entry = true;
          Self::ignore(self.entry())?;
        }
      }
    }
    if entry {
      self.finish();
    }
    Ok(())
  }

  fn table_header(&mut self) -> Step {
    self.expect(Token::BracketStart, Message::ExpectedBracket)?;
    Self::ignore(self.node(Kind::Key, Self::key))?;
    self.expect(Token::BracketEnd, Message::ExpectedBracket)
  }

  fn table_array_header(&mut self) -> Step {
    self.skip_whitespace = false;
    self.expect(Token::BracketStart, Message::ExpectedBracket)?;
    self.expect(Token::BracketStart, Message::ExpectedBracket)?;
    self.skip_whitespace = true;
    Self::ignore(self.node(Kind::Key, Self::key))?;
    self.skip_whitespace = false;
    Self::ignore(self.expect(Token::BracketEnd, Message::ExpectedBracket))?;
    // taplo adds the second `]` without stepping after it.
    match self.token()? {
      Token::BracketEnd => {
        self.token()?;
        self.insert(Token::BracketEnd, self.span);
      }
      _ => return Err(self.error(Message::ExpectedBracket)),
    }
    self.skip_whitespace = true;
    self.step()
  }

  fn entry(&mut self) -> Step {
    self.node(Kind::Key, Self::key)?;
    self.expect(Token::Eq, Message::ExpectedEquals)?;
    self.node(Kind::Value, Self::value)
  }

  fn key(&mut self) -> Step {
    match self.ident() {
      Ok(()) => {}
      Err(Fail::Silent) => return Err(self.error(Message::ExpectedName)),
      Err(fail) => return Err(fail),
    }
    let mut after_period = false;
    loop {
      let token = match self.token() {
        Ok(token) => token,
        Err(Fail::Silent) if !after_period => return Ok(()),
        Err(Fail::Silent) => return Err(self.error(Message::UnexpectedEnd)),
        Err(fail) => return Err(fail),
      };
      match token {
        Token::Period if after_period => return Err(self.error(Message::UnexpectedToken)),
        Token::Period => {
          self.bump()?;
          after_period = true;
        }
        _ if after_period => {
          match self.ident() {
            Ok(()) => {}
            Err(Fail::Silent) => return Err(self.error(Message::ExpectedName)),
            Err(fail) => return Err(fail),
          }
          after_period = false;
        }
        _ => break,
      }
    }
    Ok(())
  }

  fn ident(&mut self) -> Step {
    let token = self.token()?;
    let slice = self.slice();
    match token {
      Token::Ident => self.bump(),
      Token::IntegerHex | Token::IntegerBin | Token::IntegerOct => self.bump_as(Token::Ident),
      Token::Integer if slice.starts_with(b"+") => Err(Fail::Silent),
      Token::Integer | Token::Bool => self.bump_as(Token::Ident),
      Token::StringLiteral => {
        self.check_literal(false)?;
        self.bump_as(Token::Ident)
      }
      Token::String => {
        self.check_string(false)?;
        self.bump_as(Token::Ident)
      }
      Token::Float if slice.starts_with(b"0") => Err(self.error(Message::InvalidNumber)),
      Token::Float if slice.starts_with(b"+") => Err(Fail::Silent),
      Token::Float => {
        let (start, end) = self.span;
        let mut at = start;
        for (index, part) in self.text[start..end].split(|byte| *byte == b'.').enumerate() {
          if index != 0 {
            self.insert(Token::Period, (at, at + 1));
            at += 1;
          }
          self.insert(Token::Ident, (at, at + part.len()));
          at += part.len();
        }
        self.step()
      }
      _ => Err(self.error(Message::ExpectedName)),
    }
  }

  fn value(&mut self) -> Step {
    let token = match self.token() {
      Ok(token) => token,
      Err(Fail::Silent) => return Err(self.error(Message::ExpectedValue)),
      Err(fail) => return Err(fail),
    };
    let slice = self.slice();
    match token {
      Token::Bool | Token::DateTimeOffset | Token::DateTimeLocal | Token::Date | Token::Time => {
        self.bump()
      }
      Token::Integer => {
        if is_zero_padded(slice) {
          return Err(self.error(Message::InvalidNumber));
        }
        match check_underscores(slice, 10) {
          true => self.bump(),
          false => Err(self.error(Message::InvalidNumber)),
        }
      }
      Token::IntegerBin | Token::IntegerHex | Token::IntegerOct => {
        let radix = match token {
          Token::IntegerBin => 2,
          Token::IntegerOct => 8,
          _ => 16,
        };
        match check_underscores(slice, radix) {
          true => self.bump(),
          false => Err(self.error(Message::InvalidNumber)),
        }
      }
      Token::Float => {
        let integer = match slice.contains(&b'.') {
          true => slice.split(|byte| *byte == b'.').next(),
          false => slice.split(|byte| *byte == b'e').next(),
        };
        if integer.is_some_and(is_zero_padded) || !check_underscores(slice, 10) {
          return Err(self.error(Message::InvalidNumber));
        }
        self.bump()
      }
      Token::StringLiteral => {
        self.check_literal(false)?;
        self.bump()
      }
      Token::MultiLineStringLiteral => {
        self.check_literal(true)?;
        self.bump()
      }
      Token::String => {
        self.check_string(false)?;
        self.bump()
      }
      Token::MultiLineString => {
        self.check_string(true)?;
        self.bump()
      }
      Token::BracketStart => {
        if !self.stack.has_room() {
          return Err(Fail::Abort(Abort::TooDeep));
        }
        self.node(Kind::Array, Self::array)
      }
      Token::BraceStart => {
        if !self.stack.has_room() {
          return Err(Fail::Abort(Abort::TooDeep));
        }
        self.node(Kind::InlineTable, Self::inline_table)
      }
      _ => Err(self.error(Message::ExpectedValue)),
    }
  }

  fn inline_table(&mut self) -> Step {
    self.expect(Token::BraceStart, Message::ExpectedBrace)?;
    let mut first = true;
    let mut comma_last = false;
    loop {
      let token = match self.token() {
        Ok(token) => token,
        Err(Fail::Silent) => return Err(self.error(Message::ExpectedBrace)),
        Err(fail) => return Err(fail),
      };
      match token {
        Token::BraceEnd if comma_last => return Err(self.error(Message::UnexpectedToken)),
        Token::BraceEnd => return self.add(),
        Token::Newline => return Err(self.error(Message::LineBreakInInlineTable)),
        Token::Comma if comma_last || first => return Err(self.error(Message::UnexpectedToken)),
        Token::Comma => {
          self.bump()?;
          comma_last = true;
        }
        _ => {
          if !comma_last && !first {
            return Err(self.error(Message::ExpectedComma));
          }
          Self::ignore(self.node(Kind::Entry, Self::entry))?;
          comma_last = false;
        }
      }
      first = false;
    }
  }

  fn array(&mut self) -> Step {
    self.expect(Token::BracketStart, Message::ExpectedBracket)?;
    let mut first = true;
    let mut comma_last = false;
    loop {
      let token = match self.token() {
        Ok(token) => token,
        Err(Fail::Silent) => return Err(self.error(Message::UnexpectedEnd)),
        Err(fail) => return Err(fail),
      };
      match token {
        Token::BracketEnd => return self.add(),
        Token::Newline => {
          self.bump()?;
          continue;
        }
        Token::Comma => {
          if first || comma_last {
            return Err(self.error(Message::UnexpectedToken));
          }
          self.bump()?;
          comma_last = true;
        }
        _ => {
          if !comma_last && !first {
            return Err(self.error(Message::ExpectedComma));
          }
          Self::ignore(self.node(Kind::Value, Self::value))?;
          comma_last = false;
        }
      }
      first = false;
    }
  }

  // ───────────────────────────── checks ─────────────────────────────

  /// `allowed_chars::string_literal` and `multi_line_string_literal`: no control characters but tabs, and line breaks
  /// in a multi-line one.
  fn check_literal(&self, multi_line: bool) -> Step {
    let text = String::from_utf8_lossy(self.slice());
    let invalid = text.chars().any(|character| {
      character != '\t'
        && !(multi_line && matches!(character, '\n' | '\r'))
        && character.is_control()
    });
    match invalid {
      true => Err(self.error(Message::InvalidCharacter)),
      false => Ok(()),
    }
  }

  /// `allowed_chars::string` and `multi_line_string`, and `check_escape`.
  fn check_string(&self, multi_line: bool) -> Step {
    let slice = self.slice();
    let invalid = slice.iter().any(|byte| {
      let byte = *byte;
      byte != b'\t'
        && !(multi_line && matches!(byte, b'\n' | b'\r'))
        && (byte <= 0x08 || (0x0A..=0x1F).contains(&byte) || byte == 0x7F)
    });
    if invalid {
      return Err(self.error(Message::InvalidCharacter));
    }
    match check_escape(slice) {
      true => Ok(()),
      false => Err(self.error(Message::InvalidEscape)),
    }
  }
}

/// `allowed_chars::comment`: no control characters but tabs.
fn is_valid_comment(text: &[u8]) -> bool {
  let ascii = |byte: &u8| *byte == b'\t' || !byte.is_ascii_control();
  match text.is_ascii() {
    true => text.iter().all(ascii),
    false => !String::from_utf8_lossy(text).chars().any(|it| it != '\t' && it.is_control()),
  }
}

/// A number that starts with a zero, which isn't zero itself.
fn is_zero_padded(text: &[u8]) -> bool {
  let digits = text.strip_prefix(b"+").or_else(|| text.strip_prefix(b"-")).unwrap_or(text);
  digits.starts_with(b"0") && digits != b"0"
}

/// `check_underscores`: each `_` is between two digits of `radix`.
fn check_underscores(text: &[u8], radix: u32) -> bool {
  if text.starts_with(b"_") || text.ends_with(b"_") {
    return false;
  }
  let mut last = 0u8;
  for &byte in text {
    let digit = |byte: u8| (byte as char).is_digit(radix);
    if (byte == b'_' && !digit(last)) || (!digit(byte) && last == b'_') {
      return false;
    }
    last = byte;
  }
  true
}

/// `check_escape`: each backslash starts an escape TOML 1.0 has.
fn check_escape(text: &[u8]) -> bool {
  let mut at = 0;
  while at < text.len() {
    if text[at] != b'\\' {
      at += 1;
      continue;
    }
    let rest = &text[at + 1..];
    let length = match rest.first() {
      Some(b'b' | b't' | b'n' | b'f' | b'r' | b'"' | b'\\') => 1,
      Some(b'u') => code_point(rest, 4),
      Some(b'U') => code_point(rest, 8),
      _ => match line_continuation(rest) {
        Some(length) => length,
        None => return false,
      },
    };
    if length == 0 {
      return false;
    }
    at += 1 + length;
  }
  true
}

/// After the backslash, `u` or `U` and `count` hex digits of a Unicode scalar value. 0 if not.
fn code_point(text: &[u8], count: usize) -> usize {
  let Some(digits) = text.get(1..1 + count) else { return 0 };
  if !digits.iter().all(|byte| byte.is_ascii_hexdigit() || *byte == b'_') {
    return 0;
  }
  let value =
    std::str::from_utf8(digits).ok().and_then(|digits| u32::from_str_radix(digits, 16).ok());
  match value.and_then(char::from_u32) {
    Some(_) => 1 + count,
    None => 0,
  }
}

/// After the backslash, whitespace up to a line break: `\\\s*\n` or `\\\s*\r\n`.
fn line_continuation(text: &[u8]) -> Option<usize> {
  let text = std::str::from_utf8(text).ok()?;
  let mut length = 0;
  for character in text.chars() {
    if character == '\n' {
      return Some(length + 1);
    }
    if character == '\r' && text[length + 1..].starts_with('\n') {
      return Some(length + 2);
    }
    if !character.is_whitespace() {
      return None;
    }
    length += character.len_utf8();
  }
  None
}
