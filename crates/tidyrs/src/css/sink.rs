//! The CSS printer's output sink. It receives the parts of a document in order. Ported from bun (MIT).
//!
//! Most of a style sheet doesn't need a document. Outside of any group, a `line` is simply a line break. And if a whole
//! line fits with every group on it flat, that's how Prettier prints it, because a group is measured together with what
//! follows it up to the next possible line break. So everything is first written straight to the output that way. If a
//! line with a group on it turns out too long, or a group contains something that can't simply be written flat,
//! `Sink::end_unit` reports it. The unit's output is then rolled back, and the unit is written again, this time into a
//! document, which `crate::doc` prints in its place.

use crate::doc::{self, Align, Branch, Docs};

/// The separator between two items of a `fill`.
#[derive(Copy, Clone)]
pub(crate) enum Separator {
  Line,
  SoftLine,
  HardLine,
  /// `dedent(line)`
  DedentedLine,
  /// `dedent(hardline)`
  DedentedHardLine,
}

/// The sink's state at the start of a unit.
pub(crate) struct Mark {
  len: usize,
  line_start: usize,
  line_start_column: usize,
  depth: usize,
  outer_depths: usize,
}

/// Where the sink's output goes, and how it's laid out.
pub(crate) struct Output<'o> {
  pub(crate) out: &'o mut Vec<u8>,
  pub(crate) options: doc::Options,
}

/// A document's parts, front to back. A container is a start element and an end element.
#[derive(Copy, Clone)]
enum Element {
  /// A range of `Builder::texts`.
  Text(u32, u32),
  Line,
  SoftLine,
  /// `hardline`
  HardLine,
  /// `literallineWithoutBreakParent`
  LiteralLine,
  BreakParent,
  LineSuffixBoundary,
  /// `ifBreak(text)`: a range of `Builder::texts`.
  IfBreak(u32, u32),
  /// A removed element.
  Nothing,
  StartGroup(bool),
  StartIndent,
  StartDedent,
  StartFill,
  StartItem,
  StartLineSuffix,
  End,
}

/// The document of a style sheet that is embedded in another language.
pub(crate) struct Document(Builder);

impl Document {
  /// Writes the document to `docs`.
  pub(crate) fn write(&self, docs: &mut Docs<'_>) {
    self.0.write(docs, false);
  }

  /// Writes the document to `docs` after Prettier's `replaceEndOfLine`: each line break in a text is a literal line.
  pub(crate) fn write_lines(&self, docs: &mut Docs<'_>) {
    self.0.write(docs, true);
  }
}

/// A unit's document, which can still be changed the ways that the CSS printer needs before it's written to `Docs`.
#[derive(Default)]
struct Builder {
  list: Vec<Element>,
  texts: Vec<u8>,
  /// The start indexes of the containers that are open.
  open: Vec<usize>,
}

impl Builder {
  fn clear(&mut self) {
    self.list.clear();
    self.texts.clear();
    self.open.clear();
  }

  fn text(&mut self, text: &[u8]) {
    if text.is_empty() {
      return;
    }
    let start = self.texts.len() as u32;
    self.texts.extend_from_slice(text);
    self.list.push(Element::Text(start, text.len() as u32));
  }

  fn if_break(&mut self, text: &[u8]) {
    let start = self.texts.len() as u32;
    self.texts.extend_from_slice(text);
    self.list.push(Element::IfBreak(start, text.len() as u32));
  }

  fn start(&mut self, element: Element) {
    self.open.push(self.list.len());
    self.list.push(element);
  }

  fn end(&mut self) {
    if self.open.pop().is_some() {
      self.list.push(Element::End);
    }
  }

  /// Returns the index of the `End` of the container that starts at `start`.
  fn end_of(&self, start: usize) -> usize {
    let mut depth = 0usize;
    for (at, element) in self.list.iter().enumerate().skip(start) {
      match element {
        Element::StartGroup(_)
        | Element::StartIndent
        | Element::StartDedent
        | Element::StartFill
        | Element::StartItem
        | Element::StartLineSuffix => depth += 1,
        Element::End => {
          depth -= 1;
          if depth == 0 {
            return at;
          }
        }
        _ => {}
      }
    }
    self.list.len()
  }

  /// Removes the line breaks that end the document, inside the containers that end it too.
  fn strip_trailing_hard_lines(&mut self) {
    for at in (0..self.list.len()).rev() {
      match self.list[at] {
        Element::End | Element::Nothing | Element::BreakParent => {}
        Element::HardLine | Element::LiteralLine => self.list[at] = Element::Nothing,
        _ => break,
      }
    }
  }

