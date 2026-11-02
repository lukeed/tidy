//! A port of the lexer and parser of graphql-js 17, which Prettier uses, with `experimentalFragmentArguments` enabled.
//! Ported from bun's `src/format/graphql/parser.rs` (MIT).
//!
//! The tree is flat. A node stores its kind, its span, and its children in source order, which are all the nodes that
//! graphql-js puts inside it. A child's role in its parent follows from its kind and its position among its siblings.

use crate::stack::Stack;
use crate::syntax::{Message, Rejected, Rejection, SyntaxError};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
  Name,
  Document,
  OperationDefinition,
  VariableDefinition,
  Variable,
  SelectionSet,
  Field,
  Argument,
  FragmentArgument,
  FragmentSpread,
  InlineFragment,
  FragmentDefinition,
  IntValue,
  FloatValue,
  StringValue,
  BooleanValue,
  NullValue,
  EnumValue,
  ListValue,
  ObjectValue,
  ObjectField,
  Directive,
  NamedType,
  ListType,
  NonNullType,
  SchemaDefinition,
  OperationTypeDefinition,
  ScalarTypeDefinition,
  ObjectTypeDefinition,
  FieldDefinition,
  InputValueDefinition,
  InterfaceTypeDefinition,
  UnionTypeDefinition,
  EnumTypeDefinition,
  EnumValueDefinition,
  InputObjectTypeDefinition,
  DirectiveDefinition,
  SchemaExtension,
  DirectiveExtension,
  ScalarTypeExtension,
  ObjectTypeExtension,
  InterfaceTypeExtension,
  UnionTypeExtension,
  EnumTypeExtension,
  InputObjectTypeExtension,
}

impl Kind {
  pub(crate) fn is_type(self) -> bool {
    matches!(self, Kind::NamedType | Kind::ListType | Kind::NonNullType)
  }

  pub(crate) fn is_value(self) -> bool {
    matches!(
      self,
      Kind::Variable
        | Kind::IntValue
        | Kind::FloatValue
        | Kind::StringValue
        | Kind::BooleanValue
        | Kind::NullValue
        | Kind::EnumValue
        | Kind::ListValue
        | Kind::ObjectValue
    )
  }
}

/// A byte range of the text.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Span {
  pub(crate) start: u32,
  pub(crate) end: u32,
}

impl Span {
  pub(crate) fn new(start: u32, end: u32) -> Span {
    Span { start, end }
  }
}

pub(crate) type NodeId = u32;

#[derive(Copy, Clone, Debug)]
pub(crate) struct Node {
  pub(crate) kind: Kind,
  pub(crate) flags: u8,
  pub(crate) span: Span,
  first_child: u32,
  child_count: u32,
}

/// `Field`: its first child is the alias.
pub(crate) const HAS_ALIAS: u8 = 1;
/// `StringValue`: `""" .. """`
pub(crate) const IS_BLOCK: u8 = 1;
/// `DirectiveDefinition`
pub(crate) const IS_REPEATABLE: u8 = 1;
/// `OperationDefinition` and `OperationTypeDefinition`
pub(crate) const QUERY: u8 = 0;
pub(crate) const MUTATION: u8 = 1;
pub(crate) const SUBSCRIPTION: u8 = 2;
/// `OperationDefinition`: it's only a selection set.
pub(crate) const IS_SHORTHAND: u8 = 3;

#[derive(Default)]
pub(crate) struct Tree {
  pub(crate) nodes: Vec<Node>,
  children: Vec<NodeId>,
  /// From the `#` to the end of the line.
  pub(crate) comments: Vec<Span>,
  /// Finished nodes whose parent isn't finished yet.
  pending: Vec<NodeId>,
}

impl Tree {
  /// The `Document`.
  pub(crate) fn root(&self) -> NodeId {
    (self.nodes.len() as u32).saturating_sub(1)
  }

  pub(crate) fn node(&self, id: NodeId) -> Option<&Node> {
    self.nodes.get(id as usize)
  }

  pub(crate) fn children(&self, id: NodeId) -> &[NodeId] {
    let Some(node) = self.node(id) else { return &[] };
    let start = node.first_child as usize;
    self.children.get(start..start + node.child_count as usize).unwrap_or_default()
  }

  pub(crate) fn kind(&self, id: NodeId) -> Kind {
    self.node(id).map_or(Kind::Name, |node| node.kind)
  }

  pub(crate) fn span(&self, id: NodeId) -> Span {
    self.node(id).map_or_else(Span::default, |node| node.span)
  }
}

