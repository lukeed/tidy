//! The printer of `@prettier/plugin-xml` (`src/printer.js` and `src/embed.js`).

use super::ast::{Attribute, Doctype, Element, External, Item, Prolog, Span, Top, Tree};
use super::{Options, Quotes, sort};
use crate::Whitespace;
use crate::doc::{Align, Docs};
use crate::embed::{Embed, Kind as Purpose, Request};
use crate::stack::Stack;
use crate::strings;

pub(crate) struct Printer<'p, 'a> {
  pub(crate) tree: &'p Tree,
  pub(crate) text: &'a str,
  pub(crate) options: &'p Options,
  pub(crate) docs: &'p mut Docs<'a>,
  pub(crate) embed: &'p mut dyn Embed,
  pub(crate) stack: Stack,
  /// The text has a `<!-- prettier-ignore-start -->`.
  pub(crate) has_ignored_ranges: bool,
  /// The comments that start and end an ignored range: `<!-- prettier-ignore-start -->` by default.
  pub(crate) ignore: (&'p str, &'p str),
  pub(crate) scripts: &'p crate::embed::Scripts,
}

/// What comes before the `/>` of an element with no content.
#[derive(Clone, Copy)]
enum Space {
  Text(&'static str),
  Line,
  Soft,
}

impl<'a> Printer<'_, 'a> {
  /// `printDocument`: the parts of the document, each on its own line.
  pub(crate) fn document(&mut self) {
    for (index, top) in self.tree.top.iter().enumerate() {
      if index > 0 {
        self.docs.hardline();
      }
      match *top {
        Top::Prolog(prolog) => self.prolog(prolog),
        Top::Doctype(doctype) => self.doctype(doctype),
        Top::Item(item) => self.item(item),
      }
    }
    self.docs.hardline();
  }

  fn prolog(&mut self, Prolog { open, attributes }: Prolog) {
    let group = self.docs.group();
    self.span(open);
    let indent = self.docs.indent();
    self.docs.softline();
    for (index, attribute) in self.tree.attributes(attributes).iter().enumerate() {
      if index > 0 {
        self.docs.line();
      }
      self.attribute(attribute);
    }
    self.docs.close(indent);
    match self.options.self_closing_space {
      true => self.docs.line(),
      false => self.docs.softline(),
    }
    self.docs.text("?>");
    self.docs.close(group);
  }

  fn doctype(&mut self, Doctype { name, external }: Doctype) {
    let group = self.docs.group();
    self.docs.text("<!DOCTYPE");
    self.docs.text(" ");
    self.span(name);
    if let Some(external) = external {
      self.docs.text(" ");
      let group = self.docs.group();
      match external {
        External::System { keyword, system } => {
          self.span(keyword);
          self.indented_line(system);
        }
        External::Public { keyword, public, system } => {
          let inner = self.docs.group();
          self.span(keyword);
          self.indented_line(public);
          self.docs.close(inner);
          self.indented_line(system);
        }
      }
      self.docs.close(group);
    }
    self.docs.text(">");
    self.docs.close(group);
  }

  fn indented_line(&mut self, span: Span) {
    let indent = self.docs.indent();
    self.docs.line();
    self.span(span);
    self.docs.close(indent);
  }

  fn item(&mut self, item: Item) {
    match item {
      Item::Element(id) => self.element(id),
      Item::Text(span) | Item::Space(span) => self.lines(span.of(self.text)),
      Item::Reference(span) | Item::CData(span) | Item::Comment(span) | Item::Instruction(span) => {
        self.span(span)
      }
    }
  }

  // ───────────────────────────── `printElement` ─────────────────────────────

  fn element(&mut self, id: u32) {
    if !self.stack.has_room() {
      return;
    }
    let element = self.tree.elements[id as usize];
    if self.options.format_embedded && self.embedded(&element) {
      return;
    }
    let Some(content) = element.content else { return self.self_closed(&element) };
    let items = self.tree.content(content);
    if items.is_empty() {
      return self.self_closed(&element);
    }
    if !self.is_whitespace_ignorable(&element, items) {
      let group = self.docs.group();
      self.open_tag(&element);
      let indent = self.docs.indent();
      self.content(items);
      self.docs.close(indent);
      self.close_tag(&element);
      return self.docs.close(group);
    }
    if self.options.whitespace == Whitespace::Auto
      && items.iter().any(|item| matches!(item, Item::Text(_)))
    {
      let group = self.docs.group();
      self.open_tag(&element);
      for &item in items {
        self.item(item);
      }
      self.close_tag(&element);
      return self.docs.close(group);
    }
    self.fragments(&element, items);
  }

