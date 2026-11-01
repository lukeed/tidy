//! Prettier's printers for JSON: `estree`, which prints `json`, `jsonc` and `json5` the way it prints JavaScript, and
//! `estree-json`, which prints `json-stringify` the way `JSON.stringify(value, null, 2)` does.

use super::ast::{Comment, Items, Kind, NodeId, Tree};
use super::comments::{Attached, Comments, Place, ROOT};
use super::{Dialect, ObjectWrap, Options, QuoteProps, cook, identifier, number};
use crate::doc::{Align, Branch, Docs, GroupId};
use crate::stack::Stack;
use crate::strings;
use crate::text;

pub(crate) struct Printer<'p, 'a> {
  pub(crate) tree: &'p Tree,
  pub(crate) text: &'a str,
  pub(crate) options: &'p Options,
  pub(crate) comments: &'p Comments,
  pub(crate) docs: &'p mut Docs<'a>,
  pub(crate) stack: &'p Stack,
  /// The object whose properties are printing has a string key that can't be unquoted: `hasSiblingsRequireQuoted`.
  pub(crate) quoted: bool,
}

/// What a trailing comment printed, which the next one depends on.
#[derive(Clone, Copy, Default)]
struct Trailing {
  block: bool,
  suffix: bool,
}

impl<'a> Printer<'_, 'a> {
  /// `JsonRoot`
  pub(crate) fn document(&mut self) {
    if self.options.dialect == Dialect::Stringify {
      if let Some(root) = self.tree.root {
        self.stringify(root, false);
      }
      self.docs.hardline();
      return;
    }
    self.dangling(ROOT);
    if let Some(root) = self.tree.root {
      self.node(root);
    }
    self.docs.hardline();
  }

  // ───────────────────────────── estree ─────────────────────────────

  /// A node with its comments: Prettier's `callPluginPrintFunction`.
  fn node(&mut self, id: NodeId) {
    self.with_comments(id, |printer| match printer.is_ignored(id) {
      true => printer.docs.text(printer.tree.node(id).span.of(printer.text)),
      false => printer.value(id),
    });
  }

  /// `printComments`: the leading comments, the node, and its trailing comments.
  fn with_comments(&mut self, id: NodeId, print: impl FnOnce(&mut Self)) {
    let attached = self.comments.of(id);
    for comment in attached.iter().filter(|it| it.place == Place::Leading) {
      self.leading(self.comment(comment));
    }
    print(self);
    let mut previous = None;
    for comment in attached.iter().filter(|it| it.place == Place::Trailing) {
      previous = Some(self.trailing(self.comment(comment), previous));
    }
  }

  fn value(&mut self, id: NodeId) {
    if !self.stack.has_room() {
      return;
    }
    let node = self.tree.node(id);
    let raw = node.span.of(self.text);
    match node.kind {
      Kind::Object(items) => self.object(id, items),
      Kind::Array(items) => self.array(id, items),
      Kind::Property { key, value } => self.property(key, value),
      Kind::String => self.string(raw),
      Kind::Number => self.number(raw),
      Kind::Identifier => self.identifier(raw),
      Kind::Template => {
        self.docs.line_suffix_boundary();
        self.lines(raw);
      }
      Kind::Null => self.docs.text("null"),
      Kind::True => self.docs.text("true"),
      Kind::False => self.docs.text("false"),
      Kind::Sign { minus, argument } => {
        self.docs.text(if minus { "-" } else { "+" });
        if self.comments.of(argument).is_empty() {
          return self.node(argument);
        }
        let group = self.docs.group();
        self.docs.text("(");
        let indent = self.docs.indent();
        self.docs.softline();
        self.node(argument);
        self.docs.close(indent);
        self.docs.softline();
        self.docs.text(")");
        self.docs.close(group);
      }
    }
  }

  /// `printObject`
  fn object(&mut self, id: NodeId, items: Items) {
    let properties = self.tree.items(items);
    let Some(&Some(first)) = properties.first() else {
      let group = self.docs.group();
      self.docs.text("{");
      self.dangling_in_list(id);
      self.docs.text("}");
      return self.docs.close(group);
    };
    let start = self.tree.node(id).span.start as usize;
    let first = self.tree.node(first).span.start as usize;
    let broken = self.options.object_wrap == ObjectWrap::Preserve
      && strings::contains_char(&self.text.as_bytes()[start..first], b'\n');
    let group = self.docs.group_with(None, broken);
    self.docs.text("{");
    let indent = self.docs.indent();
    self.spacing();
    let quoted = self.siblings_need_quotes(items);
    let quoted = std::mem::replace(&mut self.quoted, quoted);
    let mut previous = None;
    for &property in properties.iter().flatten() {
      if let Some(previous) = previous {
        self.docs.text(",");
        self.docs.line();
        if text::is_next_line_empty(self.text.as_bytes(), self.end(previous)) {
          self.docs.hardline();
        }
      }
      self.node(property);
      previous = Some(property);
    }
    self.quoted = quoted;
    self.docs.close(indent);
    if self.trailing_comma() {
      let comma = self.docs.if_break(Branch::Break, None);
      self.docs.text(",");
      self.docs.close(comma);
    }
    self.spacing();
    self.docs.text("}");
    self.docs.close(group);
  }

  fn spacing(&mut self) {
    match self.options.bracket_spacing {
      true => self.docs.line(),
      false => self.docs.softline(),
    }
  }

  /// `printArray`
  fn array(&mut self, id: NodeId, items: Items) {
    let elements = self.tree.items(items);
    if elements.is_empty() {
      let group = self.docs.group();
      self.docs.text("[");
      self.dangling_in_list(id);
      self.docs.text("]");
      return self.docs.close(group);
    }
    let broken = (elements.len() > 1 && self.are_all_nested(elements))
      || self
        .comments
        .of(id)
        .iter()
        .any(|it| it.place == Place::Dangling && !self.comment(it).block);
    let concise = self.is_concise(elements);
    let id_ = self.docs.group_id();
    let group = self.docs.group_with(Some(id_), broken);
    self.docs.text("[");
    let indent = self.docs.indent();
    self.docs.softline();
    let comma = match () {
      _ if elements.last() == Some(&None) => Comma::Forced,
      _ if !self.trailing_comma() => Comma::None,
      _ if concise => Comma::Group(id_),
      _ => Comma::Break,
    };
    match concise {
      true => self.elements_concisely(elements, comma),
      false => {
        self.elements(elements);
        self.comma(comma);
      }
    }
    self.dangling(id);
    self.docs.close(indent);
    self.docs.softline();
    self.docs.text("]");
    self.docs.close(group);
  }

  /// More than one element, each an object or array with more than one item, all of the same kind.
  fn are_all_nested(&self, elements: &[Option<NodeId>]) -> bool {
    let kind = |element: &Option<NodeId>| match element.map(|id| self.tree.node(id).kind) {
      Some(Kind::Object(items)) => Some((true, items.len())),
      Some(Kind::Array(items)) => Some((false, items.len())),
      _ => None,
    };
    let Some((object, _)) = kind(&elements[0]) else { return false };
    elements.iter().all(|element| kind(element).is_some_and(|(it, len)| it == object && len > 1))
  }

  /// `isConciselyPrintedArray`: every element is a number, maybe signed, without a line comment after it on its line.
  fn is_concise(&self, elements: &[Option<NodeId>]) -> bool {
    elements.iter().all(|element| {
      let Some(element) = *element else { return false };
      let numeric = match self.tree.node(element).kind {
        Kind::Number => true,
        Kind::Sign { argument, .. } => {
          self.tree.node(argument).kind == Kind::Number && self.comments.of(argument).is_empty()
        }
        _ => false,
      };
      numeric
        && !self.comments.of(element).iter().any(|it| {
          let comment = self.comment(it);
          it.place == Place::Trailing
            && !comment.block
            && !text::has_newline_backwards(self.text.as_bytes(), comment.span.start as usize)
        })
    })
  }

  /// `printArrayElements`
  fn elements(&mut self, elements: &[Option<NodeId>]) {
    for (index, element) in elements.iter().enumerate() {
      if let Some(element) = *element {
        let group = self.docs.group();
        self.node(element);
        self.docs.close(group);
      }
      if index + 1 < elements.len() {
        self.docs.text(",");
        self.docs.line();
        if element.is_some_and(|element| self.is_line_after_empty(element)) {
          self.docs.softline();
        }
      }
    }
  }

  /// `printArrayElementsConcisely`
  fn elements_concisely(&mut self, elements: &[Option<NodeId>], comma: Comma) {
    let fill = self.docs.fill();
    for (index, element) in elements.iter().flatten().enumerate() {
      let part = self.docs.part();
      self.node(*element);
      match index + 1 == elements.len() {
        true => self.comma(comma),
        false => self.docs.text(","),
      }
      self.docs.close(part);
      let Some(Some(next)) = elements.get(index + 1) else { continue };
      let part = self.docs.part();
      if self.is_line_after_empty(*element) {
        self.docs.hardline();
        self.docs.hardline();
      } else if self
        .comments
        .of(*next)
        .iter()
        .any(|it| it.place == Place::Leading && !self.comment(it).block)
      {
        self.docs.hardline();
      } else {
        self.docs.line();
      }
      self.docs.close(part);
    }
    self.docs.close(fill);
  }

  fn comma(&mut self, comma: Comma) {
    match comma {
      Comma::None => {}
      Comma::Forced => self.docs.text(","),
      Comma::Break | Comma::Group(_) => {
        let id = match comma {
          Comma::Group(id) => Some(id),
          _ => None,
        };
        let mark = self.docs.if_break(Branch::Break, id);
        self.docs.text(",");
        self.docs.close(mark);
      }
    }
  }

  /// `isLineAfterElementEmpty`: the line after the comma that follows `element` is blank.
  fn is_line_after_empty(&self, element: NodeId) -> bool {
    let text = self.text.as_bytes();
    let mut at = self.end(element);
    while at < text.len() {
      if text[at] == b',' {
        break;
      }
      at = text::skip_inline_comment(text, text::skip_trailing_comment(text, at + 1));
    }
    text::is_next_line_empty(text, at)
  }

  /// `printProperty`, through `printAssignment` with the two layouts JSON can have.
  fn property(&mut self, key: NodeId, value: NodeId) {
    let text = self.text.as_bytes();
    let breaks = self.comments.of(value).iter().any(|it| {
      let comment = self.comment(it);
      it.place == Place::Leading
        && (text::has_newline(text, comment.span.end as usize) || self.is_indentable(comment))
    });
    let group = self.docs.group();
    let left = self.docs.group();
    self.key(key);
    self.docs.close(left);
    self.docs.text(":");
    match breaks {
      true => {
        let right = self.docs.group();
        let indent = self.docs.indent();
        self.docs.line();
        self.node(value);
        self.docs.close(indent);
        self.docs.close(right);
      }
      false => {
        self.docs.text(" ");
        self.node(value);
      }
    }
    self.docs.close(group);
  }

  /// `printKey`
  fn key(&mut self, key: NodeId) {
    let node = self.tree.node(key);
    let raw = node.span.of(self.text);
    if self.should_quote(key) {
      let name = match node.kind {
        Kind::Identifier => cook::identifier(raw.as_bytes()).into_owned(),
        _ => number::print(raw),
      };
      let quote = self.quote(&name) as char;
      let quoted = format!("{quote}{name}{quote}");
      return self.with_comments(key, |printer| printer.docs.owned(&quoted));
    }
    if self.should_unquote(key) {
      let name = &raw[1..raw.len() - 1];
      return self.with_comments(key, |printer| printer.docs.text(name));
    }
    self.node(key);
  }

  /// `shouldQuoteKey`
  fn should_quote(&self, key: NodeId) -> bool {
    let quote = match self.options.dialect {
      Dialect::Json | Dialect::Jsonc => true,
      _ => self.options.quote_props == QuoteProps::Consistent && self.quoted,
    };
    quote && self.is_safe_to_quote(key)
  }

  /// `shouldUnquoteKey`, which only JSON5 can.
  fn should_unquote(&self, key: NodeId) -> bool {
    let unquote = match self.options.quote_props {
      QuoteProps::AsNeeded => true,
      QuoteProps::Consistent => !self.quoted,
      QuoteProps::Preserve => false,
    };
    unquote && self.is_safe_to_unquote(key)
  }

  /// `isKeySafeToQuote`: a name, or a number that prints as JavaScript writes it.
  fn is_safe_to_quote(&self, key: NodeId) -> bool {
    let node = self.tree.node(key);
    match node.kind {
      Kind::Identifier => true,
      Kind::Number => {
        let printed = number::print(node.span.of(self.text));
        number::is_simple(&printed) && number::is_canonical(&printed)
      }
      _ => false,
    }
  }

  /// `isKeySafeToUnquote`: a string with no escapes that is an ES5 identifier name.
  fn is_safe_to_unquote(&self, key: NodeId) -> bool {
    let node = self.tree.node(key);
    if !matches!(self.options.dialect, Dialect::Json5) || node.kind != Kind::String {
      return false;
    }
    let raw = node.span.of(self.text);
    let content = &raw[1..raw.len() - 1];
    !content.contains('\\') && identifier::is_es5_identifier_name(content)
  }

  /// `hasSiblingsRequireQuoted`: a key of the object is a string that can't be unquoted.
  fn siblings_need_quotes(&self, items: Items) -> bool {
    self.options.quote_props == QuoteProps::Consistent
      && self.tree.items(items).iter().flatten().any(|property| {
        let Kind::Property { key, .. } = self.tree.node(*property).kind else { return false };
        self.tree.node(key).kind == Kind::String && !self.is_safe_to_unquote(key)
      })
  }

  /// `printString`, with `replaceEndOfLine`.
  fn string(&mut self, raw: &'a str) {
    let content = &raw[1..raw.len() - 1];
    let quote = self.quote(content);
    if raw.as_bytes()[0] == quote {
      return self.lines(raw);
    }
    let mut out = Vec::with_capacity(raw.len() + 2);
    text::make_string(content.as_bytes(), quote, &mut out);
    let out = self.docs.keep(&String::from_utf8_lossy(&out));
    self.lines(out);
  }

  /// The quote `printString` encloses `content` in.
  fn quote(&self, content: &str) -> u8 {
    let Options { dialect, quote_props, single_quote, .. } = *self.options;
    match dialect {
      Dialect::Json5 if !(quote_props == QuoteProps::Preserve && !single_quote) => {
        preferred_quote(content, single_quote)
      }
      _ => b'"',
    }
  }

  /// `printNumber`
  fn number(&mut self, raw: &'a str) {
    match raw.bytes().all(|byte| byte.is_ascii_digit()) {
      true => self.docs.text(raw),
      false => self.docs.owned(&number::print(raw)),
    }
  }

  fn identifier(&mut self, raw: &'a str) {
    match raw.contains('\\') {
      true => self.docs.owned(&cook::identifier(raw.as_bytes())),
      false => self.docs.text(raw),
    }
  }

  /// `replaceEndOfLine`: each line break as a `literalline`.
  fn lines(&mut self, text: &'a str) {
    for (index, line) in text.split('\n').enumerate() {
      if index > 0 {
        self.docs.literalline();
      }
      self.docs.text(line);
    }
  }

  fn trailing_comma(&self) -> bool {
    self.options.trailing_comma && self.options.dialect != Dialect::Json
  }

  fn end(&self, id: NodeId) -> usize {
    self.tree.node(id).span.end as usize
  }

  /// `hasNodeIgnoreComment`: a comment of the node, of any kind, is the ignore directive.
  fn is_ignored(&self, id: NodeId) -> bool {
    let name = self.options.ignore.as_str();
    self.comments.of(id).iter().any(|it| {
      let value = self.comment(it).value(self.text);
      strings::trim_js_whitespace(value.as_bytes()) == name.as_bytes()
    })
  }

  // ───────────────────────────── comments ─────────────────────────────

  fn comment(&self, attached: &Attached) -> Comment {
    self.tree.comments[attached.comment as usize]
  }

  /// `printDanglingComments`
  fn dangling(&mut self, id: NodeId) {
    let mut first = true;
    for comment in self.comments.of(id).iter().filter(|it| it.place == Place::Dangling) {
      if !first {
        self.docs.hardline();
      }
      first = false;
      self.print_comment(self.comment(comment));
    }
  }

  /// `printDanglingCommentsInList`
  fn dangling_in_list(&mut self, id: NodeId) {
    let dangling = || self.comments.of(id).iter().filter(|it| it.place == Place::Dangling);
    if dangling().next().is_none() {
      return;
    }
    let line = dangling().any(|it| !self.comment(it).block);
    let indent = self.docs.indent();
    self.docs.softline();
    self.dangling(id);
    self.docs.close(indent);
    match line {
      true => self.docs.hardline(),
      false => self.docs.softline(),
    }
  }

  /// `printLeadingComment`
  fn leading(&mut self, comment: Comment) {
    let text = self.text.as_bytes();
    let (start, end) = (comment.span.start as usize, comment.span.end as usize);
    self.print_comment(comment);
    if !comment.block {
      self.docs.hardline();
    } else if !text::has_newline(text, end) {
      self.docs.text(" ");
    } else if text::has_newline_backwards(text, start) {
      self.docs.hardline();
    } else {
      self.docs.line();
    }
    let after = text::skip_newline(text, text::skip_spaces(text, end));
    if text::has_newline(text, after) {
      self.docs.hardline();
    }
  }

  /// `printTrailingComment`
  fn trailing(&mut self, comment: Comment, previous: Option<Trailing>) -> Trailing {
    let text = self.text.as_bytes();
    let start = comment.span.start as usize;
    let previous = previous.unwrap_or_default();
    if (previous.suffix && !previous.block) || text::has_newline_backwards(text, start) {
      let suffix = self.docs.line_suffix();
      self.docs.hardline();
      if is_previous_line_empty(text, start) {
        self.docs.hardline();
      }
      self.print_comment(comment);
      self.docs.close(suffix);
      return Trailing { block: comment.block, suffix: true };
    }
    if !comment.block || previous.suffix {
      let suffix = self.docs.line_suffix();
      self.docs.text(" ");
      self.print_comment(comment);
      self.docs.close(suffix);
      self.docs.break_parent();
      return Trailing { block: comment.block, suffix: true };
    }
    self.docs.text(" ");
    self.print_comment(comment);
    Trailing { block: true, suffix: false }
  }

  /// `printComment` of `estree`.
  fn print_comment(&mut self, comment: Comment) {
    let raw = comment.span.of(self.text);
    if !comment.block {
      let trimmed = strings::trim_js_whitespace_end(raw.as_bytes()).len();
      return self.docs.text(&raw[..trimmed]);
    }
    if !self.is_indentable(comment) {
      return self.lines(raw);
    }
    let value = comment.value(self.text);
    let jsdoc = value.starts_with('*') && !value.starts_with("**");
    let lines: Vec<&'a str> = raw[1..raw.len() - 1].split('\n').map(trim_start).collect();
    self.docs.text("/");
    for (index, line) in lines.iter().enumerate() {
      let trimmed = trim_end(line);
      if index == 0 {
        self.docs.text(trimmed);
        self.docs.hardline();
      } else if index == lines.len() - 1 {
        self.docs.text(" ");
        self.docs.text(line);
      } else {
        self.docs.text(" ");
        self.docs.text(trimmed);
        if jsdoc && trimmed != "*" && line.ends_with("  ") {
          self.docs.text("  ");
          let root = self.docs.align(Align::Mark);
          self.docs.literalline();
          self.docs.close(root);
        } else {
          self.docs.hardline();
        }
      }
    }
    self.docs.text("/");
  }

  /// `isIndentableBlockComment`: a block comment of more than one line, each starting with `*`.
  fn is_indentable(&self, comment: Comment) -> bool {
    let raw = comment.span.of(self.text);
    comment.block
      && comment.value(self.text).contains('\n')
      && raw[1..raw.len() - 1].split('\n').all(|line| trim_start(line).starts_with('*'))
  }

  // ───────────────────────────── estree-json ─────────────────────────────

  /// `printJson` of `estree-json`. Its input has no comments.
  fn stringify(&mut self, id: NodeId, key: bool) {
    if !self.stack.has_room() {
      return;
    }
    let node = self.tree.node(id);
    let raw = node.span.of(self.text);
    match node.kind {
      Kind::Object(items) | Kind::Array(items) => {
        let object = matches!(node.kind, Kind::Object(_));
        let (open, close) = if object { ("{", "}") } else { ("[", "]") };
        self.docs.text(open);
        if !items.is_empty() {
          let indent = self.docs.indent();
          for (index, item) in self.tree.items(items).iter().enumerate() {
            if index > 0 {
              self.docs.text(",");
            }
            self.docs.hardline();
            match item {
              Some(item) => self.stringify(*item, false),
              None => self.docs.text("null"),
            }
          }
          self.docs.close(indent);
          self.docs.hardline();
        }
        self.docs.text(close);
      }
      Kind::Property { key, value } => {
        self.stringify(key, true);
        self.docs.text(": ");
        self.stringify(value, false);
      }
      Kind::Sign { minus, argument } => {
        if minus {
          self.docs.text("-");
        }
        self.stringify(argument, false);
      }
      Kind::Null => self.docs.text("null"),
      Kind::True => self.docs.text("true"),
      Kind::False => self.docs.text("false"),
      Kind::String => self.string(raw),
      Kind::Number if key && number::is_canonical(raw) => self.docs.owned(&format!("\"{raw}\"")),
      Kind::Number => self.docs.text(raw),
      Kind::Identifier => {
        let name = cook::identifier(raw.as_bytes());
        match key {
          true => self.docs.owned(&cook::stringify(&name.encode_utf16().collect::<Vec<_>>())),
          false => self.docs.owned(&name),
        }
      }
      Kind::Template => {
        let value = cook::template(&raw.as_bytes()[1..raw.len() - 1]);
        self.docs.owned(&cook::stringify(&value));
      }
    }
  }
}

