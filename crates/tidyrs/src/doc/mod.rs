//! Prettier's document IR (`src/document`), written as one flat stream.
//!
//! A doc is emitted in order: a container (a group, an indent, a fill, ...) is opened, its contents are emitted, and it
//! is closed, which records where it ends. A concatenation is elements next to each other. The printer walks ranges of
//! the stream the way Prettier's walks docs, so the output is Prettier's.

mod print;
mod width;
mod width_tables;

use bumpalo::Bump;

pub(crate) use print::print_at;
pub use print::{Eol, Options, print};
pub use width::width;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GroupId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
  /// `line`: a space when flat.
  Space,
  /// `softline`: nothing when flat.
  Soft,
  /// `hardlineWithoutBreakParent`.
  Hard,
  /// `literallineWithoutBreakParent`: a line break back to the root indent.
  Literal,
}

#[derive(Clone, Copy, Debug)]
pub enum Align<'a> {
  Width(usize),
  Text(&'a str),
  /// `dedent`: drops the innermost indent.
  Dedent,
  /// `dedentToRoot`.
  Root,
  /// `markAsRoot`.
  Mark,
}

/// Which contents of `ifBreak` a branch is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branch {
  /// `breakContents`
  Break,
  /// `flatContents`
  Flat,
}

/// `align`'s argument, with a text as an index into the texts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Margin {
  Width(u32),
  Text(u32),
  Dedent,
  Root,
  Mark,
}

/// No group id.
pub(crate) const NONE: u32 = u32::MAX;

/// An element of the stream. A container's contents are the elements after it, up to its `end`.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Element {
  /// Texts next to each other, from `start` on in the texts, and their total width: a concatenation of texts prints
  /// and measures the same as that.
  Text {
    start: u32,
    count: u32,
    width: u32,
  },
  Line(Line),
  LineSuffixBoundary,
  Trim,
  /// `id` is [`NONE`] if there's none. A conditional group's contents are its states, each a [`Element::Part`].
  Group {
    end: u32,
    id: u32,
    broken: bool,
    states: bool,
  },
  /// Its contents are parts.
  Fill {
    end: u32,
  },
  /// A part of a fill, or a state of a conditional group.
  Part {
    end: u32,
  },
  Indent {
    end: u32,
  },
  Align {
    end: u32,
    margin: Margin,
  },
  /// One of the contents of `ifBreak`.
  IfBreak {
    end: u32,
    id: u32,
    branch: Branch,
  },
  IndentIfBreak {
    end: u32,
    id: u32,
    negate: bool,
  },
  LineSuffix {
    end: u32,
  },
}

const _: () = assert!(std::mem::size_of::<Element>() <= 16);

/// An open container, which [`Docs::close`] closes.
#[must_use]
#[derive(Clone, Copy, Debug)]
pub struct Mark(u32);

/// The buffers of a [`Docs`], kept from one run to the next.
#[derive(Default)]
pub struct Scratch {
  elements: Vec<Element>,
  texts: Vec<(&'static str, u32)>,
  open: Vec<u32>,
}

/// An empty vector with the buffer of `vec`, for another lifetime: collecting in place keeps the buffer.
pub(crate) fn recycle<T, U>(mut vec: Vec<T>) -> Vec<U> {
  vec.clear();
  vec.into_iter().map(|_| unreachable!()).collect()
}

/// The stream of one format run, and the texts it refers to.
pub struct Docs<'a> {
  bump: &'a Bump,
  elements: Vec<Element>,
  /// Each text and its width.
  texts: Vec<(&'a str, u32)>,
  /// The groups that are open, innermost last, for `propagateBreaks`.
  open: Vec<u32>,
  /// The last element, if it is a text that more texts can join.
  run: u32,
  groups: u32,
  /// Whether a `trim` was emitted, which `fits` then has to account for.
  pub(crate) trims: bool,
}

impl<'a> Docs<'a> {
  /// The element the printer uses for the line break a line suffix boundary asks for.
  pub(crate) const HARD: u32 = 0;

  pub fn new(bump: &'a Bump) -> Self {
    Self::reuse(bump, &mut Scratch::default())
  }

  /// A stream that takes the buffers of `scratch`, which `into_scratch` gives back.
  pub fn reuse(bump: &'a Bump, scratch: &mut Scratch) -> Self {
    let mut elements = std::mem::take(&mut scratch.elements);
    elements.clear();
    elements.push(Element::Line(Line::Hard));
    let mut open = std::mem::take(&mut scratch.open);
    open.clear();
    let texts = recycle(std::mem::take(&mut scratch.texts));
    Self { bump, elements, texts, open, run: NONE, groups: 0, trims: false }
  }

  /// The buffers of this stream, for the next one.
  pub fn into_scratch(self) -> Scratch {
    Scratch { elements: self.elements, texts: recycle(self.texts), open: self.open }
  }

  /// Makes room for the docs of a text about `size` bytes long.
  pub fn reserve(&mut self, size: usize) {
    self.elements.reserve(size / 4);
    self.texts.reserve(size / 8);
  }

  pub(crate) fn len(&self) -> u32 {
    self.elements.len() as u32
  }

  pub(crate) fn element(&self, at: u32) -> Element {
    self.elements[at as usize]
  }

  /// A text and its width.
  pub(crate) fn text_at(&self, index: u32) -> (&'a str, u32) {
    self.texts[index as usize]
  }

