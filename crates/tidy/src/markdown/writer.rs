use std::borrow::Cow;
use std::collections::VecDeque;
use std::ops::Range;

use pulldown_cmark::{
  CodeBlockKind, Event, LinkType, MetadataBlockKind, OffsetIter, Options as Flags, Parser, Tag,
  TagEnd,
};

use crate::directive::{self, Directive};
use crate::table::Table;
use crate::wrap::Paragraph;
use crate::{Bullet, Code, Emphasis, Fence, Heading, Numbering, Options, Rule, Strong, Wrap};

/// The syntax mdtidy reads: CommonMark with GFM tables, task lists, strikethrough, footnotes, and
/// front matter.
pub(crate) fn flags() -> Flags {
  Flags::ENABLE_TABLES
    | Flags::ENABLE_FOOTNOTES
    | Flags::ENABLE_STRIKETHROUGH
    | Flags::ENABLE_TASKLISTS
    | Flags::ENABLE_YAML_STYLE_METADATA_BLOCKS
    | Flags::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
}

pub(crate) fn format(
  source: &str,
  options: &Options,
  code: &mut dyn FnMut(Code<'_>) -> Option<String>,
) -> String {
  if directive::skips_file(source, &options.ignore) {
    return source.to_string();
  }
  let parser = Parser::new_ext(source, flags());
  let mut definitions: Vec<Range<usize>> =
    parser.reference_definitions().iter().map(|(_, definition)| definition.span.clone()).collect();
  definitions.sort_unstable_by_key(|span| span.start);
  let mut writer = Writer {
    source,
    options,
    code,
    out: String::with_capacity(source.len() + source.len() / 8),
    containers: Vec::new(),
    lists: Vec::new(),
    start: true,
    gap: Gap::None,
    table: None,
    block: None,
    heading: None,
    paragraph: None,
    marks: Vec::new(),
    emphasis: 0,
    skip: 0,
    ignored: 0..0,
    pending: false,
    ignore: false,
    definitions,
    next: 0,
    adjacent: None,
    last: 0,
  };
  let mut events = Events { iter: parser.into_offset_iter(), ahead: VecDeque::new() };
  while let Some((event, range)) = events.next() {
    writer.event(event, range, &mut events);
  }
  writer.flush(usize::MAX);
  let mut out = writer.out;
  out.truncate(out.trim_end_matches(['\n', ' ']).len());
  if !out.is_empty() {
    out.push('\n');
  }
  out
}

/// The parser's events with lookahead.
struct Events<'s> {
  iter: OffsetIter<'s>,
  ahead: VecDeque<(Event<'s>, Range<usize>)>,
}

impl<'s> Events<'s> {
  fn next(&mut self) -> Option<(Event<'s>, Range<usize>)> {
    self.ahead.pop_front().or_else(|| self.iter.next())
  }

  fn peek(&mut self, index: usize) -> Option<&(Event<'s>, Range<usize>)> {
    while self.ahead.len() <= index {
      let event = self.iter.next()?;
      self.ahead.push_back(event);
    }
    self.ahead.get(index)
  }
}

#[derive(Clone, Copy, PartialEq)]
enum Gap {
  None,
  Line,
  Blank,
  /// Inside a list item: a blank line only where the source had one.
  Source,
}

enum Container {
  Quote,
  /// A list item: its first line shows the marker, the rest are indented by `width`.
  Item {
    width: usize,
    marker: Option<String>,
  },
  Footnote {
    marker: Option<String>,
  },
  /// An indented code block.
  Code,
}

struct List {
  /// The current item has a blank line between its blocks, so a blank line follows it.
  spread: bool,
  items: usize,
  ordered: bool,
  /// How many lists of the same kind directly precede this one, which alternates its marker.
  nth: usize,
  start: u64,
  /// Every item after the first repeats one number, as in `1.`, `1.`, `1.`.
  repeats: bool,
  /// Prettier pads the markers of an ordered list whose content sits on tab stops.
  aligned: bool,
}

/// The list that ended last, while nothing else followed it at the same depth.
struct Adjacent {
  depth: usize,
  ordered: bool,
  nth: usize,
}

/// A block collected whole before it's written: code for the caller to format, or front matter.
enum Block {
  Fenced { info: String, fence: String, text: String },
  Indented,
  Front { fence: &'static str, text: String },
}

struct Writer<'s, 'o> {
  source: &'s str,
  options: &'o Options,
  code: &'o mut dyn FnMut(Code<'_>) -> Option<String>,
  out: String,
  containers: Vec<Container>,
  lists: Vec<List>,
  /// At the start of a line, before its container prefix.
  start: bool,
  /// What separates the next block from the previous one.
  gap: Gap,
  table: Option<Table>,
  block: Option<Block>,
  /// The range of the heading being written.
  heading: Option<Range<usize>>,
  paragraph: Option<Paragraph>,
  marks: Vec<Cow<'s, str>>,
  emphasis: usize,
  skip: usize,
  /// Source kept as written between ignore directives: its events are dropped.
  ignored: Range<usize>,
  /// The ignored range still has to be written, after the directive that opened it.
  pending: bool,
  /// The next block is kept as written.
  ignore: bool,
  definitions: Vec<Range<usize>>,
  next: usize,
  adjacent: Option<Adjacent>,
  /// Where the last block ended in the source.
  last: usize,
}

impl<'s> Writer<'s, '_> {
  fn event(&mut self, event: Event<'s>, range: Range<usize>, events: &mut Events<'s>) {
    if self.skip > 0 {
      match event {
        Event::Start(_) => self.skip += 1,
        Event::End(_) => self.skip -= 1,
        _ => {}
      }
      return;
    }
    if self.ignored.contains(&range.start) {
      return;
    }
    match event {
      Event::Start(tag) => self.open(tag, range, events),
      Event::End(tag) => {
        self.close(tag);
        self.last = range.end;
      }
      Event::Text(text) => match &mut self.block {
        Some(Block::Fenced { text: code, .. } | Block::Front { text: code, .. }) => {
          code.push_str(&text);
        }
        Some(Block::Indented) => self.lines(&text),
        None => {
          let bytes = self.source.as_bytes();
          let escaped = range.start > 0
            && bytes[range.start - 1] == b'\\'
            && bytes.get(range.start).is_some_and(u8::is_ascii_punctuation);
          let start = if escaped { range.start - 1 } else { range.start };
          self.text(&self.source[start..range.end]);
        }
      },
      Event::Code(value) => {
        let raw = &self.source[range];
        match span(raw, &value) {
          Some(span) => self.atom(&span),
          None => self.atom(raw),
        }
      }
      Event::InlineHtml(_)
      | Event::FootnoteReference(_)
      | Event::InlineMath(_)
      | Event::DisplayMath(_) => self.atom(&self.source[range]),
      Event::Html(text) => self.lines(&text),
      Event::SoftBreak => {
        if self.table.is_some() || (self.heading.is_some() && self.options.heading == Heading::Atx)
        {
          self.atom(" ");
        } else if self.heading.is_none() && self.wraps() {
          self.paragraph.get_or_insert_default().space();
        } else {
          self.newline();
        }
      }
      Event::HardBreak => {
        let mark = if self.source[range].starts_with('\\') { "\\" } else { "  " };
        if self.wraps() {
          self.paragraph.get_or_insert_default().hard(mark);
        } else {
          self.atom(mark);
          self.newline();
        }
      }
      Event::Rule => {
        if self.verbatim(range.start, &range) {
          return;
        }
        self.block(range.start);
        let rule = match self.options.rule {
          Rule::Keep => self.source[range].trim(),
          Rule::Asterisk => "***",
          Rule::Underscore => "___",
          Rule::Dash => match self.lists.last() {
            Some(list) if list.nth.is_multiple_of(2) => "***",
            _ => "---",
          },
        };
        self.write(rule);
        self.newline();
        self.end();
      }
      Event::TaskListMarker(checked) => self.atom(if checked { "[x] " } else { "[ ] " }),
    }
  }

  fn open(&mut self, tag: Tag<'s>, range: Range<usize>, events: &mut Events<'s>) {
    let block = is_block(&tag);
    if block && self.verbatim(range.start, &range) {
      self.skip = 1;
      return;
    }
    match tag {
      Tag::Paragraph => {
        self.block(range.start);
        if self.options.wrap != Wrap::Preserve {
          self.paragraph = Some(Paragraph::default());
        }
      }
      Tag::Heading { level, .. } => {
        self.block(range.start);
        let atx = self.source[range.start..].trim_start().starts_with('#');
        if atx || self.options.heading == Heading::Atx {
          self.write(&"######"[..level as usize]);
          if !matches!(events.peek(0), Some((Event::End(_), _))) {
            self.write(" ");
          }
          self.heading = Some(0..0);
        } else {
          self.heading = Some(range);
        }
      }
      Tag::BlockQuote(_) => {
        self.block(range.start);
        self.containers.push(Container::Quote);
      }
      Tag::CodeBlock(CodeBlockKind::Fenced(info)) => {
        self.block(range.start);
        let fence = fence(&self.source[range.start..]);
        self.block = Some(Block::Fenced { info: info.into_string(), fence, text: String::new() });
      }
      Tag::CodeBlock(CodeBlockKind::Indented) => {
        self.block(range.start);
        self.containers.push(Container::Code);
        self.block = Some(Block::Indented);
      }
      Tag::HtmlBlock => {
        self.block(range.start);
        match directive::parse(&self.source[range.clone()], &self.options.ignore) {
          Some(Directive::Next) => self.ignore = true,
          Some(Directive::Start) => {
            if let Some(end) = directive::end(self.source, range.end, &self.options.ignore) {
              self.ignored = range.end..end.start;
              self.pending = true;
            }
          }
          _ => {}
        }
      }
      Tag::List(first) => {
        let depth = self.containers.len();
        let ordered = first.is_some();
        let nth = match &self.adjacent {
          Some(adjacent) if adjacent.depth == depth && adjacent.ordered == ordered => {
            adjacent.nth + 1
          }
          _ => 0,
        };
        self.block(range.start);
        self.adjacent = None;
        let items = self.scan(events, ordered);
        let numbers: Vec<u64> = items.iter().map(|item| item.number).collect();
        let repeats = match numbers[..] {
          [0, second, third, ..] => second == 1 && third == 1,
          [_, second, ..] => second == 1,
          _ => false,
        };
        let aligned = self.options.numbering == Numbering::Auto
          && self.lists.iter().all(|list| list.aligned)
          && aligned(&items);
        let start = first.unwrap_or(1);
        self.lists.push(List { spread: false, items: 0, ordered, nth, start, repeats, aligned });
      }
      Tag::Item => {
        let list = self.lists.last_mut().expect("an item is inside a list");
        if list.items > 0 {
          let blank = list.spread || blank(self.source, self.last.min(range.start), range.start);
          self.gap = if blank { Gap::Blank } else { Gap::Line };
        }
        list.spread = false;
        let index = list.items;
        list.items += 1;
        let aligned = list.aligned;
        self.separate();
        self.adjacent = None;
        let mut marker = self.marker(range.start, index);
        marker.push(' ');
        if aligned && !marker.len().is_multiple_of(TAB) {
          marker.extend(std::iter::repeat_n(' ', TAB - marker.len() % TAB));
        }
        self.containers.push(Container::Item { width: marker.len(), marker: Some(marker) });
      }
      Tag::FootnoteDefinition(label) => {
        self.block(range.start);
        self.containers.push(Container::Footnote { marker: Some(format!("[^{label}]: ")) });
      }
      Tag::Table(alignments) => {
        self.block(range.start);
        self.table = Some(Table { alignments, rows: Vec::new() });
      }
      Tag::TableHead | Tag::TableRow => {
        if let Some(table) = &mut self.table {
          table.rows.push(Vec::new());
        }
      }
      Tag::TableCell => {
        if let Some(row) = self.table.as_mut().and_then(|table| table.rows.last_mut()) {
          row.push(String::new());
        }
      }
      Tag::Emphasis => {
        let mark: Cow<'s, str> = match self.options.emphasis {
          Emphasis::Keep => self.source[range.start..range.start + 1].into(),
          Emphasis::Asterisk => "*".into(),
          Emphasis::Underscore if self.emphasis > 0 || self.inside(range.end) => "*".into(),
          Emphasis::Underscore => "_".into(),
        };
        self.emphasis += 1;
        self.atom(&mark);
        self.marks.push(mark);
      }
      Tag::Strong => {
        let mark: Cow<'s, str> = match self.options.strong {
          Strong::Keep => self.source[range.start..range.start + 2].into(),
          Strong::Underscore if !self.inside(range.end) => "__".into(),
          Strong::Underscore | Strong::Asterisk => "**".into(),
        };
        self.atom(&mark);
        self.marks.push(mark);
      }
      Tag::Strikethrough => {
        let mark: Cow<'s, str> = if self.options.emphasis == Emphasis::Keep {
          let count = self.source[range.start..].bytes().take_while(|&byte| byte == b'~').count();
          self.source[range.start..range.start + count].into()
        } else {
          "~~".into()
        };
        self.atom(&mark);
        self.marks.push(mark);
      }
      Tag::Link { link_type, .. } | Tag::Image { link_type, .. } => {
        let collapsed = matches!(link_type, LinkType::Collapsed | LinkType::CollapsedUnknown)
          && self.source[range.end..].starts_with("[]");
        let end = if collapsed { range.end + 2 } else { range.end };
        self.atom(&self.source[range.start..end]);
        self.skip = 1;
      }
      Tag::MetadataBlock(kind) => {
        self.block(range.start);
        let fence = if kind == MetadataBlockKind::YamlStyle { "---" } else { "+++" };
        self.block = Some(Block::Front { fence, text: String::new() });
      }
      _ => {
        self.atom(&self.source[range]);
        self.skip = 1;
      }
    }
  }

  fn close(&mut self, tag: TagEnd) {
    match tag {
      TagEnd::Paragraph => {
        self.finish();
        self.end();
      }
      TagEnd::HtmlBlock => {
        self.finish();
        if std::mem::take(&mut self.pending) {
          self.out.push_str(&self.source[self.ignored.clone()]);
          self.start = self.out.ends_with('\n');
          self.gap = Gap::None;
        } else if self.ignore {
          self.gap = Gap::Line;
        } else {
          self.end();
        }
      }
      TagEnd::Heading(_) => {
        self.finish();
        if let Some(range) = self.heading.take().filter(|range| !range.is_empty()) {
          let line = self.source[range].trim_end().lines().last().unwrap_or("===");
          let mark = line.chars().next_back().unwrap_or('=');
          let underline = line.trim_end_matches(mark);
          self.write(&line[underline.len()..]);
          self.newline();
        }
        self.end();
      }
      TagEnd::BlockQuote(_) => {
        self.finish();
        self.containers.pop();
        self.adjacent = None;
        self.end();
      }
      TagEnd::CodeBlock | TagEnd::MetadataBlock(_) => {
        match self.block.take() {
          Some(Block::Fenced { info, fence, text }) => self.fenced(&info, fence, text),
          Some(Block::Front { fence, text }) => self.front(fence, text),
          Some(Block::Indented) => {
            self.finish();
            self.containers.pop();
          }
          None => {}
        }
        self.end();
      }
      TagEnd::List(_) => {
        self.finish();
        if let Some(list) = self.lists.pop() {
          self.adjacent =
            Some(Adjacent { depth: self.containers.len(), ordered: list.ordered, nth: list.nth });
        }
        self.end();
      }
      TagEnd::Item | TagEnd::FootnoteDefinition => {
        self.finish();
        self.adjacent = None;
        let marker = match self.containers.pop() {
          Some(Container::Item { marker, .. } | Container::Footnote { marker }) => marker,
          _ => None,
        };
        if let Some(marker) = marker {
          self.prefix();
          self.out.push_str(marker.trim_end());
          self.newline();
        }
        if tag == TagEnd::FootnoteDefinition {
          self.end();
        } else {
          self.gap = Gap::None;
        }
      }
      TagEnd::Table => {
        if let Some(table) = self.table.take() {
          let width = self.options.width.saturating_sub(self.indent());
          for line in table.lines(width, self.options.wrap) {
            self.write(&line);
            self.newline();
          }
        }
        self.end();
      }
      TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
        if tag == TagEnd::Emphasis {
          self.emphasis -= 1;
        }
        if let Some(mark) = self.marks.pop() {
          self.atom(&mark);
        }
      }
      _ => {}
    }
  }

  /// Keep the block at `range` as written if an ignore directive came before it.
  fn verbatim(&mut self, at: usize, range: &Range<usize>) -> bool {
    if !std::mem::take(&mut self.ignore) {
      return false;
    }
    self.block(at);
    let text = self.source[range.clone()].trim_end();
    let mut lines = text.split('\n');
    if let Some(first) = lines.next() {
      self.write(first.trim_start());
    }
    for line in lines {
      self.out.push('\n');
      self.out.push_str(line);
    }
    self.newline();
    self.end();
    true
  }

  /// Read ahead through the ordered list that just started, for the first items' numbers.
  fn scan(&self, events: &mut Events<'s>, ordered: bool) -> Vec<Item> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut index = 0;
    while ordered && items.len() < 3 {
      let Some((event, range)) = events.peek(index) else { break };
      index += 1;
      match event {
        Event::Start(_) => {
          if depth == 0 {
            items.push(self.item(range.start));
          }
          depth += 1;
        }
        Event::End(_) if depth == 0 => break,
        Event::End(_) => depth -= 1,
        _ => {}
      }
    }
    items
  }

  /// The number of the ordered item whose marker is at `at`, and where its content starts.
  fn item(&self, at: usize) -> Item {
    let line = self.source[..at].rfind('\n').map_or(0, |index| index + 1);
    let rest = &self.source[at..];
    let indent = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let rest = &rest[indent..];
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let after = rest.get(digits + 1..).unwrap_or("");
    let spaces = after.bytes().take_while(|&byte| byte == b' ').count();
    let empty = after[spaces..].starts_with(['\n', '\r']) || after[spaces..].is_empty();
    Item {
      number: rest[..digits].parse().unwrap_or(0),
      spaces,
      column: (!empty).then_some(at - line + indent + digits + 1 + spaces),
    }
  }

  fn marker(&self, at: usize, index: usize) -> String {
    let list = self.lists.last().expect("an item is inside a list");
    let written = self.source[at..].trim_start();
    if list.ordered {
      let digits = written.bytes().take_while(u8::is_ascii_digit).count();
      if self.options.numbering == Numbering::Keep {
        return written[..digits + 1].to_string();
      }
      let number = match index {
        0 => list.start,
        _ if list.repeats => 1,
        _ => list.start + index as u64,
      };
      let delimiter = if list.nth.is_multiple_of(2) { '.' } else { ')' };
      return format!("{number}{delimiter}");
    }
    let (even, odd) = match self.options.bullet {
      Bullet::Keep => return written[..1].to_string(),
      Bullet::Dash => ("-", "*"),
      Bullet::Asterisk => ("*", "-"),
      Bullet::Plus => ("+", "-"),
    };
    (if list.nth.is_multiple_of(2) { even } else { odd }).to_string()
  }

  fn fenced(&mut self, info: &str, original: String, mut text: String) {
    let (lang, meta) = info.trim().split_once(char::is_whitespace).unwrap_or((info.trim(), ""));
    let meta = meta.trim_start();
    let width = self.options.width.saturating_sub(self.indent());
    if !lang.is_empty()
      && let Some(mut formatted) = (self.code)(Code { lang, text: &text, width })
    {
      if !formatted.is_empty() && !formatted.ends_with('\n') {
        formatted.push('\n');
      }
      text = formatted;
    }
    let mut char = match self.options.fence {
      Fence::Keep => original.chars().next().unwrap_or('`'),
      Fence::Backtick => '`',
      Fence::Tilde => '~',
    };
    if char == '`' && info.contains('`') {
      char = '~';
    }
    let longest = longest(&text, char);
    let least = if self.options.fence == Fence::Keep { original.len() } else { 3 };
    let fence: String = std::iter::repeat_n(char, least.max(longest + 1)).collect();
    self.write(&fence);
    if self.options.fence == Fence::Keep {
      self.write(info.trim());
    } else if !lang.is_empty() {
      self.write(lang);
      if !meta.is_empty() {
        self.write(" ");
        self.write(meta);
      }
    }
    self.newline();
    self.lines(&text);
    self.finish();
    self.write(&fence);
    self.newline();
  }

  fn front(&mut self, fence: &'static str, mut text: String) {
    let lang = if fence == "---" { "yaml" } else { "toml" };
    let width = self.options.width;
    if let Some(formatted) = (self.code)(Code { lang, text: &text, width }) {
      text = formatted;
    }
    self.out.push_str(fence);
    self.out.push('\n');
    self.out.push_str(&text);
    if !text.is_empty() && !text.ends_with('\n') {
      self.out.push('\n');
    }
    self.out.push_str(fence);
    self.out.push('\n');
    self.start = true;
  }

  /// Start a block: emit pending reference definitions before it, then separate it.
  fn block(&mut self, at: usize) {
    self.finish();
    self.flush(at);
    if self.adjacent.as_ref().is_some_and(|adjacent| adjacent.depth == self.containers.len()) {
      self.adjacent = None;
    }
    if self.gap == Gap::Source {
      let blank = blank(self.source, self.last.min(at), at);
      self.gap = if blank { Gap::Blank } else { Gap::Line };
      if blank && let Some(list) = self.lists.last_mut() {
        list.spread = true;
      }
    }
    self.separate();
  }

  fn separate(&mut self) {
    if !self.start {
      self.newline();
    }
    if self.gap == Gap::Blank {
      self.blank();
    }
    self.gap = Gap::None;
  }

  /// A block ended: the next one needs a blank line, except inside a list item, where Prettier
  /// keeps blocks together unless the source had a blank line between them.
  fn end(&mut self) {
    let item = matches!(self.containers.last(), Some(Container::Item { .. }));
    self.gap = if item { Gap::Source } else { Gap::Blank };
  }

  /// Write out a collected paragraph, and end the line.
  fn finish(&mut self) {
    if let Some(paragraph) = self.paragraph.take() {
      let width = match self.options.wrap {
        Wrap::Always => Some(self.options.width.saturating_sub(self.indent())),
        Wrap::Never | Wrap::Preserve => None,
      };
      for line in paragraph.fill(width) {
        self.write(&line);
        self.newline();
      }
    }
    if !self.start {
      self.newline();
    }
  }

  fn flush(&mut self, at: usize) {
    while let Some(span) = self.definitions.get(self.next).filter(|span| span.start < at) {
      let span = span.clone();
      self.next += 1;
      if self.ignored.contains(&span.start) {
        continue;
      }
      self.adjacent = None;
      self.separate();
      self.write(self.source[span].trim_end());
      self.newline();
      self.gap = Gap::Blank;
    }
  }

  /// Whether inline content goes into a paragraph to be filled.
  fn wraps(&self) -> bool {
    self.options.wrap != Wrap::Preserve
      && self.table.is_none()
      && self.heading.is_none()
      && (self.paragraph.is_some()
        || matches!(self.containers.last(), Some(Container::Item { .. })))
  }

  /// Whether emphasis ending at `end` touches a word on either side, where `_` doesn't work.
  fn inside(&self, end: usize) -> bool {
    let before = match (&self.paragraph, &self.table) {
      (_, Some(table)) => {
        table.rows.last().and_then(|row| row.last()).and_then(|cell| cell.chars().next_back())
      }
      (Some(paragraph), None) => paragraph.last(),
      (None, None) if self.start => None,
      (None, None) => self.out.chars().next_back(),
    };
    let after = self.source[end..].chars().next();
    before.is_some_and(char::is_alphanumeric) || after.is_some_and(char::is_alphanumeric)
  }

  fn text(&mut self, text: &str) {
    if self.wraps() {
      self.paragraph.get_or_insert_default().text(text);
    } else {
      self.atom(text);
    }
  }

  fn atom(&mut self, text: &str) {
    if let Some(cell) =
      self.table.as_mut().and_then(|table| table.rows.last_mut()).and_then(|row| row.last_mut())
    {
      cell.push_str(text);
      return;
    }
    if self.wraps() {
      self.paragraph.get_or_insert_default().atom(text);
      return;
    }
    self.write(text);
  }

  /// Write `text` on the current line, behind the container prefix at its start.
  fn write(&mut self, text: &str) {
    if self.start {
      self.prefix();
    }
    self.out.push_str(text);
  }

  fn prefix(&mut self) {
    for container in &mut self.containers {
      match container {
        Container::Quote => self.out.push_str("> "),
        Container::Code => self.out.push_str("    "),
        Container::Item { width, marker } => match marker.take() {
          Some(marker) => self.out.push_str(&marker),
          None => self.out.extend(std::iter::repeat_n(' ', *width)),
        },
        Container::Footnote { marker } => match marker.take() {
          Some(marker) => self.out.push_str(&marker),
          None => self.out.push_str("    "),
        },
      }
    }
    self.start = false;
  }

  /// The width of the container prefix.
  fn indent(&self) -> usize {
    self
      .containers
      .iter()
      .map(|container| match container {
        Container::Quote => 2,
        Container::Item { width, .. } => *width,
        Container::Footnote { .. } | Container::Code => 4,
      })
      .sum()
  }

  fn newline(&mut self) {
    let trimmed = self.out.trim_end_matches([' ', '\t']).len();
    if !self.out.ends_with("  ") || self.out[trimmed..].len() != 2 {
      self.out.truncate(trimmed);
    }
    self.out.push('\n');
    self.start = true;
  }

  fn blank(&mut self) {
    let mark = self.out.len();
    self.prefix();
    self.out.truncate(mark + self.out[mark..].trim_end().len());
    self.out.push('\n');
    self.start = true;
  }

  /// Verbatim lines of code or HTML, each behind the container prefix.
  fn lines(&mut self, text: &str) {
    for line in text.split_inclusive('\n') {
      if line == "\n" || line == "\r\n" {
        self.blank();
        continue;
      }
      if self.start {
        self.prefix();
      }
      self.out.push_str(line);
      self.start = line.ends_with('\n');
    }
  }
}

