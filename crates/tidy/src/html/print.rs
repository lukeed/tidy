//! Prettier's `printer-html.js`, `print/children.js`, `print/element.js` and `print/tag.js`, emitting the doc stream.

use std::borrow::Cow;

use super::ast::{self, Flags, Id, Kind, Tree};
use super::embed::{self, Embedded};
use super::tag::Tags;
use super::utilities::{
  dedent_string, html_split, html_trim_preserve_indentation, is_pre_like, is_script_like_tag,
  is_text_like, should_unquote_attribute_value, unescape_quote_entities,
};
use super::{Context, Parser};
use crate::doc::{Align, Branch, Docs};
use crate::embed::Embed;
use crate::strings;

pub(crate) struct Printer<'p, 'a> {
  /// The text that is formatted, which texts that are part of it borrow from.
  source: &'a str,
  /// A buffer for the markers of tags.
  buffer: Vec<u8>,
  /// Whether texts escape `"` as `&quot;`, as in the value of an attribute.
  pub(crate) quotes: bool,
  pub(crate) tree: &'p Tree<'a>,
  pub(crate) options: &'p Context<'p>,
  pub(crate) docs: &'p mut Docs<'a>,
  pub(crate) embed: &'p mut dyn Embed,
  /// The number of `indent`s around what is printed now.
  pub(crate) indents: usize,
  /// `path.ancestors.length` of the node printed now.
  ancestors: usize,
}

/// What `printBetweenLine` returns.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Between {
  None,
  Hard,
  Line,
  Soft,
}

impl<'p, 'a> Printer<'p, 'a> {
  pub(crate) fn new(
    source: &'a str,
    tree: &'p Tree<'a>,
    options: &'p Context<'p>,
    docs: &'p mut Docs<'a>,
    embed: &'p mut dyn Embed,
  ) -> Self {
    Self {
      source,
      buffer: Vec::new(),
      quotes: false,
      tree,
      options,
      docs,
      embed,
      indents: 0,
      ancestors: 0,
    }
  }