  /// The content of an element whose whitespace can be ignored, each part on its own line unless it only has text
  /// and references.
  fn fragments(&mut self, element: &Element, items: &[Item]) {
    let fragments: Vec<Item> =
      items.iter().copied().filter(|item| !matches!(item, Item::Space(_))).collect();
    let texts = fragments.iter().filter(|item| matches!(item, Item::Text(_))).count();
    let references = fragments.iter().filter(|item| matches!(item, Item::Reference(_))).count();
    let group = self.docs.group();
    match fragments[..] {
      [] => {
        self.docs.close(group);
        return self.self_closed(element);
      }
      [only] if texts == 1 => {
        self.open_tag(element);
        let indent = self.docs.indent();
        self.docs.softline();
        self.fragment(only);
        self.docs.close(indent);
        self.docs.softline();
      }
      _ => {
        let spaced = fragments.len() == texts + references;
        self.open_tag(element);
        let indent = self.docs.indent();
        self.docs.hardline();
        let mut last = 0;
        for (index, &fragment) in fragments.iter().enumerate() {
          let span = fragment.span(self.tree);
          if index > 0 {
            // chevrotain's lines: a fragment starts two lines after the last character of the one before.
            let between = &self.text.as_bytes()[last as usize..span.start as usize];
            if memchr::memchr_iter(b'\n', between).nth(1).is_some() {
              self.docs.hardline();
              self.docs.hardline();
            } else if spaced {
              self.docs.text(" ");
            } else {
              self.docs.hardline();
            }
          }
          self.fragment(fragment);
          last = span.end.saturating_sub(1).max(span.start);
        }
        self.docs.close(indent);
        self.docs.hardline();
      }
    }
    self.close_tag(element);
    self.docs.close(group);
  }

  fn fragment(&mut self, item: Item) {
    match item {
      Item::Text(span) => self.words(span),
      item => self.item(item),
    }
  }

  /// `printCharDataIgnore`: the text, trimmed, its words filled. The plugin also trims what JavaScript counts as
  /// whitespace, such as a no-break space, which XML doesn't.
  fn words(&mut self, span: Span) {
    let trimmed = span.of(self.text).trim_matches([' ', '\t', '\n']);
    let group = self.docs.group();
    for (index, line) in trimmed.split('\n').enumerate() {
      if index > 0 {
        self.docs.literalline();
      }
      let fill = self.docs.fill();
      let mut last = 0;
      for (start, end) in spaces_between_words(line.as_bytes()) {
        let part = self.docs.part();
        self.docs.text(&line[last..start]);
        self.docs.close(part);
        let part = self.docs.part();
        self.docs.line();
        self.docs.close(part);
        last = end;
      }
      let part = self.docs.part();
      self.docs.text(&line[last..]);
      self.docs.close(part);
      self.docs.close(fill);
    }
    self.docs.close(group);
  }

  /// `printContent`: the content as it is written, with its ignored ranges as they are written.
  fn content(&mut self, items: &[Item]) {
    let group = self.docs.group();
    let ranges = self.ignore_ranges(items);
    let mut pending = ranges.iter().peekable();
    for &item in items {
      let start = item.start(self.tree);
      while let Some(range) = pending.next_if(|range| range.start < start) {
        self.lines(range.of(self.text));
      }
      if ranges.iter().all(|range| start < range.start || start >= range.end) {
        self.item(item);
      }
    }
    for range in pending {
      self.lines(range.of(self.text));
    }
    self.docs.close(group);
  }

  /// `isWhitespaceIgnorable`
  fn is_whitespace_ignorable(&self, element: &Element, items: &[Item]) -> bool {
    self.options.whitespace != Whitespace::Strict
      && element.name.of(self.text) != "xsl:text"
      && !self.tree.attributes(element.attributes).iter().any(|attribute| {
        attribute.name.of(self.text) == "xml:space"
          && unquoted(attribute.value.of(self.text)) == "preserve"
      })
      && !items.iter().any(|item| matches!(item, Item::CData(_)))
      && self.ignore_ranges(items).is_empty()
  }