#[derive(Clone, Copy)]
enum Comma {
  None,
  /// After a hole, which needs it.
  Forced,
  /// `ifBreak(",")`
  Break,
  /// `ifBreak(",", "", { groupId })`
  Group(GroupId),
}

/// `getPreferredQuote`: the preferred quote, unless the text has more of it than of the other.
fn preferred_quote(content: &str, single: bool) -> u8 {
  let (preferred, alternate) = if single { (b'\'', b'"') } else { (b'"', b'\'') };
  let count = |quote: u8| content.bytes().filter(|byte| *byte == quote).count();
  match count(preferred) > count(alternate) {
    true => alternate,
    false => preferred,
  }
}

/// `isPreviousLineEmpty`
fn is_previous_line_empty(text: &[u8], start: usize) -> bool {
  let end = text::skip_spaces_backwards(text, start);
  let end = text::skip_newline_backwards(text, end);
  let end = text::skip_spaces_backwards(text, end);
  text::skip_newline_backwards(text, end) != end
}

fn trim_start(text: &str) -> &str {
  let trimmed = strings::trim_js_whitespace_start(text.as_bytes()).len();
  &text[text.len() - trimmed..]
}

fn trim_end(text: &str) -> &str {
  let trimmed = strings::trim_js_whitespace_end(text.as_bytes()).len();
  &text[..trimmed]
}