  pub(crate) fn tags(&self) -> Tags<'p, 'a, 'p> {
    Tags { tree: self.tree, options: self.options }
  }

  /// Emits a text, borrowed from the source when it is part of it.
  pub(crate) fn bytes(&mut self, bytes: &[u8]) {
    if self.quotes && strings::contains_char(bytes, b'"') {
      let escaped = replace(bytes, b"\"", b"&quot;");
      self.quotes = false;
      self.bytes(&escaped);
      self.quotes = true;
      return;
    }
    let start = (bytes.as_ptr() as usize).wrapping_sub(self.source.as_ptr() as usize);
    if let Some(text) = self.source.get(start..start + bytes.len()) {
      return self.docs.text(text);
    }
    match std::str::from_utf8(bytes) {
      Ok(text) => self.docs.owned(text),
      Err(_) => self.docs.owned(&String::from_utf8_lossy(bytes)),
    }
  }

  /// Emits the string that `write` builds from the tag markers.
  pub(crate) fn marker(&mut self, write: impl FnOnce(Tags<'p, 'a, 'p>, &mut Vec<u8>)) {
    let mut out = std::mem::take(&mut self.buffer);
    out.clear();
    write(self.tags(), &mut out);
    match std::str::from_utf8(&out) {
      Ok(text) => self.docs.owned(text),
      Err(_) => self.docs.owned(&String::from_utf8_lossy(&out)),
    }
    self.buffer = out;
  }

  /// `replaceEndOfLine(text, separator)`
  pub(crate) fn lines(&mut self, text: &[u8], separator: Separator) {
    for (index, line) in strings::split(text, b"\n").enumerate() {
      if index > 0 {
        self.separator(separator);
      }
      self.bytes(line);
    }
  }

  fn separator(&mut self, separator: Separator) {
    match separator {
      Separator::Literal => self.docs.literalline(),
      Separator::Hard => self.docs.hardline(),
    }
  }

  // ───────────────────────────── `genericPrint` ─────────────────────────────

  pub(crate) fn print_root(&mut self) {
    let root = self.tree.root;
    self.ancestors = 0;
    let group = self.docs.group();
    self.print_children(root);
    self.docs.close(group);
    self.docs.hardline();
  }

  /// `path.call(print)` for `id`: the embedded printer first, then `genericPrint`.
  fn print(&mut self, id: Id) {
    if self.print_embedded(id) {
      return;
    }
    let node = &self.tree[id];
    match node.kind {
      Kind::Element | Kind::IeConditionalComment => self.print_element(id),
      Kind::IeConditionalStartComment | Kind::IeConditionalEndComment => {
        self.marker(|tags, out| tags.opening_tag_start(id, out));
        self.marker(|tags, out| tags.closing_tag_end(id, out));
      }
      Kind::Interpolation => {
        self.marker(|tags, out| tags.opening_tag_start(id, out));
        let mut next = self.tree.first_child(id);
        while let Some(child) = next {
          next = self.tree.next(child);
          self.print_nested(child);
        }
        self.marker(|tags, out| tags.closing_tag_end(id, out));
      }
      Kind::Text => self.print_text(id),
      Kind::DocType => {
        let group = self.docs.group();
        self.marker(|tags, out| tags.opening_tag_start(id, out));
        self.docs.text(" ");
        let value = doc_type_value(&node.value);
        self.bytes(&value);
        self.docs.close(group);
        self.marker(|tags, out| tags.closing_tag_end(id, out));
      }
      Kind::Comment => {
        self.marker(|tags, out| tags.opening_tag_prefix(id, out));
        let span = node.span;
        let text = &self.options.original_text[span.start as usize..span.end as usize];
        self.lines(text, Separator::Literal);
        self.marker(|tags, out| tags.closing_tag_suffix(id, out));
      }
      Kind::FrontMatter => {
        let span = node.span;
        let raw = &self.options.original_text[span.start as usize..span.end as usize];
        if !embed::front_matter(self, raw) {
          self.lines(raw, Separator::Literal);
        }
      }
      _ => {}
    }
  }

  /// Prints a child, one level deeper.
  fn print_nested(&mut self, id: Id) {
    self.ancestors += 1;
    self.print(id);
    self.ancestors -= 1;
  }

  fn print_text(&mut self, id: Id) {
    let node = &self.tree[id];
    let parent = self.tree.parent(id).map(|parent| &self.tree[parent]);
    if parent.is_some_and(|parent| parent.kind == Kind::Interpolation) {
      let value = &node.value[..];
      let trailing = trailing_newline(value);
      self.lines(trailing.map_or(value, |at| &value[..at]), Separator::Literal);
      if trailing.is_some() {
        self.docs.hardline();
      }
      return;
    }
    // `fill(getTextValueParts(node))`, with the prefix in the first part and the suffix in the last.
    let fill = self.docs.fill();
    let mut part = self.docs.part();
    self.marker(|tags, out| tags.opening_tag_prefix(id, out));
    let value = &node.value[..];
    let parent = &self.tree[self.tree.parent(id).unwrap_or(self.tree.root)];
    let mut write =
      |printer: &mut Self, index: usize, piece: &[u8], separator: Option<Separator>| {
        if index > 0 {
          printer.docs.close(part);
          let between = printer.docs.part();
          match separator {
            Some(separator) => printer.separator(separator),
            None => printer.docs.line(),
          }
          printer.docs.close(between);
          part = printer.docs.part();
        }
        printer.bytes(piece);
      };
    if parent.has(Flags::IS_WHITESPACE_SENSITIVE) {
      if parent.has(Flags::IS_INDENTATION_SENSITIVE) {
        for (index, line) in strings::split(value, b"\n").enumerate() {
          write(self, index, line, Some(Separator::Literal));
        }
      } else {
        let dedented = dedent_string(html_trim_preserve_indentation(value));
        for (index, line) in strings::split(&dedented, b"\n").enumerate() {
          write(self, index, line, Some(Separator::Hard));
        }
      }
    } else {
      for (index, word) in html_split(value).enumerate() {
        write(self, index, word, None);
      }
    }
    self.marker(|tags, out| tags.closing_tag_suffix(id, out));
    self.docs.close(part);
    self.docs.close(fill);
  }

  // ───────────────────────────── `print/children.js` ─────────────────────────────

  /// `getEndLocation`
  fn end_location(&self, id: Id) -> u32 {
    let node = &self.tree[id];
    let end = node.span.end;
    if node.kind == Kind::Element
      && node.end_span().is_none()
      && let Some(last) = self.tree.last_child(id)
    {
      return end.max(self.end_location(last));
    }
    end
  }

  fn print_child(&mut self, id: Id) {
    if !self.tree.has_prettier_ignore(id, self.options.ignore) {
      return self.print_nested(id);
    }
    let tags = self.tags();
    let node = &self.tree[id];
    let mut start = node.span.start as usize;
    if self
      .tree
      .prev(id)
      .is_some_and(|prev| tags.needs_to_borrow_next_opening_tag_start_marker(prev))
    {
      start += tags.len_of(|tags, out| tags.opening_tag_start_marker(id, out));
    }
    let mut end = self.end_location(id) as usize;
    if tags.lends_closing_tag_end_marker(id) {
      end -= tags.len_of(|tags, out| tags.closing_tag_end_marker(id, out));
    }
    let text = self.options.original_text.get(start..end).unwrap_or_default();
    let text = strings::trim_html_whitespace_end(text);
    self.marker(|tags, out| tags.opening_tag_prefix(id, out));
    self.lines(text, Separator::Literal);
    self.marker(|tags, out| tags.closing_tag_suffix(id, out));
  }

  fn between(&self, prev: Id, next: Id) -> Between {
    let tags = self.tags();
    let (before, after) = (&self.tree[prev], &self.tree[next]);
    let hard_leading = self.tree.prefer_hardline_as_leading_spaces(next, self.options);
    if is_text_like(before) && is_text_like(after) {
      if before.has(Flags::IS_TRAILING_SPACE_SENSITIVE) {
        if before.has(Flags::HAS_TRAILING_SPACES) {
          return if hard_leading { Between::Hard } else { Between::Line };
        }
        return Between::None;
      }
      return if hard_leading { Between::Hard } else { Between::Soft };
    }
    if (tags.needs_to_borrow_next_opening_tag_start_marker(prev)
      && (self.tree.has_prettier_ignore(next, self.options.ignore)
        || self.tree.first_child(next).is_some()
        || after.has(Flags::IS_SELF_CLOSING)
        || (after.kind == Kind::Element && after.has_attrs())))
      || (before.kind == Kind::Element
        && before.has(Flags::IS_SELF_CLOSING)
        && tags.needs_to_borrow_prev_closing_tag_end_marker(next))
    {
      return Between::None;
    }
    if after.kind == Kind::Comment
      && after.has(Flags::IS_LEADING_SPACE_SENSITIVE)
      && !after.has(Flags::HAS_LEADING_SPACES)
    {
      return Between::Soft;
    }
    let borrows_deep = || {
      tags.needs_to_borrow_prev_closing_tag_end_marker(next)
        && self.tree.last_child(prev).is_some_and(|last| {
          tags.needs_to_borrow_parent_closing_tag_start_marker(last)
            && self
              .tree
              .last_child(last)
              .is_some_and(|deep| tags.needs_to_borrow_parent_closing_tag_start_marker(deep))
        })
    };
    if !after.has(Flags::IS_LEADING_SPACE_SENSITIVE) || hard_leading || borrows_deep() {
      return Between::Hard;
    }
    if after.has(Flags::HAS_LEADING_SPACES) { Between::Line } else { Between::Soft }
  }

  fn emit_between(&mut self, between: Between) {
    match between {
      Between::None => {}
      Between::Hard => self.docs.hardline(),
      Between::Line => self.docs.line(),
      Between::Soft => self.docs.softline(),
    }
  }

  fn print_children(&mut self, id: Id) {
    if self.tree.force_break_children(id) {
      self.docs.break_parent();
      let mut next = self.tree.first_child(id);
      while let Some(child) = next {
        next = self.tree.next(child);
        if let Some(prev) = self.tree.prev(child) {
          let between = self.between(prev, child);
          if between != Between::None {
            self.emit_between(between);
            if self.tree.force_next_empty_line(prev, self.options) {
              self.docs.hardline();
            }
          }
        }
        self.print_child(child);
      }
      return;
    }

    // The group of the previous child, which `ifBreak` looks at.
    let mut previous = None;
    let mut next = self.tree.first_child(id);
    while let Some(child) = next {
      next = self.tree.next(child);
      let prev = self.tree.prev(child);
      let group = self.docs.group_id();
      let current = previous.replace(group);
      if is_text_like(&self.tree[child]) {
        if let Some(prev) = prev.filter(|&prev| is_text_like(&self.tree[prev])) {
          let between = self.between(prev, child);
          if between != Between::None {
            if self.tree.force_next_empty_line(prev, self.options) {
              self.docs.hardline();
              self.docs.hardline();
            } else {
              self.emit_between(between);
            }
          }
        }
        self.print_child(child);
        continue;
      }

      let mut before = 0;
      let mut leading = Leading::None;
      let mut trailing = Between::None;
      let mut after = 0;
      if let Some(prev) = prev {
        let between = self.between(prev, child);
        if between != Between::None {
          if self.tree.force_next_empty_line(prev, self.options) {
            before = 2;
          } else if between == Between::Hard {
            before = 1;
          } else if is_text_like(&self.tree[prev]) {
            leading = Leading::Line(between);
          } else {
            leading = Leading::Soft;
          }
        }
      }
      if let Some(next) = next {
        let between = self.between(child, next);
        if between != Between::None {
          let next_is_text = is_text_like(&self.tree[next]);
          if self.tree.force_next_empty_line(child, self.options) {
            after = if next_is_text { 2 } else { 0 };
          } else if between == Between::Hard {
            after = usize::from(next_is_text);
          } else {
            trailing = between;
          }
        }
      }

      for _ in 0..before {
        self.docs.hardline();
      }
      let outer = self.docs.group();
      match leading {
        Leading::None => {}
        Leading::Line(between) => self.emit_between(between),
        Leading::Soft => {
          let flat = self.docs.if_break(Branch::Flat, current);
          self.docs.softline();
          self.docs.close(flat);
        }
      }
      let inner = self.docs.group_with(Some(group), false);
      self.print_child(child);
      self.emit_between(trailing);
      self.docs.close(inner);
      self.docs.close(outer);
      for _ in 0..after {
        self.docs.hardline();
      }
    }
  }

  // ───────────────────────────── `print/element.js` ─────────────────────────────

  fn print_element(&mut self, id: Id) {
    let tags = self.tags();
    let node = &self.tree[id];
    if node.has(Flags::PRESERVES_CONTENT) {
      let group = self.docs.group();
      self.print_opening_tag(id);
      self.docs.close(group);
      let content = tags.node_content(id);
      self.lines(content, Separator::Literal);
      match tags.is_verbatim_without_end_tag(id) {
        true => self.marker(|tags, out| tags.closing_tag_suffix(id, out)),
        false => self.print_closing_tag(id),
      }
      return;
    }

    let first = self.tree.first_child(id);
    let last = self.tree.last_child(id);
    let hug = match (first, last) {
      (Some(first), Some(last)) if first == last => {
        let child = &self.tree[first];
        child.kind == Kind::Interpolation
          && child.flags & (Flags::IS_LEADING_SPACE_SENSITIVE | Flags::HAS_LEADING_SPACES)
            == Flags::IS_LEADING_SPACE_SENSITIVE
          && child.flags & (Flags::IS_TRAILING_SPACE_SENSITIVE | Flags::HAS_TRAILING_SPACES)
            == Flags::IS_TRAILING_SPACE_SENSITIVE
      }
      _ => false,
    };
    let attr_group = self.docs.group_id();

    // `printTag`: `group([group(printOpeningTag(..), { id }), contents, printClosingTag(..)])`
    let tag = self.docs.group();
    let opening = self.docs.group_with(Some(attr_group), false);
    self.print_opening_tag(id);
    self.docs.close(opening);

    match (first, last) {
      (Some(first), Some(last)) => {
        if self.tree.force_break_content(id, self.options) {
          self.docs.break_parent();
        }
        let indented = !hug && !self.is_unindented_vue_block(id);
        let container = if hug {
          Some(self.docs.indent_if_break(attr_group, false))
        } else if indented {
          Some(self.docs.indent())
        } else {
          None
        };
        let first_node = &self.tree[first];
        if hug {
          let broken = self.docs.if_break(Branch::Break, Some(attr_group));
          self.docs.softline();
          self.docs.close(broken);
        } else if first_node
          .flags
          .contains(Flags::HAS_LEADING_SPACES | Flags::IS_LEADING_SPACE_SENSITIVE)
        {
          self.docs.line();
        } else if first_node.kind == Kind::Text
          && node.flags.contains(Flags::IS_WHITESPACE_SENSITIVE | Flags::IS_INDENTATION_SENSITIVE)
        {
          let root = self.docs.align(Align::Root);
          self.docs.softline();
          self.docs.close(root);
        } else {
          self.docs.softline();
        }
        if indented {
          self.indents += 1;
        }
        self.print_children(id);
        if indented {
          self.indents -= 1;
        }
        if let Some(container) = container {
          self.docs.close(container);
        }
        self.line_after_children(id, last, hug, attr_group);
      }
      _ => {
        if node.flags.contains(Flags::HAS_DANGLING_SPACES | Flags::IS_DANGLING_SPACE_SENSITIVE) {
          self.docs.line();
        }
      }
    }
    self.print_closing_tag(id);
    self.docs.close(tag);
  }

  /// A Vue root `<script>`, `<style>` or custom block, whose contents aren't indented.
  fn is_unindented_vue_block(&self, id: Id) -> bool {
    let node = &self.tree[id];
    self.options.parser == Parser::Vue
      && !self.options.vue_indent_script_and_style
      && self.tree.parent(id) == Some(self.tree.root)
      && (is_script_like_tag(node, self.options) || self.tree.is_vue_custom_block(id, self.options))
  }

  fn line_after_children(&mut self, id: Id, last: Id, hug: bool, attr_group: crate::doc::GroupId) {
    let tags = self.tags();
    let node = &self.tree[id];
    let last_node = &self.tree[last];
    let trailing =
      last_node.flags.contains(Flags::HAS_TRAILING_SPACES | Flags::IS_TRAILING_SPACE_SENSITIVE);
    if tags.lends_closing_tag_end_marker(id) {
      if trailing {
        self.docs.text(" ");
      }
      return;
    }
    if is_pre_like(node) && tags.needs_to_borrow_parent_closing_tag_start_marker(last) {
      return;
    }
    if hug {
      let broken = self.docs.if_break(Branch::Break, Some(attr_group));
      self.docs.softline();
      return self.docs.close(broken);
    }
    if trailing {
      return self.docs.line();
    }
    let sensitive_text = last_node.kind == Kind::Text
      && node.flags.contains(Flags::IS_WHITESPACE_SENSITIVE | Flags::IS_INDENTATION_SENSITIVE);
    if (last_node.kind == Kind::Comment || sensitive_text)
      && ends_with_indent(
        &last_node.value,
        self.options.tab_width * self.ancestors.saturating_sub(1),
      )
    {
      return;
    }
    self.docs.softline();
  }

  // ───────────────────────────── `print/tag.js` ─────────────────────────────

  pub(crate) fn print_closing_tag(&mut self, id: Id) {
    self.marker(|tags, out| tags.closing_tag(id, out));
  }

  pub(crate) fn print_opening_tag(&mut self, id: Id) {
    self.marker(|tags, out| tags.opening_tag_start(id, out));
    self.print_attributes(id);
    if !self.tree[id].has(Flags::IS_SELF_CLOSING) {
      self.marker(|tags, out| tags.opening_tag_end(id, out));
    }
  }

  fn print_attributes(&mut self, id: Id) {
    let tags = self.tags();
    let node = &self.tree[id];
    let attrs = self.tree.attrs(id);
    if attrs.is_empty() {
      if node.has(Flags::IS_SELF_CLOSING) {
        self.docs.text(" ");
      }
      return;
    }
    let ignored = self
      .tree
      .prev_of(id)
      .filter(|prev| prev.kind == Kind::Comment)
      .and_then(|prev| ignore_attribute_data(&prev.value, self.options.ignore));

    let only_src = node.kind == Kind::Element
      && node.is_full_name(b"script")
      && attrs.len() == 1
      && attrs[0].is_full_name(b"src")
      && self.tree.first_child(id).is_none();
    let per_line = self.options.single_attribute_per_line
      && attrs.len() > 1
      && !self.tree.is_vue_sfc_block(id, self.options);
    // `indent([line, join(attributeLine, printedAttributes)])`
    let indent = self.docs.indent();
    if only_src {
      self.docs.text(" ");
    } else {
      self.docs.line();
    }
    for (index, attr) in attrs.iter().enumerate() {
      if index > 0 {
        if per_line {
          self.docs.hardline();
        } else {
          self.docs.line();
        }
      }
      let is_ignored = match &ignored {
        Some(Ignored::All) => true,
        Some(Ignored::Names(names)) => {
          let raw = raw_name(attr.raw_name());
          names.iter().any(|ignored| *ignored == &raw[..])
        }
        None => false,
      };
      if is_ignored {
        let span = attr.span;
        let text = &self.options.original_text[span.start as usize..span.end as usize];
        self.lines(text, Separator::Literal);
      } else {
        self.print_attribute(id, index);
      }
    }
    self.docs.close(indent);

    let self_closing = node.has(Flags::IS_SELF_CLOSING);
    if self
      .tree
      .first_child(id)
      .is_some_and(|first| tags.needs_to_borrow_parent_opening_tag_end_marker(first))
      || (self_closing
        && self
          .tree
          .parent(id)
          .is_some_and(|parent| tags.needs_to_borrow_last_child_closing_tag_end_marker(parent)))
      || only_src
      || self.options.bracket_same_line
    {
      if self_closing {
        self.docs.text(" ");
      }
    } else if self_closing {
      self.docs.line();
    } else {
      self.docs.softline();
    }
  }

  fn print_attribute(&mut self, id: Id, index: usize) {
    let attr = &self.tree.attrs(id)[index];
    let name = &attr.name[..];
    let embeds = self.options.parser == Parser::Vue
      || matches!(name, b"srcset" | b"allow" | b"style")
      || embed::is_event_handler(name);
    let written = self.written(attr);
    if let Some((text, size, printable)) = written
      && !embeds
      && (name != b"class" || (printable && attr.value.is_some_and(is_collapsed)))
    {
      return self.docs.sized(text, size);
    }
    if embed::attribute(self, id, index) {
      return;
    }
    if let Some((text, size, _)) = written {
      return self.docs.sized(text, size);
    }
    let raw = raw_name(attr.raw_name());
    let Some(value) = attr.value else {
      return self.bytes(&raw);
    };
    let value = unescape_quote_entities(value);
    let quote: &[u8] = if should_unquote_attribute_value(attr, self.options) {
      b""
    } else {
      preferred_quote(&value)
    };
    let escaped: Cow<[u8]> = match quote {
      b"\"" => replace(&value, b"\"", b"&quot;"),
      b"'" => replace(&value, b"'", b"&apos;"),
      _ => value,
    };
    self.bytes(&raw);
    self.docs.text("=");
    self.bytes(quote);
    self.lines(&escaped, Separator::Literal);
    self.bytes(quote);
  }

  /// The source of an attribute that the plain printer prints as it is written, its width, and whether its value is
  /// printable ASCII. That is a quoted value with no line break and nothing to unescape,
  /// whose quotes are the ones the printer prefers.
  fn written(&self, attr: &ast::Attribute<'_>) -> Option<(&'a str, u32, bool)> {
    let (value, span) = (attr.value?, attr.value_span?);
    let name = &attr.name[..];
    let start = attr.span.start as usize;
    let source = self.source.as_bytes();
    let quoted = attr.namespace.is_empty()
      && span.start as usize == start + name.len() + 1
      && span.len() as usize == value.len() + 2
      && source.get(start..start + name.len()) == Some(name)
      && source.get(span.start as usize - 1) == Some(&b'=');
    if !quoted {
      return None;
    }
    let scan = scan(value);
    let quote = source[span.start as usize];
    let preferred = quote == b'"' || (quote == b'\'' && scan.double);
    let escapes = scan.amp
      && (memchr::memmem::find(value, b"&quot;").is_some()
        || memchr::memmem::find(value, b"&apos;").is_some());
    if scan.newline || escapes || !preferred {
      return None;
    }
    let text = &self.source[start..span.end as usize];
    let size = match scan.printable && name.is_ascii() {
      true => text.len(),
      false => crate::doc::width(text),
    };
    Some((text, size as u32, scan.printable))
  }

  // ───────────────────────────── `embed.js` ─────────────────────────────

  /// Emits `id` with an embedded printer, if one applies and its code is formatted.
  fn print_embedded(&mut self, id: Id) -> bool {
    let node = &self.tree[id];
    match node.kind {
      Kind::Element => embed::element(self, id),
      Kind::Text => {
        let Some(parent) = self.tree.parent(id) else { return false };
        if is_script_like_tag(&self.tree[parent], self.options) {
          return embed::script(self, id, parent);
        }
        if self.tree[parent].kind == Kind::Interpolation {
          return embed::interpolation(self, id, parent);
        }
        false
      }
      _ => false,
    }
  }

  /// Formats `code` with the host's formatter for its language.
  pub(crate) fn embed_text(&mut self, embedded: Embedded<'_>) -> Option<String> {
    let mut formatted = self.embed.format(embedded.request(self.indents, self.options))?;
    formatted.truncate(formatted.trim_end_matches(['\n', '\r']).len());
    Some(formatted)
  }

  /// Emits a formatted text, its lines joined by literal lines.
  pub(crate) fn text_doc(&mut self, text: &str) {
    self.lines(text.as_bytes(), Separator::Literal)
  }
}