  fn ignore_ranges(&self, items: &[Item]) -> Vec<Span> {
    match self.has_ignored_ranges {
      true => ignore_ranges(self.text, items, self.ignore),
      false => Vec::new(),
    }
  }

  // ───────────────────────────── tags ─────────────────────────────

  fn self_closed(&mut self, element: &Element) {
    let group = self.docs.group();
    self.tag_start(element, true);
    match self.space() {
      Space::Text(text) => self.docs.text(text),
      Space::Line => self.docs.line(),
      Space::Soft => self.docs.softline(),
    }
    self.docs.text("/>");
    self.docs.close(group);
  }

  fn space(&self) -> Space {
    match (self.options.bracket_same_line, self.options.self_closing_space) {
      (true, true) => Space::Text(" "),
      (true, false) => Space::Text(""),
      (false, true) => Space::Line,
      (false, false) => Space::Soft,
    }
  }

  fn open_tag(&mut self, element: &Element) {
    let group = self.docs.group();
    self.tag_start(element, true);
    if !self.options.bracket_same_line {
      self.docs.softline();
    }
    self.docs.text(">");
    self.docs.close(group);
  }

  fn close_tag(&mut self, element: &Element) {
    let group = self.docs.group();
    self.docs.text("</");
    self.span(element.end_name);
    self.docs.text(">");
    self.docs.close(group);
  }

  /// `<`, the name and the attributes. `printElement` sorts them and may put each on its own line; the tag of an
  /// embedded element does neither.
  fn tag_start(&mut self, element: &Element, is_printed: bool) {
    self.docs.text("<");
    self.span(element.name);
    let mut attributes = std::borrow::Cow::Borrowed(self.tree.attributes(element.attributes));
    if attributes.is_empty() {
      return;
    }
    if is_printed && self.options.sort_attributes {
      attributes
        .to_mut()
        .sort_by(|left, right| sort::attributes(left.name.of(self.text), right.name.of(self.text)));
    }
    let hard = is_printed && self.options.single_attribute_per_line;
    let indent = self.docs.indent();
    self.docs.line();
    for (index, attribute) in attributes.iter().enumerate() {
      if index > 0 {
        match hard {
          true => self.docs.hardline(),
          false => self.docs.line(),
        }
      }
      self.attribute(attribute);
    }
    self.docs.close(indent);
  }

  fn attribute(&mut self, attribute: &Attribute) {
    self.span(attribute.name);
    self.docs.text("=");
    let value = attribute.value.of(self.text);
    match self.options.quotes {
      Quotes::Preserve => self.docs.text(value),
      Quotes::Double => self.docs.owned(&format!("\"{}\"", unquoted(value).replace('"', "&quot;"))),
      Quotes::Single => self.docs.owned(&format!("'{}'", unquoted(value).replace('\'', "&apos;"))),
    }
  }

  // ───────────────────────────── `embed` ─────────────────────────────

  /// Prints an element whose content is code in another language, formatted, and returns true. Returns false if it
  /// isn't one, or if its code isn't formatted.
  fn embedded(&mut self, element: &Element) -> bool {
    let Some(language) = self.embedded_language(element) else { return false };
    let Some(items) = element.content.map(|range| self.tree.content(range)) else { return false };
    let (Some(first), Some(last)) = (items.first(), items.last()) else { return false };
    if !items.iter().all(|item| item.is_chardata()) {
      return false;
    }
    let code = &self.text[first.start(self.tree) as usize..last.span(self.tree).end as usize];
    let request = Request {
      code,
      language,
      kind: Purpose::Block,
      indent: 0,
      prefix: 0,
      suffix: 0,
      width: self.options.width,
      module: true,
    };
    let (options, scripts) = (self.options, self.scripts);
    let formatted = self.embed.format(request).or_else(|| {
      let builtin = options.format_embedded.then_some(request)?;
      crate::embed::builtin(&builtin, options.tab_width, options.tabs, scripts)
    });
    let document = match &formatted {
      Some(_) => None,
      None => match self.style_sheet(language, code) {
        Some(document) => Some(document),
        None => return false,
      },
    };
    let group = self.docs.group();
    let open = self.docs.group();
    self.tag_start(element, false);
    if !self.options.bracket_same_line {
      self.docs.softline();
    }
    self.docs.text(">");
    self.docs.close(open);
    self.docs.literalline();
    let root = self.docs.align(Align::Root);
    match (formatted, document) {
      (Some(formatted), _) => {
        let formatted = formatted.trim_end_matches('\n');
        for (index, line) in formatted.split('\n').enumerate() {
          if index > 0 {
            self.docs.literalline();
          }
          self.docs.owned(line);
        }
      }
      (None, Some(document)) => document.write_lines(self.docs),
      (None, None) => {}
    }
    self.docs.close(root);
    self.docs.hardline();
    self.close_tag(element);
    self.docs.close(group);
    true
  }