type Result<T> = std::result::Result<T, Rejected>;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum TokenKind {
  StartOfFile,
  EndOfFile,
  Bang,
  Dollar,
  Amp,
  ParenL,
  ParenR,
  Spread,
  Colon,
  Equals,
  At,
  BracketL,
  BracketR,
  BraceL,
  Pipe,
  BraceR,
  Name,
  Int,
  Float,
  String,
  BlockString,
}

#[derive(Copy, Clone, Debug)]
struct Token {
  kind: TokenKind,
  start: u32,
  end: u32,
}

const fn is_name_start(byte: u8) -> bool {
  byte.is_ascii_alphabetic() || byte == b'_'
}

const fn is_name_continue(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The value of `\uXXXX`'s four hex digits.
fn hex4(text: &[u8]) -> Option<u32> {
  let digits = text.get(..4)?;
  digits.iter().try_fold(0, |code, &digit| Some(code << 4 | (digit as char).to_digit(16)?))
}

/// Decodes the escape sequence at the start of `text`. Returns the character and the sequence's length.
pub(crate) fn read_escape(text: &[u8]) -> Option<(char, usize)> {
  let simple = |c: char| Some((c, 2));
  match *text.get(1)? {
    b'"' => simple('"'),
    b'\\' => simple('\\'),
    b'/' => simple('/'),
    b'b' => simple('\u{8}'),
    b'f' => simple('\u{c}'),
    b'n' => simple('\n'),
    b'r' => simple('\r'),
    b't' => simple('\t'),
    // `\u{1F600}`
    b'u' if text.get(2) == Some(&b'{') => {
      let mut code: u32 = 0;
      for size in 3..12 {
        if text.get(size) == Some(&b'}') {
          return (size >= 4).then(|| char::from_u32(code)).flatten().map(|c| (c, size + 1));
        }
        code = code.checked_shl(4)? | (*text.get(size)? as char).to_digit(16)?;
      }
      None
    }
    b'u' => {
      let code = hex4(text.get(2..)?)?;
      if let Some(c) = char::from_u32(code) {
        return Some((c, 6));
      }
      // A surrogate pair written as two escapes.
      if !(0xD800..=0xDBFF).contains(&code) || text.get(6..8) != Some(b"\\u") {
        return None;
      }
      let trailing = hex4(text.get(8..)?)?;
      if !(0xDC00..=0xDFFF).contains(&trailing) {
        return None;
      }
      char::from_u32(0x10000 + ((code - 0xD800) << 10) + (trailing - 0xDC00)).map(|c| (c, 12))
    }
    _ => None,
  }
}

struct Parser<'t> {
  text: &'t [u8],
  tree: &'t mut Tree,
  rejection: Rejection,
  token: Token,
  /// The token after `token`, if it has been read already.
  next: Option<Token>,
  /// The end of the token before `token`.
  last_end: u32,
  stack: &'t Stack,
}

/// Parses `text` into `tree`. If the stack runs out, `stack` says so, and the error is any error.
pub(crate) fn parse(
  text: &[u8],
  tree: &mut Tree,
  stack: &Stack,
) -> std::result::Result<(), SyntaxError> {
  tree.nodes.clear();
  tree.children.clear();
  tree.comments.clear();
  tree.pending.clear();
  if u32::try_from(text.len()).is_err() {
    return Err(SyntaxError(Message::TooLarge, 0));
  }
  let start = Token { kind: TokenKind::StartOfFile, start: 0, end: 0 };
  let rejection = Rejection::default();
  let mut parser = Parser { text, tree, rejection, token: start, next: None, last_end: 0, stack };
  parser.document().map_err(|_| parser.rejection.reason())
}

/// The error for a missing token of `kind`.
#[cold]
fn expected(kind: TokenKind) -> Message {
  match kind {
    TokenKind::Name => Message::ExpectedName,
    TokenKind::Colon => Message::ExpectedColon,
    TokenKind::BraceL => Message::ExpectedOpeningBrace,
    TokenKind::BracketR => Message::ExpectedClosingBracket,
    _ => Message::UnexpectedToken,
  }
}