/// What comes before a child's group in `printChildren`.
#[derive(Clone, Copy)]
enum Leading {
  None,
  Line(Between),
  /// `ifBreak("", softline, { groupId })` of the previous child's group.
  Soft,
}

#[derive(Clone, Copy)]
pub(crate) enum Separator {
  Literal,
  Hard,
}

enum Ignored<'a> {
  All,
  Names(Vec<&'a [u8]>),
}

/// `getPrettierIgnoreAttributeCommentData`
fn ignore_attribute_data<'v>(value: &'v [u8], ignore: &[u8]) -> Option<Ignored<'v>> {
  let value = strings::trim_js_whitespace(value);
  let rest = value.strip_prefix(ignore)?.strip_prefix(b"-attribute")?;
  if rest.is_empty() {
    return Some(Ignored::All);
  }
  let first = strings::js_whitespace_len(rest);
  if first == 0 {
    return None;
  }
  let names: Vec<&[u8]> =
    rest.split(|byte| byte.is_ascii_whitespace()).filter(|name| !name.is_empty()).collect();
  if names.is_empty() { Some(Ignored::All) } else { Some(Ignored::Names(names)) }
}

pub(crate) fn raw_name<'t>((namespace, name): (&'t [u8], &'t [u8])) -> Cow<'t, [u8]> {
  if namespace.is_empty() {
    Cow::Borrowed(name)
  } else {
    Cow::Owned([namespace, b":", name].concat())
  }
}