  /// `getParser`, for `<style>` and `<script>` only: the language that `type="text/..."` names.
  fn embedded_language(&self, element: &Element) -> Option<&'a str> {
    if !matches!(element.name.of(self.text).to_ascii_lowercase().as_str(), "style" | "script") {
      return None;
    }
    let attributes = self.tree.attributes(element.attributes);
    let kind = attributes.iter().find(|attribute| attribute.name.of(self.text) == "type")?;
    let value = kind.value.of(self.text);
    let language = value.strip_prefix("\"text/")?.strip_suffix('"')?;
    match language {
      "" => None,
      "javascript" => Some("babel"),
      language => Some(language),
    }
  }

  fn style_sheet(&mut self, language: &str, code: &str) -> Option<crate::css::Document> {
    let dialect = match language {
      "css" => crate::css::Dialect::Css,
      "scss" => crate::css::Dialect::Scss,
      "less" => crate::css::Dialect::Less,
      _ => return None,
    };
    let options = crate::css::Options {
      dialect,
      width: self.options.width,
      tab_width: self.options.tab_width,
      tabs: self.options.tabs,
      ..crate::css::Options::default()
    };
    crate::css::document(code, &options, false, false, self.embed).ok()
  }

  // ───────────────────────────── texts ─────────────────────────────

  fn span(&mut self, span: Span) {
    self.docs.text(span.of(self.text));
  }

  /// A text, its lines joined by literal lines: `printCharData` and `replaceEndOfLine`.
  fn lines(&mut self, text: &'a str) {
    for (index, line) in text.split('\n').enumerate() {
      if index > 0 {
        self.docs.literalline();
      }
      self.docs.text(line);
    }
  }
}

/// The ranges that `<!-- prettier-ignore-start -->` and `<!-- prettier-ignore-end -->` comments mark among `items`,
/// from the start of the first to the end of the second.
fn ignore_ranges(
  text: &str,
  items: &[Item],
  (start_comment, end_comment): (&str, &str),
) -> Vec<Span> {
  let mut ranges = Vec::new();
  let mut start = None;
  for item in items {
    let Item::Comment(span) = *item else { continue };
    let comment = span.of(text);
    if comment == start_comment {
      start = Some(span.start);
    } else if comment == end_comment
      && let Some(from) = start.take()
    {
      ranges.push(Span { start: from, end: span.end });
    }
  }
  ranges
}

/// `value.split(/\b( +)\b/)`: the runs of spaces that have a word character on both sides.
fn spaces_between_words(line: &[u8]) -> impl Iterator<Item = (usize, usize)> + '_ {
  let mut at = 0;
  std::iter::from_fn(move || {
    while let Some(found) = memchr::memchr(b' ', &line[at..]) {
      let start = at + found;
      let end = start + line[start..].iter().take_while(|&&byte| byte == b' ').count();
      at = end;
      let word = |at: Option<&u8>| at.is_some_and(|&byte| strings::is_regexp_word_byte(byte));
      if start > 0 && word(line.get(start - 1)) && word(line.get(end)) {
        return Some((start, end));
      }
    }
    None
  })
}

/// An attribute's value without its quotes.
fn unquoted(value: &str) -> &str {
  &value[1..value.len() - 1]
}
