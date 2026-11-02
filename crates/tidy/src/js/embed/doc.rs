//! Writes a document of one of tidy's printers, a `crate::doc` stream, into the JavaScript document. Both are
//! Prettier's doc, but `js::ir` prints line breaks its own way: two in a row are one, and a line break that is printed
//! as text doesn't indent. So runs of line breaks are written as the lines Prettier would print.

use crate::doc::{Branch, Docs, Element, Line, Margin, NONE};
use crate::js::base::*;
use crate::js::ir::element::{
  Align, Condition, DedentMode, Group, GroupMode, LineMode, PrintMode, TextWidth,
};

/// Writes the texts of a document.
pub(crate) trait Texts<'a> {
  /// Writes `text`, which Prettier measures as `width` columns.
  fn write(&mut self, text: &str, width: u32, f: &mut Formatter<'a>);
}

/// Writes `text`, which Prettier measures as `width` columns, as one string.
pub(crate) fn write_string(text: &[u8], width: u32, f: &mut Formatter<'_>) {
  let width = match crate::js::strings::contains_char(text, b'\n') {
    true => TextWidth::multiline_string(width),
    false => TextWidth::single(width),
  };
  f.write_text(text, Some(width));
}

/// What ends where a container of the stream ends.
#[derive(Copy, Clone, PartialEq)]
enum Close {
  Nothing,
  Group {
    broken: bool,
  },
  Fill,
  Entry,
  Indent,
  Align,
  Dedent(DedentMode),
  /// `count` dedents by one level.
  Dedents(u32),
  Mark,
  Condition,
  IndentIfBreak(GroupId),
  LineSuffix,
  /// The first state of a conditional group: the printer goes on at the group's end.
  Jump(u32),
}

struct Frame {
  end: u32,
  close: Close,
  /// `Writer::depth` before the container.
  depth: u32,
}

/// How a document's stream is written.
pub(crate) struct Writer<'d, 'x> {
  docs: &'d Docs<'x>,
  /// The JavaScript group of each group id of the stream.
  ids: Vec<Option<GroupId>>,
  frames: Vec<Frame>,
  /// The line breaks written since the last text, which `js::ir` doesn't print twice.
  breaks: u32,
  /// The frame of the group of the soft line that was written last, if nothing came after it.
  soft: Option<usize>,
  /// The indents and aligns around what is written.
  depth: u32,
  /// `depth` at each `markAsRoot` around what is written, innermost last.
  roots: Vec<u32>,
  /// The document's root is in a group that breaks.
  broken: bool,
}

impl<'d, 'x> Writer<'d, 'x> {
  /// `breaks` are the line breaks right before the document. `broken` tells that the document is in a group that
  /// breaks.
  pub(crate) fn new(docs: &'d Docs<'x>, breaks: u32, broken: bool) -> Self {
    Self {
      docs,
      ids: Vec::new(),
      frames: Vec::new(),
      breaks,
      soft: None,
      depth: 0,
      roots: Vec::new(),
      broken,
    }
  }