/// Whether `tag` opens a block, as opposed to inline content.
fn is_block(tag: &Tag<'_>) -> bool {
  matches!(
    tag,
    Tag::Paragraph
      | Tag::Heading { .. }
      | Tag::BlockQuote(_)
      | Tag::CodeBlock(_)
      | Tag::HtmlBlock
      | Tag::List(_)
      | Tag::Table(_)
      | Tag::FootnoteDefinition(_)
      | Tag::MetadataBlock(_)
  )
}

/// The opening fence of a fenced code block written at the start of `text`.
fn fence(text: &str) -> String {
  let text = text.trim_start_matches([' ', '\t']);
  let char = text.chars().next().filter(|&char| char == '~').unwrap_or('`');
  let count = text.chars().take_while(|&next| next == char).count().max(3);
  std::iter::repeat_n(char, count).collect()
}

/// The longest run of `char` in `text`.
fn longest(text: &str, char: char) -> usize {
  let mut longest = 0;
  let mut run = 0;
  for next in text.chars() {
    run = if next == char { run + 1 } else { 0 };
    longest = longest.max(run);
  }
  longest
}

/// A code span rewritten as Prettier writes it, or `None` when `raw` already is: the fewest
/// backticks that no run inside matches, padded with a space only where the content needs one.
fn span(raw: &str, value: &str) -> Option<String> {
  let ticks = raw.bytes().take_while(|&byte| byte == b'`').count();
  let inner = &raw[ticks..raw.len() - ticks];
  let unpadded = inner
    .strip_prefix([' ', '\n'])
    .and_then(|rest| rest.strip_suffix([' ', '\n']))
    .filter(|_| inner.bytes().any(|byte| !matches!(byte, b' ' | b'\n' | b'\r')))
    .unwrap_or(inner);
  let mut fewest = 1;
  while has_run(value, fewest) {
    fewest += 1;
  }
  let edge = |byte: Option<u8>| matches!(byte, Some(b' ' | b'\n'));
  let bytes = value.as_bytes();
  let pad = value.starts_with('`')
    || value.ends_with('`')
    || (edge(bytes.first().copied())
      && edge(bytes.last().copied())
      && bytes.iter().any(|byte| !matches!(byte, b' ' | b'\n')));
  let spaced = inner.starts_with(' ') && inner.ends_with(' ');
  if ticks == fewest && (unpadded.len() < inner.len()) == pad && (!pad || spaced) {
    return None;
  }
  let fence = "`".repeat(fewest);
  let pad = if pad { " " } else { "" };
  Some(format!("{fence}{pad}{unpadded}{pad}{fence}"))
}

