//! Prettier's `printer-html.js`, `print/children.js`, `print/element.js` and `print/tag.js`, building a [`Doc`].

use std::borrow::Cow;

use super::ast::{Flags, Id, Kind, Tree};
use super::embed::{self, Embedded};
use super::strings;
use super::tag::Tags;
use super::utilities::{
  dedent_string, html_split, html_trim_preserve_indentation, is_pre_like, is_script_like_tag,
  is_text_like, should_unquote_attribute_value, unescape_quote_entities,
};
use super::{Options, Parser};
use crate::doc::{Align, Doc, Docs};
use crate::embed::Embed;

pub(crate) struct Printer<'p, 'a> {
  /// The text that is formatted, which texts that are part of it borrow from.
  source: &'a str,
  /// A buffer for the markers of tags.
  buffer: Vec<u8>,
  /// Whether texts escape `"` as `&quot;`, as in the value of an attribute.
  pub(crate) quotes: bool,
  /// Parts of the docs being built, shared so that building a list needs no allocation.
  stack: Vec<Doc>,
  pub(crate) tree: &'p Tree<'a>,
  pub(crate) options: &'p Options<'p>,
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
    options: &'p Options<'p>,
    docs: &'p mut Docs<'a>,
    embed: &'p mut dyn Embed,
  ) -> Self {
    Self {
      source,
      buffer: Vec::new(),
      quotes: false,
      stack: Vec::new(),
      tree,
      options,
      docs,
      embed,
      indents: 0,
      ancestors: 0,
    }
  }

  fn tags(&self) -> Tags<'p, 'a, 'p> {
    Tags { tree: self.tree, options: self.options }
  }

  /// A text that the arena keeps.
  pub(crate) fn bytes(&mut self, bytes: &[u8]) -> Doc {
    if self.quotes && strings::contains_char(bytes, b'"') {
      let escaped = replace(bytes, b"\"", b"&quot;");
      self.quotes = false;
      let doc = self.bytes(&escaped);
      self.quotes = true;
      return doc;
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

  /// A string that `write` builds from the tag markers.
  fn marker(&mut self, write: impl FnOnce(Tags<'p, 'a, 'p>, &mut Vec<u8>)) -> Doc {
    let mut out = std::mem::take(&mut self.buffer);
    out.clear();
    write(self.tags(), &mut out);
    let doc = match std::str::from_utf8(&out) {
      Ok(text) => self.docs.owned(text),
      Err(_) => self.docs.owned(&String::from_utf8_lossy(&out)),
    };
    self.buffer = out;
    doc
  }

  /// `replaceEndOfLine(text, separator)`
  pub(crate) fn lines(&mut self, text: &[u8], separator: Separator) -> Doc {
    if !strings::contains_char(text, b'\n') {
      return self.bytes(text);
    }
    let parts = self.line_parts(text, separator);
    self.docs.concat(&parts)
  }

  fn line_parts(&mut self, text: &[u8], separator: Separator) -> Vec<Doc> {
    let mut parts = Vec::new();
    for (index, line) in strings::split(text, b"\n").enumerate() {
      if index > 0 {
        let separator = match separator {
          Separator::Literal => self.docs.literalline(),
          Separator::Hard => self.docs.hardline(),
        };
        parts.push(separator);
      }
      parts.push(self.bytes(line));
    }
    parts
  }

  // ───────────────────────────── `genericPrint` ─────────────────────────────

  pub(crate) fn print_root(&mut self) -> Doc {
    let root = self.tree.root;
    self.ancestors = 0;
    let children = self.print_children(root);
    let group = self.docs.group(children);
    let hardline = self.docs.hardline();
    self.docs.concat(&[group, hardline])
  }

  /// `path.call(print)` for `id`: the embedded printer first, then `genericPrint`.
  fn print(&mut self, id: Id) -> Doc {
    if let Some(doc) = self.print_embedded(id) {
      return doc;
    }
    let node = &self.tree[id];
    match node.kind {
      Kind::Element | Kind::IeConditionalComment => self.print_element(id),
      Kind::IeConditionalStartComment | Kind::IeConditionalEndComment => {
        let start = self.marker(|tags, out| tags.opening_tag_start(id, out));
        let end = self.marker(|tags, out| tags.closing_tag_end(id, out));
        self.docs.concat(&[start, end])
      }
      Kind::Interpolation => {
        let mut parts = vec![self.marker(|tags, out| tags.opening_tag_start(id, out))];
        let mut next = self.tree.first_child(id);
        while let Some(child) = next {
          next = self.tree.next(child);
          parts.push(self.print_nested(child));
        }
        parts.push(self.marker(|tags, out| tags.closing_tag_end(id, out)));
        self.docs.concat(&parts)
      }
      Kind::Text => self.print_text(id),
      Kind::DocType => {
        let start = self.marker(|tags, out| tags.opening_tag_start(id, out));
        let space = self.docs.text(" ");
        let value = doc_type_value(&node.value);
        let value = self.bytes(&value);
        let inner = self.docs.concat(&[start, space, value]);
        let group = self.docs.group(inner);
        let end = self.marker(|tags, out| tags.closing_tag_end(id, out));
        self.docs.concat(&[group, end])
      }
      Kind::Comment => {
        let prefix = self.marker(|tags, out| tags.opening_tag_prefix(id, out));
        let span = node.span;
        let text = &self.options.original_text[span.start as usize..span.end as usize];
        let value = self.lines(text, Separator::Literal);
        let suffix = self.marker(|tags, out| tags.closing_tag_suffix(id, out));
        self.docs.concat(&[prefix, value, suffix])
      }
      Kind::FrontMatter => {
        let span = node.span;
        let raw = &self.options.original_text[span.start as usize..span.end as usize];
        self.print_front_matter(raw)
      }
      _ => Docs::EMPTY,
    }
  }

  /// Prints a child, one level deeper.
  fn print_nested(&mut self, id: Id) -> Doc {
    self.ancestors += 1;
    let doc = self.print(id);
    self.ancestors -= 1;
    doc
  }

  fn print_text(&mut self, id: Id) -> Doc {
    let node = &self.tree[id];
    let parent = self.tree.parent(id).map(|parent| &self.tree[parent]);
    if parent.is_some_and(|parent| parent.kind == Kind::Interpolation) {
      let value = &node.value[..];
      let trailing = trailing_newline(value);
      let value = trailing.map_or(value, |at| &value[..at]);
      let printed = self.lines(value, Separator::Literal);
      let end = if trailing.is_some() { self.docs.hardline() } else { Docs::EMPTY };
      return self.docs.concat(&[printed, end]);
    }
    let prefix = self.marker(|tags, out| tags.opening_tag_prefix(id, out));
    let mut parts = self.text_value_parts(id, &self.tree[id].value);
    let suffix = self.marker(|tags, out| tags.closing_tag_suffix(id, out));
    parts[0] = self.docs.concat(&[prefix, parts[0]]);
    let last = parts.len() - 1;
    parts[last] = self.docs.concat(&[parts[last], suffix]);
    self.docs.fill(&parts)
  }

  /// `getTextValueParts`, as the parts of a fill.
  fn text_value_parts(&mut self, id: Id, value: &[u8]) -> Vec<Doc> {
    let parent = self.tree.parent(id).map_or(self.tree.root, |parent| parent);
    let parent = &self.tree[parent];
    if parent.has(Flags::IS_WHITESPACE_SENSITIVE) {
      if parent.has(Flags::IS_INDENTATION_SENSITIVE) {
        return self.line_parts(value, Separator::Literal);
      }
      let dedented = dedent_string(html_trim_preserve_indentation(value));
      return self.line_parts(&dedented, Separator::Hard);
    }
    let words: Vec<&[u8]> = html_split(value).collect();
    let mut parts = Vec::with_capacity(words.len() * 2);
    for (index, word) in words.into_iter().enumerate() {
      if index > 0 {
        parts.push(self.docs.line());
      }
      parts.push(self.bytes(word));
    }
    parts
  }

  fn print_front_matter(&mut self, raw: &[u8]) -> Doc {
    if let Some(doc) = embed::front_matter(self, raw) {
      return doc;
    }
    self.lines(raw, Separator::Literal)
  }

  // ───────────────────────────── `print/children.js` ─────────────────────────────

  /// `getEndLocation`
  fn end_location(&self, id: Id) -> u32 {
    let node = &self.tree[id];
    let end = node.span.end;
    if node.kind == Kind::Element && node.end_span().is_none() {
      if let Some(last) = self.tree.last_child(id) {
        return end.max(self.end_location(last));
      }
    }
    end
  }

  fn print_child(&mut self, id: Id) -> Doc {
    if !self.tree.has_prettier_ignore(id) {
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
    if self.tree.next(id).is_some_and(|next| tags.needs_to_borrow_prev_closing_tag_end_marker(next))
    {
      end -= tags.len_of(|tags, out| tags.closing_tag_end_marker(id, out));
    }
    let text = self.options.original_text.get(start..end).unwrap_or_default();
    let text = strings::trim_html_whitespace_end(text);
    let prefix = self.marker(|tags, out| tags.opening_tag_prefix(id, out));
    let value = self.lines(text, Separator::Literal);
    let suffix = self.marker(|tags, out| tags.closing_tag_suffix(id, out));
    self.docs.concat(&[prefix, value, suffix])
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
      && (self.tree.has_prettier_ignore(next)
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

  fn between_doc(&mut self, between: Between) -> Doc {
    match between {
      Between::None => Docs::EMPTY,
      Between::Hard => self.docs.hardline(),
      Between::Line => self.docs.line(),
      Between::Soft => self.docs.softline(),
    }
  }

  fn print_children(&mut self, id: Id) -> Doc {
    let mark = self.stack.len();
    if self.tree.force_break_children(id) {
      self.stack.push(self.docs.break_parent());
      let mut next = self.tree.first_child(id);
      while let Some(child) = next {
        next = self.tree.next(child);
        if let Some(prev) = self.tree.prev(child) {
          let between = self.between(prev, child);
          if between != Between::None {
            let line = self.between_doc(between);
            self.stack.push(line);
            if self.tree.force_next_empty_line(prev, self.options) {
              self.stack.push(self.docs.hardline());
            }
          }
        }
        let printed = self.print_child(child);
        self.stack.push(printed);
      }
      return self.concat_from(mark);
    }

    // The group of the previous child, which `ifBreak` looks at.
    let mut previous = None;
    let mut next = self.tree.first_child(id);
    while let Some(child) = next {
      next = self.tree.next(child);
      let prev = self.tree.prev(child);
      let group = self.docs.group_id();
      let current = std::mem::replace(&mut previous, Some(group));
      if is_text_like(&self.tree[child]) {
        if let Some(prev) = prev.filter(|&prev| is_text_like(&self.tree[prev])) {
          let between = self.between(prev, child);
          if between != Between::None {
            let printed = self.print_child(child);
            let doc = if self.tree.force_next_empty_line(prev, self.options) {
              let hard = self.docs.hardline();
              self.docs.concat(&[hard, hard, printed])
            } else {
              let line = self.between_doc(between);
              self.docs.concat(&[line, printed])
            };
            self.stack.push(doc);
            continue;
          }
        }
        let printed = self.print_child(child);
        self.stack.push(printed);
        continue;
      }

      let mut before = [Docs::EMPTY; 2];
      let mut leading = Docs::EMPTY;
      let mut trailing = Docs::EMPTY;
      let mut after = [Docs::EMPTY; 2];
      if let Some(prev) = prev {
        let between = self.between(prev, child);
        if between != Between::None {
          if self.tree.force_next_empty_line(prev, self.options) {
            before = [self.docs.hardline(); 2];
          } else if between == Between::Hard {
            before[0] = self.docs.hardline();
          } else if is_text_like(&self.tree[prev]) {
            leading = self.between_doc(between);
          } else {
            let soft = self.docs.softline();
            leading = self.docs.if_break(Docs::EMPTY, soft, current);
          }
        }
      }
      if let Some(next) = next {
        let between = self.between(child, next);
        if between != Between::None {
          let next_is_text = is_text_like(&self.tree[next]);
          if self.tree.force_next_empty_line(child, self.options) {
            if next_is_text {
              after = [self.docs.hardline(); 2];
            }
          } else if between == Between::Hard {
            if next_is_text {
              after[0] = self.docs.hardline();
            }
          } else {
            trailing = self.between_doc(between);
          }
        }
      }

      let printed = self.print_child(child);
      let inner = self.docs.concat(&[printed, trailing]);
      let inner = self.docs.group_with(inner, Some(group), false);
      let outer = self.docs.concat(&[leading, inner]);
      let outer = self.docs.group(outer);
      let doc = self.docs.concat(&[before[0], before[1], outer, after[0], after[1]]);
      self.stack.push(doc);
    }
    self.concat_from(mark)
  }

  /// The concatenation of what was pushed onto the stack since `mark`, which it takes off again.
  fn concat_from(&mut self, mark: usize) -> Doc {
    let doc = self.docs.concat(&self.stack[mark..]);
    self.stack.truncate(mark);
    doc
  }

  // ───────────────────────────── `print/element.js` ─────────────────────────────

  fn print_element(&mut self, id: Id) -> Doc {
    let tags = self.tags();
    let node = &self.tree[id];
    if node.has(Flags::PRESERVES_CONTENT) {
      let prefix = self.marker(|tags, out| tags.opening_tag_prefix(id, out));
      let opening = self.print_opening_tag(id);
      let opening = self.docs.group(opening);
      let content = tags.node_content(id);
      let content = self.lines(content, Separator::Literal);
      let closing = self.print_closing_tag(id);
      let suffix = self.marker(|tags, out| tags.closing_tag_suffix(id, out));
      return self.docs.concat(&[prefix, opening, content, closing, suffix]);
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

    let (Some(first), Some(last)) = (first, last) else {
      let contents =
        if node.flags.contains(Flags::HAS_DANGLING_SPACES | Flags::IS_DANGLING_SPACE_SENSITIVE) {
          self.docs.line()
        } else {
          Docs::EMPTY
        };
      return self.print_tag(id, attr_group, contents);
    };

    let first_node = &self.tree[first];
    let line_before = if hug {
      let soft = self.docs.softline();
      self.docs.if_break(soft, Docs::EMPTY, Some(attr_group))
    } else if first_node
      .flags
      .contains(Flags::HAS_LEADING_SPACES | Flags::IS_LEADING_SPACE_SENSITIVE)
    {
      self.docs.line()
    } else if first_node.kind == Kind::Text
      && node.flags.contains(Flags::IS_WHITESPACE_SENSITIVE | Flags::IS_INDENTATION_SENSITIVE)
    {
      let soft = self.docs.softline();
      self.docs.align(Align::Root, soft)
    } else {
      self.docs.softline()
    };

    let indented = !hug && !self.is_unindented_vue_block(id);
    if indented {
      self.indents += 1;
    }
    let children = self.print_children(id);
    if indented {
      self.indents -= 1;
    }
    let children = self.docs.concat(&[line_before, children]);
    let children = if hug {
      self.docs.indent_if_break(children, attr_group, false)
    } else if indented {
      self.docs.indent(children)
    } else {
      children
    };

    let line_after = self.line_after_children(id, last, hug, attr_group);
    let force = if self.tree.force_break_content(id, self.options) {
      self.docs.break_parent()
    } else {
      Docs::EMPTY
    };
    let contents = self.docs.concat(&[force, children, line_after]);
    self.print_tag(id, attr_group, contents)
  }

  /// A Vue root `<script>`, `<style>` or custom block, whose contents aren't indented.
  fn is_unindented_vue_block(&self, id: Id) -> bool {
    let node = &self.tree[id];
    self.options.parser == Parser::Vue
      && !self.options.vue_indent_script_and_style
      && self.tree.parent(id) == Some(self.tree.root)
      && (is_script_like_tag(node, self.options) || self.tree.is_vue_custom_block(id, self.options))
  }

  fn line_after_children(
    &mut self,
    id: Id,
    last: Id,
    hug: bool,
    attr_group: crate::doc::GroupId,
  ) -> Doc {
    let tags = self.tags();
    let node = &self.tree[id];
    let last_node = &self.tree[last];
    let trailing =
      last_node.flags.contains(Flags::HAS_TRAILING_SPACES | Flags::IS_TRAILING_SPACE_SENSITIVE);
    if tags.lends_closing_tag_end_marker(id) {
      return if trailing { self.docs.text(" ") } else { Docs::EMPTY };
    }
    if is_pre_like(node) && tags.needs_to_borrow_parent_closing_tag_start_marker(last) {
      return Docs::EMPTY;
    }
    if hug {
      let soft = self.docs.softline();
      return self.docs.if_break(soft, Docs::EMPTY, Some(attr_group));
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
      return Docs::EMPTY;
    }
    self.docs.softline()
  }

  fn print_tag(&mut self, id: Id, attr_group: crate::doc::GroupId, contents: Doc) -> Doc {
    let opening = self.print_opening_tag(id);
    let opening = self.docs.group_with(opening, Some(attr_group), false);
    let closing = self.print_closing_tag(id);
    let inner = self.docs.concat(&[opening, contents, closing]);
    self.docs.group(inner)
  }

  // ───────────────────────────── `print/tag.js` ─────────────────────────────

  pub(crate) fn print_closing_tag(&mut self, id: Id) -> Doc {
    self.marker(|tags, out| tags.closing_tag(id, out))
  }

  pub(crate) fn print_opening_tag(&mut self, id: Id) -> Doc {
    let start = self.marker(|tags, out| tags.opening_tag_start(id, out));
    let attributes = self.print_attributes(id);
    let end = if self.tree[id].has(Flags::IS_SELF_CLOSING) {
      Docs::EMPTY
    } else {
      self.marker(|tags, out| tags.opening_tag_end(id, out))
    };
    self.docs.concat(&[start, attributes, end])
  }

  fn print_attributes(&mut self, id: Id) -> Doc {
    let tags = self.tags();
    let node = &self.tree[id];
    let attrs = self.tree.attrs(id);
    if attrs.is_empty() {
      return if node.has(Flags::IS_SELF_CLOSING) { self.docs.text(" ") } else { Docs::EMPTY };
    }
    let ignored = self
      .tree
      .prev_of(id)
      .filter(|prev| prev.kind == Kind::Comment)
      .and_then(|prev| ignore_attribute_data(&prev.value));

    let only_src = node.kind == Kind::Element
      && node.is_full_name(b"script")
      && attrs.len() == 1
      && attrs[0].is_full_name(b"src")
      && self.tree.first_child(id).is_none();
    let per_line = self.options.single_attribute_per_line
      && attrs.len() > 1
      && !self.tree.is_vue_sfc_block(id, self.options);
    let separator = if per_line { self.docs.hardline() } else { self.docs.line() };
    // `indent([line, join(attributeLine, printedAttributes)])`, as one list.
    let mark = self.stack.len();
    self.stack.push(if only_src { self.docs.text(" ") } else { self.docs.line() });
    for index in 0..attrs.len() {
      if index > 0 {
        self.stack.push(separator);
      }
      let attr = &self.tree.attrs(id)[index];
      let is_ignored = match &ignored {
        Some(Ignored::All) => true,
        Some(Ignored::Names(names)) => {
          let (namespace, name) = attr.raw_name();
          let raw: Cow<[u8]> = if namespace.is_empty() {
            Cow::Borrowed(name)
          } else {
            Cow::Owned([namespace, b":", name].concat())
          };
          names.iter().any(|ignored| *ignored == &raw[..])
        }
        None => false,
      };
      let doc = if is_ignored {
        let span = attr.span;
        let text = &self.options.original_text[span.start as usize..span.end as usize];
        self.lines(text, Separator::Literal)
      } else {
        self.print_attribute(id, index)
      };
      self.stack.push(doc);
    }
    let lead = self.concat_from(mark);
    let indented = self.docs.indent(lead);

    let self_closing = node.has(Flags::IS_SELF_CLOSING);
    let end = if self
      .tree
      .first_child(id)
      .is_some_and(|first| tags.needs_to_borrow_parent_opening_tag_end_marker(first))
      || (self_closing
        && self
          .tree
          .parent(id)
          .is_some_and(|parent| tags.needs_to_borrow_last_child_closing_tag_end_marker(parent)))
      || only_src
    {
      if self_closing { self.docs.text(" ") } else { Docs::EMPTY }
    } else if self.options.bracket_same_line {
      if self_closing { self.docs.text(" ") } else { Docs::EMPTY }
    } else if self_closing {
      self.docs.line()
    } else {
      self.docs.softline()
    };
    self.docs.concat(&[indented, end])
  }

  fn print_attribute(&mut self, id: Id, index: usize) -> Doc {
    if let Some(doc) = embed::attribute(self, id, index) {
      return doc;
    }
    let attr = &self.tree.attrs(id)[index];
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
    let name = self.bytes(&raw);
    let equals = self.docs.text("=");
    let quote = self.bytes(quote);
    let value = self.lines(&escaped, Separator::Literal);
    self.docs.concat(&[name, equals, quote, value, quote])
  }

  // ───────────────────────────── `embed.js` ─────────────────────────────

  fn print_embedded(&mut self, id: Id) -> Option<Doc> {
    let node = &self.tree[id];
    match node.kind {
      Kind::Element => embed::element(self, id),
      Kind::Text => {
        let parent = self.tree.parent(id)?;
        if is_script_like_tag(&self.tree[parent], self.options) {
          return embed::script(self, id, parent);
        }
        if self.tree[parent].kind == Kind::Interpolation {
          return embed::interpolation(self, id, parent);
        }
        None
      }
      _ => None,
    }
  }

  /// Formats `code` with the host's formatter for `language`, as a doc.
  pub(crate) fn embedded(&mut self, embedded: Embedded<'_>) -> Option<Doc> {
    let formatted = self.embed_text(embedded)?;
    Some(self.text_doc(&formatted))
  }

  /// Formats `code` with the host's formatter for `language`.
  pub(crate) fn embed_text(&mut self, embedded: Embedded<'_>) -> Option<String> {
    let mut formatted = self.embed.format(embedded.request(self.indents, self.options))?;
    formatted.truncate(formatted.trim_end_matches(['\n', '\r']).len());
    Some(formatted)
  }

  /// A formatted text, its lines joined by literal lines.
  pub(crate) fn text_doc(&mut self, text: &str) -> Doc {
    self.lines(text.as_bytes(), Separator::Literal)
  }
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
fn ignore_attribute_data(value: &[u8]) -> Option<Ignored<'_>> {
  let value = strings::trim_js_whitespace(value);
  let rest = value.strip_prefix(b"prettier-ignore-attribute")?;
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