  /// Writes the elements from `start` to `end`, which must hold whole containers.
  pub(crate) fn write<'a>(
    &mut self,
    start: u32,
    end: u32,
    texts: &mut impl Texts<'a>,
    f: &mut Formatter<'a>,
  ) {
    let mut pos = start;
    loop {
      while let Some(frame) = self.frames.pop_if(|frame| frame.end <= pos) {
        if let Some(to) = self.close(frame, f) {
          pos = to;
        }
      }
      if pos >= end {
        break;
      }
      let element = self.docs.element(pos);
      pos += 1;
      match element {
        Element::Text { start, count, width } => {
          let text = run(self.docs, start, count);
          if text.is_empty() {
            continue;
          }
          texts.write(&text, width, f);
          self.breaks = u32::from(text.ends_with('\n'));
          self.soft = None;
        }
        Element::Line(line) => self.line(line, f),
        Element::LineSuffixBoundary => f.write_element(FormatElement::LineSuffixBoundary),
        Element::Trim => {}
        Element::Group { end, id, broken, states } => {
          let mode = if broken { GroupMode::Expand } else { GroupMode::Flat };
          let id = (id != NONE).then(|| self.id(id, f));
          self.open(Tag::StartGroup(Group::new().with_id(id).with_mode(mode)), f);
          self.push(end, Close::Group { broken });
          if states && let Element::Part { end: first } = self.docs.element(pos) {
            self.push(first, Close::Jump(end));
            pos += 1;
          }
        }
        Element::Fill { end } => {
          self.open(Tag::StartFill, f);
          self.push(end, Close::Fill);
        }
        Element::Part { end } => match self.frames.last() {
          Some(Frame { close: Close::Fill, .. }) => {
            self.open(Tag::StartEntry, f);
            self.push(end, Close::Entry);
          }
          _ => self.push(end, Close::Nothing),
        },
        Element::Indent { end } => {
          f.write_element(FormatElement::Tag(Tag::StartIndent));
          self.push(end, Close::Indent);
          self.depth += 1;
        }
        Element::Align { end, margin } => self.align(end, margin, f),
        Element::IfBreak { end, id, branch } => {
          if pos == end {
            continue;
          }
          let mode = match branch {
            Branch::Break => PrintMode::Expanded,
            Branch::Flat => PrintMode::Flat,
          };
          let id = (id != NONE).then(|| self.id(id, f));
          let condition = Condition::new(mode).with_group_id(id);
          self.open(Tag::StartConditionalContent(condition), f);
          self.push(end, Close::Condition);
        }
        Element::IndentIfBreak { end, id, negate } => match negate {
          true => self.push(end, Close::Nothing),
          false => {
            let id = self.id(id, f);
            f.write_element(FormatElement::Tag(Tag::StartIndentIfGroupBreaks(id)));
            self.push(end, Close::IndentIfBreak(id));
          }
        },
        Element::LineSuffix { end } => {
          self.open(Tag::StartLineSuffix, f);
          self.push(end, Close::LineSuffix);
        }
      }
    }
  }

  fn push(&mut self, end: u32, close: Close) {
    self.frames.push(Frame { end, close, depth: self.depth });
  }

  /// Writes the start of a container whose line breaks a soft line before it doesn't share.
  fn open(&mut self, tag: Tag, f: &mut Formatter<'_>) {
    self.soft = None;
    f.write_element(FormatElement::Tag(tag));
  }

  fn id(&mut self, id: u32, f: &Formatter<'_>) -> GroupId {
    let index = id as usize;
    if self.ids.len() <= index {
      self.ids.resize(index + 1, None);
    }
    *self.ids[index].get_or_insert_with(|| f.group_id("embedded"))
  }

  fn align(&mut self, end: u32, margin: Margin, f: &mut Formatter<'_>) {
    let tag = |tag: Tag, f: &mut Formatter<'_>| f.write_element(FormatElement::Tag(tag));
    let width = match margin {
      Margin::Width(width) => width,
      Margin::Text(index) => self.docs.text_at(index).1,
      Margin::Dedent => {
        tag(Tag::StartDedent(DedentMode::Level), f);
        self.push(end, Close::Dedent(DedentMode::Level));
        self.depth = self.depth.saturating_sub(1);
        return;
      }
      Margin::Root => {
        match self.roots.last() {
          Some(&root) => {
            let count = self.depth.saturating_sub(root);
            (0..count).for_each(|_| tag(Tag::StartDedent(DedentMode::Level), f));
            self.push(end, Close::Dedents(count));
            self.depth = root;
          }
          None => {
            tag(Tag::StartDedent(DedentMode::Root), f);
            self.push(end, Close::Dedent(DedentMode::Root));
            self.depth = 0;
          }
        }
        return;
      }
      Margin::Mark => {
        self.push(end, Close::Mark);
        self.roots.push(self.depth);
        return;
      }
    };
    match width {
      0 => self.push(end, Close::Nothing),
      _ => {
        tag(Tag::StartAlign(Align(width)), f);
        self.push(end, Close::Align);
        self.depth += 1;
      }
    }
  }

  /// Writes the end of the container of `frame`. Returns where to go on if that isn't its end.
  fn close(&mut self, frame: Frame, f: &mut Formatter<'_>) -> Option<u32> {
    let tag = match frame.close {
      Close::Nothing => None,
      Close::Group { .. } => Some(Tag::EndGroup),
      Close::Fill => Some(Tag::EndFill),
      Close::Entry => Some(Tag::EndEntry),
      Close::Indent => Some(Tag::EndIndent),
      Close::Align => Some(Tag::EndAlign),
      Close::Dedent(mode) => Some(Tag::EndDedent(mode)),
      Close::Dedents(count) => {
        (0..count)
          .for_each(|_| f.write_element(FormatElement::Tag(Tag::EndDedent(DedentMode::Level))));
        None
      }
      Close::Mark => {
        self.roots.pop();
        None
      }
      Close::Condition => Some(Tag::EndConditionalContent),
      Close::IndentIfBreak(id) => Some(Tag::EndIndentIfGroupBreaks(id)),
      Close::LineSuffix => Some(Tag::EndLineSuffix),
      Close::Jump(to) => return Some(to),
    };
    self.depth = frame.depth;
    if let Some(tag) = tag {
      if matches!(tag, Tag::EndGroup | Tag::EndFill | Tag::EndEntry) {
        self.soft = None;
      }
      f.write_element(FormatElement::Tag(tag));
    }
    None
  }

  /// Writes a line break of the stream. Prettier prints each one that breaks, so two in a row make an empty line.
  fn line(&mut self, line: Line, f: &mut Formatter<'_>) {
    let group = self
      .frames
      .iter()
      .rposition(|frame| matches!(frame.close, Close::Group { .. } | Close::Fill));
    let breaks = match group {
      Some(at) => self.frames[at].close == Close::Group { broken: true },
      None => self.broken,
    };
    match line {
      Line::Literal => return self.literal(f),
      Line::Space | Line::Soft if !breaks => {
        let group = group.unwrap_or(usize::MAX);
        let empty = self.breaks > 0 || self.soft == Some(group);
        let mode = match (line, empty) {
          (Line::Space, false) => LineMode::SoftOrSpace,
          (Line::Space, true) => LineMode::SoftOrSpaceEmpty,
          (_, false) => LineMode::Soft,
          (_, true) => LineMode::SoftEmpty,
        };
        f.write_element(FormatElement::Line(mode));
        self.soft = Some(group);
        return;
      }
      _ => {}
    }
    match self.breaks {
      0 => f.write_element(FormatElement::Line(LineMode::Hard)),
      1 => f.write_element(FormatElement::Line(LineMode::Empty)),
      _ => {
        f.write_element(FormatElement::Tag(Tag::StartDedent(DedentMode::Root)));
        f.write_element(FormatElement::Line(LineMode::Hard));
        f.write_text(b"\n", Some(TextWidth::multiline(0)));
        f.write_element(FormatElement::Tag(Tag::EndDedent(DedentMode::Root)));
        f.write_element(FormatElement::Line(LineMode::Hard));
      }
    }
    self.breaks += 1;
    self.soft = None;
  }

  /// A `literalline`: a line break printed as text, which keeps the white space before it, and the indent of the
  /// innermost `markAsRoot`.
  fn literal(&mut self, f: &mut Formatter<'_>) {
    f.write_text(b"\n", Some(TextWidth::multiline(0)));
    if let Some(&root) = self.roots.last() {
      let count = self.depth.saturating_sub(root);
      let dedent = |tag: Tag, f: &mut Formatter<'_>| {
        (0..count).for_each(|_| f.write_element(FormatElement::Tag(tag)))
      };
      dedent(Tag::StartDedent(DedentMode::Level), f);
      f.write_element(FormatElement::Line(LineMode::Hard));
      dedent(Tag::EndDedent(DedentMode::Level), f);
    }
    self.breaks = 1;
    self.soft = None;
  }
}

/// The texts from `start` on, joined: Prettier's `cleanDoc` joins the strings next to each other.
fn run<'x>(docs: &Docs<'x>, start: u32, count: u32) -> std::borrow::Cow<'x, str> {
  match count {
    1 => docs.text_at(start).0.into(),
    _ => (start..start + count).map(|index| docs.text_at(index).0).collect::<String>().into(),
  }
}

/// Calls `write` with each run of texts of `docs`.
pub(crate) fn each_text(docs: &Docs<'_>, mut write: impl FnMut(&str)) {
  for pos in 1..docs.len() {
    if let Element::Text { start, count, .. } = docs.element(pos) {
      write(&run(docs, start, count));
    }
  }
}