  /// Prettier's `removeLines`, applied to every element from `start` on.
  fn remove_lines_from(&mut self, start: usize) {
    for at in start..self.list.len() {
      match self.list[at] {
        Element::Line => {
          let start = self.texts.len() as u32;
          self.texts.push(b' ');
          self.list[at] = Element::Text(start, 1);
        }
        // An `ifBreak`'s flat contents are empty.
        Element::SoftLine | Element::IfBreak(..) => self.list[at] = Element::Nothing,
        _ => {}
      }
    }
  }

  fn insert(&mut self, at: usize, inserted: &[Element]) {
    let count = inserted.len();
    self.open.iter_mut().filter(|start| **start >= at).for_each(|start| *start += count);
    self.list.splice(at..at, inserted.iter().copied());
  }

  /// Call inside an item of a `fill`. Closes the `fill` and nests it at the start of the first item of a new `fill`.
  /// That item stays open.
  fn nest_fill(&mut self) {
    self.end();
    let Some(&start) = self.open.last() else { return };
    self.end();
    self.insert(start, &[Element::StartFill, Element::StartItem]);
    self.open.extend([start, start + 1]);
  }

  /// Call inside an item of a `fill`. Inserts an empty item and a `hardline` separator before the `fill`'s items.
  fn start_fill_with_hard_line(&mut self) {
    let [.., fill, _] = self.open[..] else { return };
    let items =
      [Element::StartItem, Element::End, Element::StartItem, Element::HardLine, Element::End];
    self.insert(fill + 1, &items);
  }

  /// Ends a `fill` the way `cleanDoc` would leave it: a `fill` with a single item becomes that item.
  fn end_cleaned_fill(&mut self) {
    let has_one_item = self.open.last().is_some_and(|&start| {
      matches!(self.list.get(start + 1), Some(Element::StartItem))
        && self.end_of(start + 1) + 1 == self.list.len()
    });
    match has_one_item {
      true => self.end_fill_as_array(),
      false => self.end(),
    }
  }

  /// Ends a `fill`, but turns it into a plain array of its items.
  fn end_fill_as_array(&mut self) {
    let Some(start) = self.open.pop() else { return };
    self.list[start] = Element::Nothing;
    let mut at = start + 1;
    while let Some(Element::StartItem) = self.list.get(at) {
      let end = self.end_of(at);
      self.list[at] = Element::Nothing;
      if let Some(element) = self.list.get_mut(end) {
        *element = Element::Nothing;
      }
      at = end + 1;
    }
  }

  /// Writes the document to `docs`, with each line break in a text as a literal line if `lines` is set.
  fn write(&self, docs: &mut Docs<'_>, lines: bool) {
    let mut marks = Vec::new();
    let text = |docs: &mut Docs<'_>, start: u32, len: u32| {
      let text = String::from_utf8_lossy(&self.texts[start as usize..(start + len) as usize]);
      if !lines {
        return docs.owned(&text);
      }
      for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
          docs.literalline();
        }
        docs.owned(line);
      }
    };
    for element in &self.list {
      match *element {
        Element::Text(start, len) => text(docs, start, len),
        Element::Line => docs.line(),
        Element::SoftLine => docs.softline(),
        Element::HardLine => docs.hardline(),
        Element::LiteralLine => docs.literalline_without_break_parent(),
        Element::BreakParent => docs.break_parent(),
        Element::LineSuffixBoundary => docs.line_suffix_boundary(),
        Element::IfBreak(start, len) => {
          let mark = docs.if_break(Branch::Break, None);
          text(docs, start, len);
          docs.close(mark);
        }
        Element::Nothing => {}
        Element::StartGroup(broken) => marks.push(docs.group_with(None, broken)),
        Element::StartIndent => marks.push(docs.indent()),
        Element::StartDedent => marks.push(docs.align(Align::Dedent)),
        Element::StartFill => marks.push(docs.fill()),
        Element::StartItem => marks.push(docs.part()),
        Element::StartLineSuffix => marks.push(docs.line_suffix()),
        Element::End => {
          if let Some(mark) = marks.pop() {
            docs.close(mark);
          }
        }
      }
    }
  }
}

