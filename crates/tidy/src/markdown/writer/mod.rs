use std::borrow::Cow;
use std::collections::VecDeque;
use std::ops::Range;

mod code;
mod define;
mod footnote;
mod inline;
mod layout;
mod liquid;
mod list;

use pulldown_cmark::{
  CodeBlockKind, Event, LinkType, OffsetIter, Options as Flags, Parser, Tag, TagEnd,
};

use crate::directive::{self, Directive};
use crate::front;
use crate::table::Table;
use crate::wrap::Paragraph;
use crate::{Code, Emphasis, Heading, Numbering, Options, Rule, Strong, Wrap};
use code::{Block, fence};
use inline::{Image, Link, collapse, span};
use list::{Adjacent, List, TAB, aligned, widen};

/// The syntax mdtidy reads: CommonMark with GFM tables, task lists, strikethrough, footnotes, and
/// front matter.
pub(crate) fn flags() -> Flags {
  Flags::ENABLE_TABLES
    | Flags::ENABLE_FOOTNOTES
    | Flags::ENABLE_STRIKETHROUGH
    | Flags::ENABLE_TASKLISTS
}

pub(crate) fn format(
  source: &str,
  options: &Options,
  code: &mut dyn FnMut(Code<'_>) -> Option<String>,
) -> String {
  if directive::skips_file(source, &options.ignore) {
    return source.to_string();
  }
  let front = front::split(source);
  let body = front.as_ref().map_or(source, |front| &source[front.end..]);
  let parser = Parser::new_ext(body, flags());
  let mut definitions: Vec<Definition> = parser
    .reference_definitions()
    .iter()
    .map(|(_, definition)| {
      let span = definition.span.clone();
      let title = definition.title.as_deref();
      let line = inline::definition(&body[span.clone()], &definition.dest, title);
      Definition { span, line }
    })
    .collect();
  definitions.sort_unstable_by_key(|definition| definition.span.start);
  let mut writer = Writer {
    source: body,
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
    link: None,
    image: None,
    defined: None,
    cursor: Some(0),
    joins: None,
    html: None,
    liquid: None,
    lazy: false,
    bare: false,
    raw: None,
  };
  if let Some(front) = front {
    writer.front(&front);
  }
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
    /// The lines after the first are indented past a task box too, as Prettier does in the first
    /// paragraph of a task item.
    hang: bool,
    /// Prettier separates every block in a loose item with a blank line.
    loose: bool,
  },
  Footnote {
    marker: Option<String>,
  },
  /// An indented code block.
  Code,
}

/// The width of a task box, `[ ] `.
const BOX: usize = 4;

/// A link reference definition, and Prettier's form of it unless that would read differently.
struct Definition {
  span: Range<usize>,
  line: Option<String>,
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
  definitions: Vec<Definition>,
  next: usize,
  adjacent: Option<Adjacent>,
  /// Where the last block ended in the source.
  last: usize,
  link: Option<Link>,
  image: Option<Image>,
  /// The output length and depth right after the last definition, to keep the next one beside it.
  defined: Option<(usize, usize)>,
  /// Where the last sibling block ended, to search the gap after it for definitions. A container
  /// that just started has none, since its gap holds markers.
  cursor: Option<usize>,
  /// The depth of the paragraph or HTML block that just ended, which an HTML block on the next
  /// line stays beside, as in Prettier.
  joins: Option<usize>,
  /// Inside an HTML block, where pulldown-cmark reports the first line's indent as text: the
  /// indent it must stay below, so it doesn't join a list that just ended.
  html: Option<usize>,
  /// The depth of the paragraph that just ended with a Liquid line, which Prettier keeps beside the
  /// next block.
  liquid: Option<usize>,
  /// The last line was a lazy line kept at the left margin, so the next lazy line stays there too.
  lazy: bool,
  /// The next line starts at the left margin, without the container prefix.
  bare: bool,
  /// Where the rest of a paragraph starts that is copied as written, up to its first block event.
  raw: Option<usize>,
}