  pub fn group_id(&mut self) -> GroupId {
    self.groups += 1;
    GroupId(self.groups - 1)
  }

  fn emit(&mut self, element: Element) {
    self.run = NONE;
    self.elements.push(element);
  }

  fn open(&mut self, element: Element) -> Mark {
    self.run = NONE;
    self.elements.push(element);
    Mark(self.elements.len() as u32 - 1)
  }

  // ───────────────────────────── leaves ─────────────────────────────

  pub fn text(&mut self, text: &'a str) {
    if text.is_empty() {
      return;
    }
    self.sized(text, width(text) as u32);
  }

  /// A text whose width is known.
  pub(crate) fn sized(&mut self, text: &'a str, size: u32) {
    self.texts.push((text, size));
    if let Some(Element::Text { count, width, .. }) = self.elements.get_mut(self.run as usize) {
      *count += 1;
      *width += size;
      return;
    }
    let start = self.texts.len() as u32 - 1;
    self.elements.push(Element::Text { start, count: 1, width: size });
    self.run = self.elements.len() as u32 - 1;
  }

  /// A text this stream keeps.
  pub fn owned(&mut self, text: &str) {
    if text.is_empty() {
      return;
    }
    let text = self.bump.alloc_str(text);
    self.text(text);
  }

  pub fn line(&mut self) {
    self.emit(Element::Line(Line::Space));
  }

  pub fn softline(&mut self) {
    self.emit(Element::Line(Line::Soft));
  }

  pub fn hardline(&mut self) {
    self.emit(Element::Line(Line::Hard));
    self.break_parent();
  }

  pub fn literalline(&mut self) {
    self.emit(Element::Line(Line::Literal));
    self.break_parent();
  }

  pub fn hardline_without_break_parent(&mut self) {
    self.emit(Element::Line(Line::Hard));
  }

  pub fn literalline_without_break_parent(&mut self) {
    self.emit(Element::Line(Line::Literal));
  }

  /// `breakParent`: breaks the innermost open group, unless that's a conditional group.
  pub fn break_parent(&mut self) {
    let Some(&group) = self.open.last() else { return };
    if let Element::Group { broken, states: false, .. } = &mut self.elements[group as usize] {
      *broken = true;
    }
  }

  pub fn trim(&mut self) {
    self.trims = true;
    self.emit(Element::Trim);
  }

  pub fn line_suffix_boundary(&mut self) {
    self.emit(Element::LineSuffixBoundary);
  }

  // ───────────────────────────── containers ─────────────────────────────

  pub fn group(&mut self) -> Mark {
    self.group_with(None, false)
  }

  pub fn group_with(&mut self, id: Option<GroupId>, broken: bool) -> Mark {
    let id = id.map_or(NONE, |id| id.0);
    let mark = self.open(Element::Group { end: 0, id, broken, states: false });
    self.open.push(mark.0);
    mark
  }

  /// `conditionalGroup`: each state is a [`Docs::part`].
  pub fn conditional(&mut self) -> Mark {
    let mark = self.open(Element::Group { end: 0, id: NONE, broken: false, states: true });
    self.open.push(mark.0);
    mark
  }

  /// `fill`: each part is a [`Docs::part`], contents and separators taking turns.
  pub fn fill(&mut self) -> Mark {
    self.open(Element::Fill { end: 0 })
  }

  pub fn part(&mut self) -> Mark {
    self.open(Element::Part { end: 0 })
  }

  pub fn indent(&mut self) -> Mark {
    self.open(Element::Indent { end: 0 })
  }

  pub fn align(&mut self, align: Align<'a>) -> Mark {
    let margin = match align {
      Align::Width(width) => Margin::Width(width as u32),
      Align::Text(text) => {
        self.texts.push((text, width(text) as u32));
        Margin::Text(self.texts.len() as u32 - 1)
      }
      Align::Dedent => Margin::Dedent,
      Align::Root => Margin::Root,
      Align::Mark => Margin::Mark,
    };
    self.open(Element::Align { end: 0, margin })
  }

  /// One of the contents of `ifBreak(breakContents, flatContents, { groupId })`.
  pub fn if_break(&mut self, branch: Branch, id: Option<GroupId>) -> Mark {
    self.open(Element::IfBreak { end: 0, id: id.map_or(NONE, |id| id.0), branch })
  }

  pub fn indent_if_break(&mut self, id: GroupId, negate: bool) -> Mark {
    self.open(Element::IndentIfBreak { end: 0, id: id.0, negate })
  }

  pub fn line_suffix(&mut self) -> Mark {
    self.open(Element::LineSuffix { end: 0 })
  }

  /// Closes the container that `mark` opened. A broken group breaks the group around it.
  pub fn close(&mut self, mark: Mark) {
    self.run = NONE;
    let end = self.elements.len() as u32;
    let broken = match &mut self.elements[mark.0 as usize] {
      Element::Group { end: at, broken, .. } => {
        *at = end;
        self.open.pop();
        *broken
      }
      Element::Fill { end: at }
      | Element::Part { end: at }
      | Element::Indent { end: at }
      | Element::Align { end: at, .. }
      | Element::IfBreak { end: at, .. }
      | Element::IndentIfBreak { end: at, .. }
      | Element::LineSuffix { end: at } => {
        *at = end;
        false
      }
      _ => unreachable!("not a container"),
    };
    if broken {
      self.break_parent();
    }
  }
}