impl Parser<'_> {
  // ───────────────────────────── the lexer ─────────────────────────────

  fn byte(&self, position: usize) -> Option<u8> {
    self.text.get(position).copied()
  }

  /// Reads the token at or after `start`, and records the comments it skips.
  fn read_token(&mut self, start: u32) -> Result<Token> {
    let mut position = start as usize;
    let token =
      |kind, start: usize, end: usize| Ok(Token { kind, start: start as u32, end: end as u32 });
    loop {
      let Some(byte) = self.byte(position) else {
        return token(TokenKind::EndOfFile, self.text.len(), self.text.len());
      };
      let punctuator = match byte {
        b'\t' | b' ' | b',' | b'\n' | b'\r' => {
          position += 1;
          continue;
        }
        0xEF if self.text[position..].starts_with(b"\xEF\xBB\xBF") => {
          position += 3;
          continue;
        }
        b'#' => {
          let rest = &self.text[position..];
          let len = crate::strings::index_of_any(rest, b"\n\r").unwrap_or(rest.len());
          self.tree.comments.push(Span::new(position as u32, (position + len) as u32));
          position += len;
          continue;
        }
        b'!' => TokenKind::Bang,
        b'$' => TokenKind::Dollar,
        b'&' => TokenKind::Amp,
        b'(' => TokenKind::ParenL,
        b')' => TokenKind::ParenR,
        b':' => TokenKind::Colon,
        b'=' => TokenKind::Equals,
        b'@' => TokenKind::At,
        b'[' => TokenKind::BracketL,
        b']' => TokenKind::BracketR,
        b'{' => TokenKind::BraceL,
        b'|' => TokenKind::Pipe,
        b'}' => TokenKind::BraceR,
        b'.' if self.text[position..].starts_with(b"...") => {
          return token(TokenKind::Spread, position, position + 3);
        }
        b'"' if self.text[position..].starts_with(b"\"\"\"") => {
          return token(TokenKind::BlockString, position, self.read_block_string(position)?);
        }
        b'"' => return token(TokenKind::String, position, self.read_string(position)?),
        b'0'..=b'9' | b'-' => {
          let (kind, end) = self.read_number(position)?;
          return token(kind, position, end);
        }
        _ if is_name_start(byte) => {
          let rest = &self.text[position..];
          let len = rest.iter().take_while(|&&byte| is_name_continue(byte)).count();
          return token(TokenKind::Name, position, position + len);
        }
        _ => return Err(self.rejection.note(Message::UnexpectedCharacter, position as u32)),
      };
      return token(punctuator, position, position + 1);
    }
  }

  /// Returns the end of the digits at `position`. Fails if there are none.
  fn read_digits(&self, position: usize) -> Result<usize> {
    let rest = self.text.get(position..).unwrap_or_default();
    match rest.iter().take_while(|byte| byte.is_ascii_digit()).count() {
      0 => Err(self.rejection.note(Message::ExpectedDigit, position as u32)),
      count => Ok(position + count),
    }
  }

  fn read_number(&self, start: usize) -> Result<(TokenKind, usize)> {
    let mut position = start;
    let mut kind = TokenKind::Int;
    if self.byte(position) == Some(b'-') {
      position += 1;
    }
    if self.byte(position) == Some(b'0') {
      position += 1;
      if self.byte(position).is_some_and(|byte| byte.is_ascii_digit()) {
        return Err(self.rejection.note(Message::InvalidNumber, start as u32));
      }
    } else {
      position = self.read_digits(position)?;
    }
    if self.byte(position) == Some(b'.') {
      kind = TokenKind::Float;
      position = self.read_digits(position + 1)?;
    }
    if matches!(self.byte(position), Some(b'E' | b'e')) {
      kind = TokenKind::Float;
      position += 1;
      if matches!(self.byte(position), Some(b'+' | b'-')) {
        position += 1;
      }
      position = self.read_digits(position)?;
    }
    match self.byte(position) {
      Some(byte) if byte == b'.' || is_name_start(byte) => {
        Err(self.rejection.note(Message::InvalidNumber, start as u32))
      }
      _ => Ok((kind, position)),
    }
  }

  /// Returns the end of the string that starts at `start`.
  fn read_string(&self, start: usize) -> Result<usize> {
    let mut position = start + 1;
    loop {
      let rest = self.text.get(position..).unwrap_or_default();
      position += crate::strings::index_of_any(rest, b"\"\\\n\r")
        .ok_or_else(|| self.rejection.note(Message::UnclosedString, start as u32))?;
      match self.byte(position) {
        Some(b'"') => return Ok(position + 1),
        Some(b'\\') => {
          position += read_escape(&self.text[position..])
            .ok_or_else(|| self.rejection.note(Message::InvalidEscape, position as u32))?
            .1
        }
        _ => return Err(self.rejection.note(Message::UnclosedString, start as u32)),
      }
    }
  }

  fn read_block_string(&self, start: usize) -> Result<usize> {
    let mut position = start + 3;
    loop {
      let rest = self.text.get(position..).unwrap_or_default();
      position += crate::strings::index_of(rest, b"\"\"\"")
        .ok_or_else(|| self.rejection.note(Message::UnclosedString, start as u32))?;
      // `\"""` doesn't end the string.
      if position > start + 3 && self.byte(position - 1) == Some(b'\\') {
        position += 3;
      } else {
        return Ok(position + 3);
      }
    }
  }

  fn advance(&mut self) -> Result<()> {
    self.last_end = self.token.end;
    self.token = self.lookahead()?;
    self.next = None;
    Ok(())
  }

  fn lookahead(&mut self) -> Result<Token> {
    if self.token.kind == TokenKind::EndOfFile {
      return Ok(self.token);
    }
    match self.next {
      Some(next) => Ok(next),
      None => {
        let next = self.read_token(self.token.end)?;
        self.next = Some(next);
        Ok(next)
      }
    }
  }

  fn text_of(&self, token: Token) -> &[u8] {
    self.text.get(token.start as usize..token.end as usize).unwrap_or_default()
  }

  fn peek(&self, kind: TokenKind) -> bool {
    self.token.kind == kind
  }

  fn is_keyword(&self, token: Token, keyword: &[u8]) -> bool {
    token.kind == TokenKind::Name && self.text_of(token) == keyword
  }

  /// Reports `message` at the current token.
  #[cold]
  fn unexpected(&self, message: Message) -> Rejected {
    let message = match self.token.kind {
      TokenKind::EndOfFile => Message::UnexpectedEnd,
      _ => message,
    };
    self.rejection.note(message, self.token.start)
  }

  fn expect(&mut self, kind: TokenKind) -> Result<Token> {
    let token = self.token;
    if token.kind != kind {
      return Err(self.unexpected(expected(kind)));
    }
    self.advance()?;
    Ok(token)
  }

  fn eat(&mut self, kind: TokenKind) -> Result<bool> {
    let found = self.peek(kind);
    if found {
      self.advance()?;
    }
    Ok(found)
  }

  fn expect_keyword(&mut self, keyword: &[u8]) -> Result<()> {
    match self.eat_keyword(keyword)? {
      true => Ok(()),
      false => Err(self.unexpected(Message::UnexpectedToken)),
    }
  }

  fn eat_keyword(&mut self, keyword: &[u8]) -> Result<bool> {
    let found = self.is_keyword(self.token, keyword);
    if found {
      self.advance()?;
    }
    Ok(found)
  }

  // ───────────────────────────── nodes ─────────────────────────────

  /// Returns what [`Parser::finish`] needs to know about where a node starts.
  fn begin(&self) -> (u32, usize) {
    (self.token.start, self.tree.pending.len())
  }

  /// Ends the node that started at `begin` just before the current token. The nodes finished since then become its
  /// children.
  fn finish(&mut self, kind: Kind, flags: u8, (start, first_pending): (u32, usize)) {
    let tree = &mut *self.tree;
    let first_child = tree.children.len() as u32;
    tree.children.extend(tree.pending.drain(first_pending.min(tree.pending.len())..));
    tree.pending.push(tree.nodes.len() as NodeId);
    let child_count = tree.children.len() as u32 - first_child;
    let span = Span::new(start, self.last_end.max(start));
    tree.nodes.push(Node { kind, flags, span, first_child, child_count });
  }

  /// Makes a node from just the current token.
  fn token_as(&mut self, kind: Kind, flags: u8) -> Result<()> {
    let begin = self.begin();
    self.advance()?;
    self.finish(kind, flags, begin);
    Ok(())
  }

  fn check_depth(&self) -> Result<()> {
    match self.stack.has_room() {
      true => Ok(()),
      false => Err(self.rejection.note(Message::UnexpectedToken, self.token.start)),
    }
  }

  /// `open item+ close`
  fn many(
    &mut self,
    open: TokenKind,
    item: fn(&mut Self) -> Result<()>,
    close: TokenKind,
  ) -> Result<()> {
    self.expect(open)?;
    loop {
      item(self)?;
      if self.eat(close)? {
        return Ok(());
      }
    }
  }

  /// Like `many`, but optional. Returns true if it's present.
  fn optional_many(
    &mut self,
    open: TokenKind,
    item: fn(&mut Self) -> Result<()>,
    close: TokenKind,
  ) -> Result<bool> {
    let present = self.peek(open);
    if present {
      self.many(open, item, close)?;
    }
    Ok(present)
  }

  /// `open item* close`
  fn any(
    &mut self,
    open: TokenKind,
    item: fn(&mut Self) -> Result<()>,
    close: TokenKind,
  ) -> Result<()> {
    self.expect(open)?;
    while !self.eat(close)? {
      item(self)?;
    }
    Ok(())
  }

  /// `delimiter? item (delimiter item)*`
  fn delimited_many(
    &mut self,
    delimiter: TokenKind,
    item: fn(&mut Self) -> Result<()>,
  ) -> Result<()> {
    self.eat(delimiter)?;
    loop {
      item(self)?;
      if !self.eat(delimiter)? {
        return Ok(());
      }
    }
  }

  // ───────────────────────────── the grammar ─────────────────────────────

  fn name(&mut self) -> Result<()> {
    match self.peek(TokenKind::Name) {
      true => self.token_as(Kind::Name, 0),
      false => Err(self.unexpected(Message::ExpectedName)),
    }
  }

  fn document(&mut self) -> Result<()> {
    let begin = self.begin();
    self.many(TokenKind::StartOfFile, Self::definition, TokenKind::EndOfFile)?;
    self.finish(Kind::Document, 0, begin);
    Ok(())
  }

  fn definition(&mut self) -> Result<()> {
    if self.peek(TokenKind::BraceL) {
      return self.operation_definition();
    }
    let described = self.peek_description();
    let keyword = if described { self.lookahead()? } else { self.token };
    if keyword.kind != TokenKind::Name {
      return Err(self.unexpected(Message::ExpectedDefinition));
    }
    match self.text_of(keyword) {
      b"schema" => self.schema_definition(),
      b"scalar" => self.scalar_type_definition(),
      b"type" => self.object_like_definition(b"type", Kind::ObjectTypeDefinition),
      b"interface" => self.object_like_definition(b"interface", Kind::InterfaceTypeDefinition),
      b"union" => self.union_type_definition(),
      b"enum" => self.enum_type_definition(),
      b"input" => self.input_object_type_definition(),
      b"directive" => self.directive_definition(),
      b"query" | b"mutation" | b"subscription" => self.operation_definition(),
      b"fragment" => self.fragment_definition(),
      b"extend" if !described => self.type_system_extension(),
      _ => Err(self.unexpected(Message::ExpectedDefinition)),
    }
  }

  fn operation_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    let mut flags = IS_SHORTHAND;
    if !self.peek(TokenKind::BraceL) {
      self.description()?;
      flags = self.operation_type()?;
      if self.peek(TokenKind::Name) {
        self.name()?;
      }
      self.variable_definitions()?;
      self.directives()?;
    }
    self.selection_set()?;
    self.finish(Kind::OperationDefinition, flags, begin);
    Ok(())
  }

  /// `query`, `mutation` or `subscription`. It isn't a node but a flag on the enclosing node.
  fn operation_type(&mut self) -> Result<u8> {
    let token = self.expect(TokenKind::Name)?;
    match self.text_of(token) {
      b"query" => Ok(QUERY),
      b"mutation" => Ok(MUTATION),
      b"subscription" => Ok(SUBSCRIPTION),
      _ => Err(self.rejection.note(Message::ExpectedDefinition, token.start)),
    }
  }

  fn variable_definitions(&mut self) -> Result<bool> {
    self.optional_many(TokenKind::ParenL, Self::variable_definition, TokenKind::ParenR)
  }

  fn variable_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.variable()?;
    self.expect(TokenKind::Colon)?;
    self.type_reference()?;
    if self.eat(TokenKind::Equals)? {
      self.const_value()?;
    }
    self.const_directives()?;
    self.finish(Kind::VariableDefinition, 0, begin);
    Ok(())
  }

  fn variable(&mut self) -> Result<()> {
    let begin = self.begin();
    self.expect(TokenKind::Dollar)?;
    self.name()?;
    self.finish(Kind::Variable, 0, begin);
    Ok(())
  }

  fn selection_set(&mut self) -> Result<()> {
    self.check_depth()?;
    let begin = self.begin();
    self.many(TokenKind::BraceL, Self::selection, TokenKind::BraceR)?;
    self.finish(Kind::SelectionSet, 0, begin);
    Ok(())
  }

  fn selection(&mut self) -> Result<()> {
    match self.peek(TokenKind::Spread) {
      true => self.fragment(),
      false => self.field(),
    }
  }

  fn field(&mut self) -> Result<()> {
    let begin = self.begin();
    self.name()?;
    let alias = self.eat(TokenKind::Colon)?;
    if alias {
      self.name()?;
    }
    self.optional_many(TokenKind::ParenL, Self::argument, TokenKind::ParenR)?;
    self.directives()?;
    if self.peek(TokenKind::BraceL) {
      self.selection_set()?;
    }
    self.finish(Kind::Field, if alias { HAS_ALIAS } else { 0 }, begin);
    Ok(())
  }

  /// `name: value`
  fn named_value(&mut self, kind: Kind, value: fn(&mut Self) -> Result<()>) -> Result<()> {
    let begin = self.begin();
    self.name()?;
    self.expect(TokenKind::Colon)?;
    value(self)?;
    self.finish(kind, 0, begin);
    Ok(())
  }

  fn argument(&mut self) -> Result<()> {
    self.named_value(Kind::Argument, Self::value)
  }

  fn const_argument(&mut self) -> Result<()> {
    self.named_value(Kind::Argument, Self::const_value)
  }

  fn fragment_argument(&mut self) -> Result<()> {
    self.named_value(Kind::FragmentArgument, Self::value)
  }

  fn fragment(&mut self) -> Result<()> {
    let begin = self.begin();
    self.expect(TokenKind::Spread)?;
    let typed = self.eat_keyword(b"on")?;
    if !typed && self.peek(TokenKind::Name) {
      self.fragment_name()?;
      self.optional_many(TokenKind::ParenL, Self::fragment_argument, TokenKind::ParenR)?;
      self.directives()?;
      self.finish(Kind::FragmentSpread, 0, begin);
      return Ok(());
    }
    if typed {
      self.named_type()?;
    }
    self.directives()?;
    self.selection_set()?;
    self.finish(Kind::InlineFragment, 0, begin);
    Ok(())
  }

  fn fragment_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.expect_keyword(b"fragment")?;
    self.fragment_name()?;
    self.variable_definitions()?;
    self.expect_keyword(b"on")?;
    self.named_type()?;
    self.directives()?;
    self.selection_set()?;
    self.finish(Kind::FragmentDefinition, 0, begin);
    Ok(())
  }

  fn fragment_name(&mut self) -> Result<()> {
    match self.is_keyword(self.token, b"on") {
      true => Err(self.unexpected(Message::ExpectedName)),
      false => self.name(),
    }
  }

  fn value(&mut self) -> Result<()> {
    self.value_literal(false)
  }

  fn const_value(&mut self) -> Result<()> {
    self.value_literal(true)
  }

  fn value_literal(&mut self, constant: bool) -> Result<()> {
    let value = if constant { Self::const_value } else { Self::value };
    match self.token.kind {
      TokenKind::BracketL => {
        self.check_depth()?;
        let begin = self.begin();
        self.any(TokenKind::BracketL, value, TokenKind::BracketR)?;
        self.finish(Kind::ListValue, 0, begin);
        Ok(())
      }
      TokenKind::BraceL => {
        self.check_depth()?;
        let begin = self.begin();
        let field = if constant { Self::const_object_field } else { Self::object_field };
        self.any(TokenKind::BraceL, field, TokenKind::BraceR)?;
        self.finish(Kind::ObjectValue, 0, begin);
        Ok(())
      }
      TokenKind::Int => self.token_as(Kind::IntValue, 0),
      TokenKind::Float => self.token_as(Kind::FloatValue, 0),
      TokenKind::String => self.token_as(Kind::StringValue, 0),
      TokenKind::BlockString => self.token_as(Kind::StringValue, IS_BLOCK),
      TokenKind::Name => {
        let kind = match self.text_of(self.token) {
          b"true" | b"false" => Kind::BooleanValue,
          b"null" => Kind::NullValue,
          _ => Kind::EnumValue,
        };
        self.token_as(kind, 0)
      }
      TokenKind::Dollar if !constant => self.variable(),
      _ => Err(self.unexpected(Message::ExpectedValue)),
    }
  }

  fn object_field(&mut self) -> Result<()> {
    self.named_value(Kind::ObjectField, Self::value)
  }

  fn const_object_field(&mut self) -> Result<()> {
    self.named_value(Kind::ObjectField, Self::const_value)
  }

  /// Returns true if there's at least one directive.
  fn directives_of(&mut self, argument: fn(&mut Self) -> Result<()>) -> Result<bool> {
    let any = self.peek(TokenKind::At);
    while self.peek(TokenKind::At) {
      let begin = self.begin();
      self.advance()?;
      self.name()?;
      self.optional_many(TokenKind::ParenL, argument, TokenKind::ParenR)?;
      self.finish(Kind::Directive, 0, begin);
    }
    Ok(any)
  }

  fn directives(&mut self) -> Result<bool> {
    self.directives_of(Self::argument)
  }

  fn const_directives(&mut self) -> Result<bool> {
    self.directives_of(Self::const_argument)
  }

  fn type_reference(&mut self) -> Result<()> {
    self.check_depth()?;
    let begin = self.begin();
    if self.eat(TokenKind::BracketL)? {
      self.type_reference()?;
      self.expect(TokenKind::BracketR)?;
      self.finish(Kind::ListType, 0, begin);
    } else {
      self.named_type()?;
    }
    if self.eat(TokenKind::Bang)? {
      self.finish(Kind::NonNullType, 0, begin);
    }
    Ok(())
  }

  fn named_type(&mut self) -> Result<()> {
    let begin = self.begin();
    self.name()?;
    self.finish(Kind::NamedType, 0, begin);
    Ok(())
  }

  fn peek_description(&self) -> bool {
    self.peek(TokenKind::String) || self.peek(TokenKind::BlockString)
  }

  fn description(&mut self) -> Result<()> {
    match self.token.kind {
      TokenKind::String => self.token_as(Kind::StringValue, 0),
      TokenKind::BlockString => self.token_as(Kind::StringValue, IS_BLOCK),
      _ => Ok(()),
    }
  }

  fn schema_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.expect_keyword(b"schema")?;
    self.const_directives()?;
    self.many(TokenKind::BraceL, Self::operation_type_definition, TokenKind::BraceR)?;
    self.finish(Kind::SchemaDefinition, 0, begin);
    Ok(())
  }

  fn operation_type_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    let operation = self.operation_type()?;
    self.expect(TokenKind::Colon)?;
    self.named_type()?;
    self.finish(Kind::OperationTypeDefinition, operation, begin);
    Ok(())
  }

  fn scalar_type_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.expect_keyword(b"scalar")?;
    self.name()?;
    self.const_directives()?;
    self.finish(Kind::ScalarTypeDefinition, 0, begin);
    Ok(())
  }

  /// `type A implements B @c { .. }`, `interface A ..`
  fn object_like_definition(&mut self, keyword: &[u8], kind: Kind) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.expect_keyword(keyword)?;
    self.name()?;
    self.implements_interfaces()?;
    self.const_directives()?;
    self.fields_definition()?;
    self.finish(kind, 0, begin);
    Ok(())
  }

  fn implements_interfaces(&mut self) -> Result<bool> {
    let found = self.eat_keyword(b"implements")?;
    if found {
      self.delimited_many(TokenKind::Amp, Self::named_type)?;
    }
    Ok(found)
  }

  fn fields_definition(&mut self) -> Result<bool> {
    self.optional_many(TokenKind::BraceL, Self::field_definition, TokenKind::BraceR)
  }

  fn field_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.name()?;
    self.argument_definitions()?;
    self.expect(TokenKind::Colon)?;
    self.type_reference()?;
    self.const_directives()?;
    self.finish(Kind::FieldDefinition, 0, begin);
    Ok(())
  }

  fn argument_definitions(&mut self) -> Result<bool> {
    self.optional_many(TokenKind::ParenL, Self::input_value_definition, TokenKind::ParenR)
  }

  fn input_value_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.name()?;
    self.expect(TokenKind::Colon)?;
    self.type_reference()?;
    if self.eat(TokenKind::Equals)? {
      self.const_value()?;
    }
    self.const_directives()?;
    self.finish(Kind::InputValueDefinition, 0, begin);
    Ok(())
  }

  fn union_type_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.expect_keyword(b"union")?;
    self.name()?;
    self.const_directives()?;
    self.union_member_types()?;
    self.finish(Kind::UnionTypeDefinition, 0, begin);
    Ok(())
  }

  fn union_member_types(&mut self) -> Result<bool> {
    let found = self.eat(TokenKind::Equals)?;
    if found {
      self.delimited_many(TokenKind::Pipe, Self::named_type)?;
    }
    Ok(found)
  }

  fn enum_type_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.expect_keyword(b"enum")?;
    self.name()?;
    self.const_directives()?;
    self.enum_values_definition()?;
    self.finish(Kind::EnumTypeDefinition, 0, begin);
    Ok(())
  }

  fn enum_values_definition(&mut self) -> Result<bool> {
    self.optional_many(TokenKind::BraceL, Self::enum_value_definition, TokenKind::BraceR)
  }

  fn enum_value_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    if matches!(self.text_of(self.token), b"true" | b"false" | b"null") {
      return Err(self.unexpected(Message::ExpectedName));
    }
    self.name()?;
    self.const_directives()?;
    self.finish(Kind::EnumValueDefinition, 0, begin);
    Ok(())
  }

  fn input_object_type_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.expect_keyword(b"input")?;
    self.name()?;
    self.const_directives()?;
    self.input_fields_definition()?;
    self.finish(Kind::InputObjectTypeDefinition, 0, begin);
    Ok(())
  }

  fn input_fields_definition(&mut self) -> Result<bool> {
    self.optional_many(TokenKind::BraceL, Self::input_value_definition, TokenKind::BraceR)
  }

  /// `extend ..`. An extension has to add something.
  fn type_system_extension(&mut self) -> Result<()> {
    let begin = self.begin();
    self.expect_keyword(b"extend")?;
    let keyword = self.expect(TokenKind::Name)?;
    let (kind, adds) = match self.text_of(keyword) {
      b"schema" => {
        let directives = self.const_directives()?;
        let types = self.optional_many(
          TokenKind::BraceL,
          Self::operation_type_definition,
          TokenKind::BraceR,
        )?;
        (Kind::SchemaExtension, directives || types)
      }
      b"scalar" => {
        self.name()?;
        (Kind::ScalarTypeExtension, self.const_directives()?)
      }
      keyword @ (b"type" | b"interface") => {
        let kind =
          if keyword == b"type" { Kind::ObjectTypeExtension } else { Kind::InterfaceTypeExtension };
        self.name()?;
        let interfaces = self.implements_interfaces()?;
        let directives = self.const_directives()?;
        let fields = self.fields_definition()?;
        (kind, interfaces || directives || fields)
      }
      b"union" => {
        self.name()?;
        let directives = self.const_directives()?;
        let types = self.union_member_types()?;
        (Kind::UnionTypeExtension, directives || types)
      }
      b"enum" => {
        self.name()?;
        let directives = self.const_directives()?;
        let values = self.enum_values_definition()?;
        (Kind::EnumTypeExtension, directives || values)
      }
      b"input" => {
        self.name()?;
        let directives = self.const_directives()?;
        let fields = self.input_fields_definition()?;
        (Kind::InputObjectTypeExtension, directives || fields)
      }
      b"directive" => {
        self.expect(TokenKind::At)?;
        self.name()?;
        (Kind::DirectiveExtension, self.const_directives()?)
      }
      _ => return Err(self.unexpected(Message::ExpectedDefinition)),
    };
    if !adds {
      return Err(self.rejection.note(Message::EmptyExtension, self.last_end));
    }
    self.finish(kind, 0, begin);
    Ok(())
  }

  fn directive_definition(&mut self) -> Result<()> {
    let begin = self.begin();
    self.description()?;
    self.expect_keyword(b"directive")?;
    self.expect(TokenKind::At)?;
    self.name()?;
    self.argument_definitions()?;
    self.const_directives()?;
    let repeatable = self.eat_keyword(b"repeatable")?;
    self.expect_keyword(b"on")?;
    self.delimited_many(TokenKind::Pipe, Self::directive_location)?;
    self.finish(Kind::DirectiveDefinition, if repeatable { IS_REPEATABLE } else { 0 }, begin);
    Ok(())
  }

  fn directive_location(&mut self) -> Result<()> {
    const LOCATIONS: [&[u8]; 21] = [
      b"QUERY",
      b"MUTATION",
      b"SUBSCRIPTION",
      b"FIELD",
      b"FRAGMENT_DEFINITION",
      b"FRAGMENT_SPREAD",
      b"INLINE_FRAGMENT",
      b"VARIABLE_DEFINITION",
      b"FRAGMENT_VARIABLE_DEFINITION",
      b"SCHEMA",
      b"SCALAR",
      b"OBJECT",
      b"FIELD_DEFINITION",
      b"ARGUMENT_DEFINITION",
      b"INTERFACE",
      b"UNION",
      b"ENUM",
      b"ENUM_VALUE",
      b"INPUT_OBJECT",
      b"INPUT_FIELD_DEFINITION",
      b"DIRECTIVE_DEFINITION",
    ];
    match LOCATIONS.contains(&self.text_of(self.token)) {
      true => self.name(),
      false => Err(self.unexpected(Message::ExpectedDirectiveLocation)),
    }
  }
}