impl<'s> Writer<'s, '_> {
  fn event(&mut self, event: Event<'s>, range: Range<usize>, events: &mut Events<'s>) {
    if self.skip > 0 {
      match event {
        Event::Start(_) => self.skip += 1,
        Event::End(_) => self.skip -= 1,
        _ => {}
      }
      if self.skip > 0 {
        if let Some(image) = &mut self.image {
          image.text = image.text.max(range.end);
        }
      } else if let Some(image) = self.image.take() {
        let alt = &self.source[image.start..image.text];
        self.atom(&format!("{alt}{}", image.tail));
      }
      return;
    }
    if self.ignored.contains(&range.start) {
      return;
    }
    if let Some(start) = self.raw {
      let end = match &event {
        Event::End(tag) if liquid::ends_block(tag) => range.end,
        Event::Start(tag) if is_block(tag) => range.start,
        _ => return,
      };
      self.raw = None;
      self.write(self.source[start..end].trim_end());
    }
    if let Some(link) = &mut self.link
      && !matches!(event, Event::End(TagEnd::Link))
    {
      link.text = link.text.max(range.end);
    }
    match event {
      Event::Start(tag) => {
        let container =
          matches!(tag, Tag::BlockQuote(_) | Tag::List(_) | Tag::Item | Tag::FootnoteDefinition(_));
        self.open(tag, range, events);
        if container {
          self.cursor = None;
        }
      }
      Event::End(tag) => {
        let joins = matches!(tag, TagEnd::Paragraph | TagEnd::HtmlBlock);
        let liquid = tag == TagEnd::Paragraph && self.ends_liquid(range.end);
        let block = liquid::ends_block(&tag);
        if matches!(tag, TagEnd::BlockQuote(_) | TagEnd::Item | TagEnd::FootnoteDefinition) {
          self.flush(range.end);
        }
        self.close(tag);
        if block {
          self.cursor = Some(range.end);
        }
        self.joins = joins.then_some(self.containers.len());
        self.liquid = liquid.then_some(self.containers.len());
        self.last = range.end;
      }
      Event::Text(text) => match &mut self.block {
        Some(Block::Fenced { text: code, .. }) => {
          code.push_str(&text);
        }
        Some(Block::Indented) => self.lines(&text, true),
        None if range.is_empty() && self.html.is_some_and(|limit| text.len() < limit) => {
          self.write(&text);
        }
        None if range.is_empty() => {}
        None => {
          let punctuation =
            self.source.as_bytes().get(range.start).is_some_and(u8::is_ascii_punctuation);
          let start = if punctuation && escaped(self.source, range.start) {
            range.start - 1
          } else {
            range.start
          };
          self.text(&self.source[start..range.end]);
        }
      },
      Event::Code(value) => {
        let raw = &self.source[range];
        let joined = self.wraps() && raw.contains('\n');
        match span(raw, &value, joined) {
          Some(span) => self.atom(&span),
          None => self.atom(raw),
        }
      }
      Event::InlineHtml(_)
      | Event::FootnoteReference(_)
      | Event::InlineMath(_)
      | Event::DisplayMath(_) => self.atom(&self.source[range]),
      Event::Html(text) => self.lines(&text, false),
      Event::SoftBreak => {
        if self.table.is_some() || (self.heading.is_some() && self.options.heading == Heading::Atx)
        {
          self.atom(" ");
        } else if self.link.is_some() && self.wraps() {
          self.paragraph.get_or_insert_default().atom(" ");
        } else if self.heading.is_none() && self.wraps() {
          let next = events.peek(0).map_or(self.source.len(), |(_, range)| range.start);
          let kept = self.liquid_break(range.start, next);
          let paragraph = self.paragraph.get_or_insert_default();
          if kept {
            paragraph.hard("");
          } else {
            paragraph.space();
          }
        } else {
          let next = events.peek(0).map_or(self.source.len(), |(_, range)| range.start);
          self.soft(range.start, next);
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
        self.joins = None;
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
      Event::TaskListMarker(checked) => {
        let mark = if checked { "[x] " } else { "[ ] " };
        match self.containers.last_mut() {
          Some(Container::Item { marker: Some(marker), hang, .. }) => {
            marker.push_str(mark);
            *hang = true;
          }
          _ => self.atom(mark),
        }
      }
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
        if self.starts_liquid(range.start) {
          self.liquid = Some(self.containers.len());
        }
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
        let depth = self.containers.len();
        let limit =
          self.adjacent.as_ref().filter(|list| list.depth == depth).map(|list| list.width);
        self.html = Some(limit.unwrap_or(usize::MAX));
        let beside = self.joins == Some(self.containers.len())
          && !blank(self.source, self.last.min(range.start), range.start);
        if beside && matches!(self.gap, Gap::Blank | Gap::Source) {
          self.gap = Gap::Line;
        }
        self.block(range.start);
        match directive::parse(&self.source[range.clone()], &self.options.ignore) {
          Some(Directive::Next) => self.ignore = true,
          Some(Directive::Start) => {
            if let Some(end) = directive::end(self.source, range.end, &self.options.ignore) {
              let line = self.source[..end.start].rfind('\n').map_or(0, |index| index + 1);
              self.ignored = range.end..line.max(range.end);
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
        self.settle(range.start, false);
        self.block(range.start);
        self.adjacent = None;
        let least = self.least(events);
        let items = self.scan(events, ordered);
        let numbers: Vec<u64> = items.iter().map(|item| item.number).collect();
        let repeats = match numbers[..] {
          [0, second, third, ..] => second == 1 && third == 1,
          [_, second, ..] => second == 1,
          _ => false,
        };
        let aligned = self.options.numbering == Numbering::Auto
          && self.lists.iter().all(|list| list.aligned)
          && least == 0
          && (!ordered || aligned(&items));
        let start = first.unwrap_or(1);
        let list =
          List { spread: false, items: 0, ordered, nth, start, repeats, aligned, width: 0, least };
        self.lists.push(list);
      }
      Tag::Item => {
        let loose = self.loose(events);
        let list = self.lists.last_mut().expect("an item is inside a list");
        if list.items > 0 {
          let blank = list.spread || blank(self.source, self.last.min(range.start), range.start);
          self.gap = if blank { Gap::Blank } else { Gap::Line };
        }
        list.spread = loose;
        let index = list.items;
        list.items += 1;
        let (aligned, least) = (list.aligned, list.least);
        self.separate();
        self.adjacent = None;
        let mut marker = self.marker(range.start, index);
        marker.push(' ');
        if aligned && !marker.len().is_multiple_of(TAB) {
          marker.extend(std::iter::repeat_n(' ', TAB - marker.len() % TAB));
        }
        widen(&mut marker, least);
        let width = marker.len();
        let item = Container::Item { width, marker: Some(marker), hang: false, loose };
        self.containers.push(item);
      }
      Tag::FootnoteDefinition(label) => {
        self.block(range.start);
        let marker = format!("[^{label}]: ");
        if self.hangs(events, &marker) {
          self.write(marker.trim_end());
          self.newline();
          self.containers.push(Container::Footnote { marker: None });
        } else {
          self.containers.push(Container::Footnote { marker: Some(marker) });
        }
      }
      Tag::Table(alignments) => {
        self.block(range.start);
        self.table = Some(Table::new(alignments));
      }
      Tag::TableHead | Tag::TableRow => {
        if let Some(table) = &mut self.table {
          table.row(range);
        }
      }
      Tag::TableCell => {
        if let Some(table) = &mut self.table {
          table.cell(range);
        }
      }
      Tag::Emphasis => {
        let word = self.inside(range.end);
        let mark: Cow<'s, str> = match self.options.emphasis {
          Emphasis::Keep => self.source[range.start..range.start + 1].into(),
          _ if !word && self.before() == Some('*') => "_".into(),
          Emphasis::Asterisk => "*".into(),
          Emphasis::Underscore if self.emphasis > 0 || word => "*".into(),
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
      Tag::Link { link_type, ref dest_url, ref title, .. }
      | Tag::Image { link_type, ref dest_url, ref title, .. } => {
        let collapsed = matches!(link_type, LinkType::Collapsed | LinkType::CollapsedUnknown)
          && self.source[range.end..].starts_with("[]");
        let end = if collapsed { range.end + 2 } else { range.end };
        let tail = match link_type {
          LinkType::Inline => inline::tail(dest_url, title),
          _ => None,
        };
        let image = matches!(tag, Tag::Image { .. });
        if image || matches!(link_type, LinkType::Autolink | LinkType::Email) {
          if let Some(link) = &mut self.link {
            link.text = link.text.max(end);
          }
          match tail.filter(|_| image) {
            Some(tail) => {
              self.image = Some(Image { start: range.start, text: range.start + 2, tail })
            }
            None => self.atom(&self.source[range.start..end]),
          }
          self.skip = 1;
        } else {
          self.atom("[");
          self.link = Some(Link { text: range.start + 1, end, tail });
        }
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
        self.html = None;
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
      TagEnd::CodeBlock => {
        match self.block.take() {
          Some(Block::Fenced { info, fence, text }) => self.fenced(&info, fence, text),
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
          let (ordered, nth, width) = (list.ordered, list.nth, list.width);
          self.adjacent = Some(Adjacent { depth: self.containers.len(), ordered, nth, width });
        }
        self.end();
      }
      TagEnd::Item | TagEnd::FootnoteDefinition => {
        self.finish();
        self.adjacent = None;
        let marker = match self.containers.pop() {
          Some(Container::Item { marker, width, .. }) => {
            if let Some(list) = self.lists.last_mut() {
              list.width = width;
            }
            marker
          }
          Some(Container::Footnote { marker }) => marker,
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
      TagEnd::TableHead | TagEnd::TableRow => {
        if let Some(table) = &mut self.table {
          table.end(self.source);
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
      TagEnd::Link => {
        if let Some(link) = self.link.take() {
          let raw = &self.source[link.text..link.end];
          let raw = if self.start { raw.trim_start_matches([' ', '\t', '>']) } else { raw };
          let reference = raw.strip_prefix(']').and_then(inline::label);
          let tail = match (link.tail, reference) {
            (Some(tail), _) => Cow::Owned(tail),
            (None, Some(label)) if !label.contains('\\') => {
              let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
              Cow::Owned(format!("][{label}]"))
            }
            (None, _) => Cow::Borrowed(raw),
          };
          self.atom(&tail);
        }
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

  /// Whether inline content goes into a paragraph to be filled.
  fn wraps(&self) -> bool {
    self.options.wrap != Wrap::Preserve
      && self.table.is_none()
      && self.heading.is_none()
      && (self.paragraph.is_some()
        || matches!(self.containers.last(), Some(Container::Item { .. })))
  }

  fn text(&mut self, text: &str) {
    let text = collapse(text);
    if self.link.is_none() && self.wraps() {
      self.paragraph.get_or_insert_default().text(&text);
    } else {
      self.atom(&text);
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
}

/// Whether a backslash escapes the character at `at`: an odd run of them comes right before it.
fn escaped(source: &str, at: usize) -> bool {
  let run = source.as_bytes()[..at].iter().rev().take_while(|&&byte| byte == b'\\').count();
  run % 2 == 1
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
  )
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