pub(crate) struct Sink<'o> {
  /// `None` if the caller only wants the document.
  output: Option<Output<'o>>,
  /// Prettier's `cleanDoc` runs on the document: the style sheet is embedded in HTML.
  is_cleaned: bool,

  /// True if writes go to `builder` instead of straight to the output.
  is_document: bool,
  builder: Builder,
  /// Start positions of the open `removeLines` regions.
  without_lines: Vec<usize>,

  /// How many indents deep the output is.
  depth: usize,
  /// Saved values of `depth`, restored by `end_indent`.
  outer_depths: Vec<usize>,
  /// The number of open groups that are being written flat.
  flat_groups: u32,
  /// The output offset just after the current line's indentation, and the column at that offset.
  line_start: usize,
  line_start_column: usize,
  /// True if a group has been written flat on the current line.
  has_group_in_line: bool,
  /// How many times something other than text has gone to the output.
  line_breaks: u32,
  /// The unit needs a document.
  needs_document: bool,
  /// Where this sink's output starts. Trimming never goes before it.
  start: usize,
  /// What a document is printed with: its arena and its buffers.
  bump: bumpalo::Bump,
  scratch: doc::Scratch,
  is_too_deep: bool,
}

impl<'o> Sink<'o> {
  pub(crate) fn to_output(output: Output<'o>, is_cleaned: bool) -> Self {
    let start = output.out.len();
    Sink { line_start: start, start, ..Sink::new(Some(output), is_cleaned, false) }
  }

  /// A sink that only builds a document, which `write_document` writes.
  pub(crate) fn to_document(is_cleaned: bool) -> Self {
    Sink::new(None, is_cleaned, true)
  }

  fn new(output: Option<Output<'o>>, is_cleaned: bool, is_document: bool) -> Self {
    Sink {
      output,
      is_cleaned,
      is_document,
      builder: Builder::default(),
      without_lines: Vec::new(),
      depth: 0,
      outer_depths: Vec::new(),
      flat_groups: 0,
      line_start: 0,
      line_start_column: 0,
      has_group_in_line: false,
      line_breaks: 0,
      needs_document: false,
      start: 0,
      bump: bumpalo::Bump::new(),
      scratch: doc::Scratch::default(),
      is_too_deep: false,
    }
  }

  /// Everything that a `to_document` sink received, without its trailing `hardline`: Prettier's
  /// `stripTrailingHardline`.
  pub(crate) fn into_document(self) -> Document {
    let mut builder = self.builder;
    builder.strip_trailing_hard_lines();
    Document(builder)
  }

  /// True if the stack ran out while the document was written.
  pub(crate) fn is_too_deep(&self) -> bool {
    self.is_too_deep
  }

  /// Not everything could be written: the nesting is too deep.
  pub(crate) fn fail_too_deep(&mut self) {
    self.is_too_deep = true;
  }

  // ───────────────────────────── units ─────────────────────────────

  /// Starts a unit: output that isn't inside a group and is followed by a line break. Returns `None` if it's already
  /// going to a document, or if it's part of something larger that would be rewritten as a whole.
  pub(crate) fn start_unit(&mut self) -> Option<Mark> {
    let output = self.output.as_ref()?;
    if self.is_document || self.has_group_in_line || self.needs_document || self.flat_groups > 0 {
      return None;
    }
    Some(Mark {
      len: output.out.len(),
      line_start: self.line_start,
      line_start_column: self.line_start_column,
      depth: self.depth,
      outer_depths: self.outer_depths.len(),
    })
  }

  /// Ends a unit. If it returns `false`, the unit must be written again between `start_document` and `end_document`.
  pub(crate) fn end_unit(&mut self) -> bool {
    self.check_line();
    self.has_group_in_line = false;
    !std::mem::take(&mut self.needs_document)
  }

  /// Rolls back the output of the unit that started at `mark`. From now on, writes go to a document.
  pub(crate) fn start_document(&mut self, mark: &Mark) {
    if let Some(output) = &mut self.output {
      output.out.truncate(mark.len);
    }
    self.line_start = mark.line_start;
    self.line_start_column = mark.line_start_column;
    self.depth = mark.depth;
    self.outer_depths.truncate(mark.outer_depths);
    self.flat_groups = 0;
    self.builder.clear();
    self.is_document = true;
  }

  /// True if a text of `len` characters fits on the line.
  pub(crate) fn has_room_for(&self, len: usize) -> bool {
    self.output.as_ref().is_none_or(|output| {
      self.line_start_column + (output.out.len() - self.line_start) + len <= output.options.width
    })
  }