/// Whether `text` has a run of exactly `length` backticks.
fn has_run(text: &str, length: usize) -> bool {
  text.split(|char| char != '`').any(|run| run.len() == length)
}

/// The tab width that Prettier aligns ordered list content to.
const TAB: usize = 2;

/// An ordered list item as written.
struct Item {
  number: u64,
  /// Spaces between the marker and the content.
  spaces: usize,
  /// The column the content starts at, unless the item starts empty.
  column: Option<usize>,
}

/// Prettier's `isAligned`: whether an ordered list keeps its content on tab stops.
fn aligned(items: &[Item]) -> bool {
  let Some(first) = items.first() else { return false };
  if first.spaces > 1 {
    return true;
  }
  let Some(column) = first.column else { return false };
  match items.get(1) {
    None => column.is_multiple_of(TAB),
    Some(second) if second.column != Some(column) => false,
    Some(_) if column.is_multiple_of(TAB) => true,
    Some(second) => second.spaces > 1,
  }
}

/// Whether a blank line, holding nothing but whitespace and block quote markers, separates the
/// block that ended at `last` from the one starting at `at`.
fn blank(source: &str, last: usize, at: usize) -> bool {
  let end = source[..last].trim_end_matches(['\n', '\r', ' ', '\t', '>']).len();
  let from = source[end..].find('\n').map_or(source.len(), |index| end + index + 1);
  let to = source[..at].rfind('\n').map_or(0, |index| index + 1);
  from < to
    && source[from..to].lines().any(|line| line.trim_matches([' ', '\t', '\r', '>']).is_empty())
}