/// What [`Printer::written`] needs to know of an attribute value.
#[derive(Clone, Copy)]
struct Scan {
  amp: bool,
  newline: bool,
  double: bool,
  printable: bool,
}

/// Scans an attribute value in one pass that the compiler can vectorize.
fn scan(value: &[u8]) -> Scan {
  let (mut amp, mut newline, mut double, mut other) = (0u8, 0u8, 0u8, 0u8);
  let mut add = |bytes: &[u8]| {
    for &byte in bytes {
      amp |= u8::from(byte == b'&');
      newline |= u8::from(byte == b'\n');
      double |= u8::from(byte == b'"');
      other |= u8::from(byte.wrapping_sub(0x20) >= 0x60);
    }
  };
  let (chunks, rest) = value.as_chunks::<16>();
  chunks.iter().for_each(|chunk| add(chunk));
  add(rest);
  Scan { amp: amp != 0, newline: newline != 0, double: double != 0, printable: other == 0 }
}

/// True if trimming the printable ASCII class names and joining them with single spaces gives them back.
fn is_collapsed(value: &[u8]) -> bool {
  !value.starts_with(b" ")
    && !value.ends_with(b" ")
    && !value
      .iter()
      .zip(&value[1.min(value.len())..])
      .fold(false, |double, (&a, &b)| double | ((a == b' ') & (b == b' ')))
}