  /// Prints the document that the unit was written to.
  pub(crate) fn end_document(&mut self) {
    self.is_document = false;
    self.line_breaks += 1;
    let column = self.column();
    let Some(output) = &mut self.output else { return };
    self.bump.reset();
    let mut docs = Docs::reuse(&self.bump, &mut self.scratch);
    self.builder.write(&mut docs, false);
    self.line_start_column = doc::print_at(&docs, &output.options, output.out, self.depth, column);
    self.line_start = output.out.len();
    self.scratch = docs.into_scratch();
  }

  fn column(&self) -> usize {
    let line_so_far =
      self.output.as_ref().and_then(|output| output.out.get(self.line_start..)).unwrap_or_default();
    self.line_start_column + doc::width(&String::from_utf8_lossy(line_so_far))
  }

  /// Called when the current line is complete. The unit fails if the line has a flat group on it and is too long.
  fn check_line(&mut self) {
    if self.has_group_in_line
      && self.output.as_ref().is_some_and(|output| self.column() > output.options.width)
    {
      self.needs_document = true;
    }
  }

  // ───────────────────────────── replaying cached output ─────────────────────────────

  /// Returns the current output position for `written_since`, or `None` unless writes go straight to the output.
  pub(crate) fn position(&self) -> Option<(usize, u32)> {
    let output = self.output.as_ref().filter(|_| !self.is_document && !self.needs_document)?;
    Some((output.out.len(), self.line_breaks))
  }

  /// Returns the output since `position` if it's all on one line, and whether that line has a group on it.
  pub(crate) fn written_since(&self, (len, line_breaks): (usize, u32)) -> Option<(&[u8], bool)> {
    let output = self
      .output
      .as_ref()
      .filter(|_| !self.is_document && !self.needs_document && self.line_breaks == line_breaks)?;
    Some((output.out.get(len..)?, self.has_group_in_line))
  }

  /// Writes output that `written_since` returned earlier.
  pub(crate) fn replay_output(&mut self, text: &[u8], has_group: bool) {
    self.text(text);
    self.has_group_in_line |= has_group;
  }

  // ───────────────────────────── the parts of a document ─────────────────────────────

  #[inline]
  pub(crate) fn text(&mut self, text: &[u8]) {
    if self.is_document {
      self.builder.text(text);
    } else if let Some(output) = &mut self.output {
      output.out.extend_from_slice(text);
    }
  }

  #[inline]
  pub(crate) fn token(&mut self, text: &'static str) {
    self.text(text.as_bytes());
  }

  /// Writes a line break outside of any flat group.
  fn write_line_break(&mut self) {
    self.check_line();
    self.has_group_in_line = false;
    self.line_breaks += 1;
    let Some(output) = &mut self.output else { return };
    let blank =
      output.out[self.start..].iter().rev().take_while(|byte| matches!(byte, b' ' | b'\t')).count();
    output.out.truncate(output.out.len() - blank);
    output.out.extend_from_slice(output.options.eol.text().as_bytes());
    let (unit, width) = match output.options.tabs {
      true => ("\t", output.options.tab_width),
      false => (" ", 1),
    };
    let count =
      if output.options.tabs { self.depth } else { self.depth * output.options.tab_width };
    for _ in 0..count {
      output.out.extend_from_slice(unit.as_bytes());
    }
    self.line_start = output.out.len();
    self.line_start_column = count * width;
  }

  pub(crate) fn line(&mut self) {
    if self.is_document {
      self.builder.list.push(Element::Line);
    } else if self.flat_groups > 0 {
      self.token(" ");
    } else {
      self.write_line_break();
    }
  }

  pub(crate) fn soft_line(&mut self) {
    if self.is_document {
      self.builder.list.push(Element::SoftLine);
    } else if self.flat_groups == 0 {
      self.write_line_break();
    }
  }

  pub(crate) fn hard_line(&mut self) {
    if self.is_document {
      self.builder.list.push(Element::HardLine);
    } else if self.flat_groups > 0 {
      self.needs_document = true;
    } else {
      self.write_line_break();
    }
  }

  /// `literallineWithoutBreakParent`
  pub(crate) fn literal_line(&mut self) {
    match self.is_document {
      true => self.builder.list.push(Element::LiteralLine),
      false => self.needs_document = true,
    }
  }

  pub(crate) fn break_parent(&mut self) {
    if self.is_document {
      self.builder.list.push(Element::BreakParent);
    } else if self.flat_groups > 0 {
      self.needs_document = true;
    }
  }

  pub(crate) fn line_suffix_boundary(&mut self) {
    // There's never a line suffix when writing straight to the output.
    if self.is_document {
      self.builder.list.push(Element::LineSuffixBoundary);
    }
  }

