//! The grammar of `@xml-tools/parser`. Its parser recovers from errors, but the plugin throws on the first one, so this
//! accepts exactly what the grammar accepts and rejects the rest.

use super::Abort;
use super::ast::{Attribute, Doctype, Element, External, Item, Prolog, Span, Top, Tree};
use super::lexer::{Kind, Token};
use crate::stack::Stack;
use crate::syntax::{Message, Rejection};

pub(crate) struct Parser<'p> {
  pub(crate) text: &'p str,
  pub(crate) tokens: &'p [Token],
  pub(crate) tree: &'p mut Tree,
  /// The content of the elements that are open, innermost last.
  pub(crate) pending: &'p mut Vec<Item>,
  pub(crate) rejection: &'p Rejection,
  pub(crate) stack: &'p Stack,
  pub(crate) at: usize,
}

impl Parser<'_> {
  /// `document`: a prolog, a doctype, and one element, with comments, processing instructions and whitespace around
  /// them.
  pub(crate) fn document(&mut self) -> Result<(), Abort> {
    if self.kind() == Some(Kind::Declaration) {
      let prolog = self.prolog()?;
      self.tree.top.push(Top::Prolog(prolog));
    }
    self.misc();
    if self.kind() == Some(Kind::DocType) {
      let doctype = self.doctype()?;
      self.tree.top.push(Top::Doctype(doctype));
      self.misc();
    }
    let element = self.element()?;
    self.tree.top.push(Top::Item(Item::Element(element)));
    self.misc();
    match self.at < self.tokens.len() {
      true => Err(self.error(Message::ExpectedEndOfInput)),
      false => Ok(()),
    }
  }

  fn misc(&mut self) {
    while let Some(token) = self.tokens.get(self.at) {
      let span = Span { start: token.start, end: token.end };
      match token.kind {
        Kind::Comment => self.tree.top.push(Top::Item(Item::Comment(span))),
        Kind::Instruction => self.tree.top.push(Top::Item(Item::Instruction(span))),
        Kind::Space => {}
        _ => return,
      }
      self.at += 1;
    }
  }

  fn prolog(&mut self) -> Result<Prolog, Abort> {
    let open = self.expect(Kind::Declaration, Message::UnexpectedToken)?;
    let attributes = self.attributes()?;
    self.expect(Kind::SpecialClose, Message::ExpectedDeclarationEnd)?;
    Ok(Prolog { open, attributes })
  }

  fn doctype(&mut self) -> Result<Doctype, Abort> {
    self.expect(Kind::DocType, Message::UnexpectedToken)?;
    let name = self.expect(Kind::Name, Message::ExpectedName)?;
    let mut external = None;
    if self.kind() == Some(Kind::Name) {
      let keyword = self.expect(Kind::Name, Message::ExpectedName)?;
      external = Some(match keyword.of(self.text) {
        "SYSTEM" => External::System { keyword, system: self.string()? },
        "PUBLIC" => External::Public { keyword, public: self.string()?, system: self.string()? },
        _ => return Err(self.error_at(Message::ExpectedExternalId, keyword.start)),
      });
    }
    self.expect(Kind::Close, Message::ExpectedClose)?;
    Ok(Doctype { name, external })
  }

  fn element(&mut self) -> Result<u32, Abort> {
    if !self.stack.has_room() {
      return Err(Abort::TooDeep);
    }
    let open = self.expect(Kind::Open, Message::ExpectedElement)?;
    let name = self.expect(Kind::Name, Message::ExpectedName)?;
    let attributes = self.attributes()?;
    let (content, end_name, end) = match self.kind() {
      Some(Kind::Close) => {
        self.at += 1;
        let content = self.content()?;
        self.expect(Kind::SlashOpen, Message::ExpectedEndTag)?;
        let end_name = self.expect(Kind::Name, Message::ExpectedName)?;
        let close = self.expect(Kind::Close, Message::ExpectedClose)?;
        (Some(content), end_name, close.end)
      }
      Some(Kind::SlashClose) => {
        self.at += 1;
        (None, name, self.tokens[self.at - 1].end)
      }
      _ => return Err(self.error(Message::ExpectedTagEnd)),
    };
    let span = Span { start: open.start, end };
    self.tree.elements.push(Element { span, name, attributes, content, end_name });
    Ok(self.tree.elements.len() as u32 - 1)
  }

  fn content(&mut self) -> Result<(u32, u32), Abort> {
    let mark = self.pending.len();
    while let Some(token) = self.tokens.get(self.at) {
      let span = Span { start: token.start, end: token.end };
      let item = match token.kind {
        Kind::Open => Item::Element(self.element()?),
        Kind::Text => Item::Text(span),
        Kind::Space => Item::Space(span),
        Kind::EntityRef | Kind::CharRef => Item::Reference(span),
        Kind::CData => Item::CData(span),
        Kind::Instruction => Item::Instruction(span),
        Kind::Comment => Item::Comment(span),
        _ => break,
      };
      if !matches!(item, Item::Element(_)) {
        self.at += 1;
      }
      self.pending.push(item);
    }
    let start = self.tree.items.len() as u32;
    self.tree.items.extend(self.pending.drain(mark..));
    Ok((start, self.tree.items.len() as u32))
  }

  fn attributes(&mut self) -> Result<(u32, u32), Abort> {
    let start = self.tree.attributes.len() as u32;
    while self.kind() == Some(Kind::Name) {
      let name = self.expect(Kind::Name, Message::ExpectedName)?;
      self.expect(Kind::Equals, Message::ExpectedEquals)?;
      let value = self.string()?;
      self.tree.attributes.push(Attribute { name, value });
    }
    Ok((start, self.tree.attributes.len() as u32))
  }

  fn string(&mut self) -> Result<Span, Abort> {
    self.expect(Kind::String, Message::ExpectedString)
  }

  fn kind(&self) -> Option<Kind> {
    self.tokens.get(self.at).map(|token| token.kind)
  }

  fn expect(&mut self, kind: Kind, message: Message) -> Result<Span, Abort> {
    match self.tokens.get(self.at) {
      Some(token) if token.kind == kind => {
        self.at += 1;
        Ok(Span { start: token.start, end: token.end })
      }
      _ => Err(self.error(message)),
    }
  }

  fn error(&self, message: Message) -> Abort {
    let at = self.tokens.get(self.at).map_or(self.text.len() as u32, |token| token.start);
    self.error_at(message, at)
  }

  fn error_at(&self, message: Message, at: u32) -> Abort {
    Abort::Syntax(self.rejection.note(message, at))
  }
}