/// `getPreferredQuote(text, '"')`
pub(crate) fn preferred_quote(text: &[u8]) -> &'static [u8] {
  let doubles = text.iter().filter(|&&byte| byte == b'"').count();
  let singles = text.iter().filter(|&&byte| byte == b'\'').count();
  if doubles > singles { b"'" } else { b"\"" }
}

pub(crate) fn replace<'t>(text: &'t [u8], from: &[u8], to: &[u8]) -> Cow<'t, [u8]> {
  if strings::index_of(text, from).is_none() {
    return Cow::Borrowed(text);
  }
  let mut out = Vec::with_capacity(text.len() + 8);
  let mut rest = text;
  while let Some(at) = strings::index_of(rest, from) {
    out.extend_from_slice(&rest[..at]);
    out.extend_from_slice(to);
    rest = &rest[at + from.len()..];
  }
  out.extend_from_slice(rest);
  Cow::Owned(out)
}

/// `value.replace(/^html\b/i, "html").replaceAll(/\s+/g, " ")`
fn doc_type_value(value: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(value.len());
  let lowered = value.len() >= 4
    && value[..4].eq_ignore_ascii_case(b"html")
    && value.get(4).is_none_or(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_'));
  let rest = if lowered {
    out.extend_from_slice(b"html");
    &value[4..]
  } else {
    value
  };
  let mut rest = rest;
  while !rest.is_empty() {
    let len = strings::js_whitespace_len(rest);
    if len > 0 {
      while let len @ 1.. = strings::js_whitespace_len(rest) {
        rest = &rest[len..];
      }
      out.push(b' ');
    } else {
      out.push(rest[0]);
      rest = &rest[1..];
    }
  }
  out
}

/// Where `/\n[^\S\n]*$/` matches.
fn trailing_newline(value: &[u8]) -> Option<usize> {
  let at = value.iter().rposition(|&byte| byte == b'\n')?;
  value[at + 1..].iter().all(|byte| byte.is_ascii_whitespace() && *byte != b'\n').then_some(at)
}

/// `/\n[\t ]{width}$/.test(value)`
fn ends_with_indent(value: &[u8], width: usize) -> bool {
  value.len() > width
    && value[value.len() - width - 1] == b'\n'
    && value[value.len() - width..].iter().all(|byte| matches!(byte, b'\t' | b' '))
}