  /// `ifBreak(text)`
  pub(crate) fn if_break(&mut self, text: &'static str) {
    if self.is_document {
      self.builder.if_break(text.as_bytes());
    } else if self.flat_groups == 0 {
      self.token(text);
    }
  }

  pub(crate) fn start_group(&mut self, should_break: bool) {
    if self.is_document {
      self.builder.start(Element::StartGroup(should_break));
    } else if self.flat_groups > 0 {
      self.needs_document |= should_break;
      self.flat_groups += 1;
    } else if !should_break {
      self.flat_groups = 1;
      self.has_group_in_line = true;
    }
  }

  pub(crate) fn end_group(&mut self) {
    match self.is_document {
      true => self.builder.end(),
      false => self.flat_groups = self.flat_groups.saturating_sub(1),
    }
  }

  pub(crate) fn start_indent(&mut self) {
    if self.is_document {
      self.builder.start(Element::StartIndent);
    } else if self.flat_groups == 0 {
      self.outer_depths.push(self.depth);
      self.depth += 1;
    }
  }

  pub(crate) fn start_dedent(&mut self) {
    if self.is_document {
      self.builder.start(Element::StartDedent);
    } else if self.flat_groups == 0 {
      self.outer_depths.push(self.depth);
      self.depth = self.depth.saturating_sub(1);
    }
  }

  /// Ends an `indent` or a `dedent`.
  pub(crate) fn end_indent(&mut self) {
    if self.is_document {
      self.builder.end();
    } else if self.flat_groups == 0 {
      self.depth = self.outer_depths.pop().unwrap_or(0);
    }
  }

  /// Starts a `fill` and its first item.
  pub(crate) fn start_fill(&mut self) {
    if self.is_document {
      self.builder.start(Element::StartFill);
      self.builder.start(Element::StartItem);
    } else if self.flat_groups == 0 {
      // Only the doc printer can work out how much fits on each line.
      self.needs_document = true;
    }
  }

  /// Ends an item, writes `separator` and starts the next item.
  pub(crate) fn fill_separator(&mut self, separator: Separator) {
    if !self.is_document {
      return match separator {
        Separator::Line | Separator::DedentedLine => self.token(" "),
        Separator::SoftLine => {}
        Separator::HardLine | Separator::DedentedHardLine => self.needs_document = true,
      };
    }
    let builder = &mut self.builder;
    builder.end();
    builder.start(Element::StartItem);
    let is_dedented = matches!(separator, Separator::DedentedLine | Separator::DedentedHardLine);
    if is_dedented {
      builder.start(Element::StartDedent);
    }
    builder.list.push(match separator {
      Separator::Line | Separator::DedentedLine => Element::Line,
      Separator::SoftLine => Element::SoftLine,
      Separator::HardLine | Separator::DedentedHardLine => Element::HardLine,
    });
    if is_dedented {
      builder.end();
    }
    builder.end();
    builder.start(Element::StartItem);
  }

  /// See `Builder::nest_fill`.
  pub(crate) fn nest_fill(&mut self) {
    if self.is_document {
      self.builder.nest_fill();
    }
  }

  /// Inserts an empty item and a `hardline` separator before the `fill`'s items.
  pub(crate) fn start_fill_with_hard_line(&mut self) {
    if self.is_document {
      self.builder.start_fill_with_hard_line();
    }
  }

  pub(crate) fn end_fill(&mut self) {
    if self.is_document {
      self.builder.end();
      match self.is_cleaned {
        true => self.builder.end_cleaned_fill(),
        false => self.builder.end(),
      }
    }
  }

  /// Ends a `fill`, but turns it into a plain array of its items.
  pub(crate) fn end_fill_as_array(&mut self) {
    if self.is_document {
      self.builder.end();
      self.builder.end_fill_as_array();
    }
  }

  pub(crate) fn start_line_suffix(&mut self) {
    match self.is_document {
      true => self.builder.start(Element::StartLineSuffix),
      false => self.needs_document = true,
    }
  }

  pub(crate) fn end_line_suffix(&mut self) {
    if self.is_document {
      self.builder.end();
    }
  }

  /// Starts a region that `removeLines` is applied to.
  pub(crate) fn start_without_lines(&mut self) {
    match self.is_document {
      true => self.without_lines.push(self.builder.list.len()),
      false => self.needs_document = true,
    }
  }

  pub(crate) fn end_without_lines(&mut self) {
    if self.is_document
      && let Some(start) = self.without_lines.pop()
    {
      self.builder.remove_lines_from(start);
    }
  }
}
