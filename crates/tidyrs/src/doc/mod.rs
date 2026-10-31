//! Prettier's document IR (`src/document`), stored flat in an arena.
//!
//! A [`Doc`] is an index into [`Docs`]. Builders mirror Prettier's: the same doc may appear in
//! several places, as in Prettier, and keeps its identity there.

mod print;
mod width;
mod width_tables;

use bumpalo::Bump;

pub use print::{Eol, Options, print};
pub use width::width;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Doc(u32);

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

#[derive(Clone, Copy, Debug)]
pub(crate) struct List {
  start: u32,
  len: u32,
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

/// No group id, or no conditional states.
pub(crate) const NONE: u32 = u32::MAX;

/// A doc, in 16 bytes: texts and conditional states are in tables of their own.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Node {
  /// An index into the texts.
  Text(u32),
  Concat(List),
  Fill(List),
  Indent(Doc),
  Align(Margin, Doc),
  /// `id` and `states`, an index into the states, are [`NONE`] if there's none.
  Group {
    contents: Doc,
    id: u32,
    states: u32,
    broken: bool,
  },
  IfBreak {
    broken: Doc,
    flat: Doc,
    id: u32,
  },
  IndentIfBreak {
    contents: Doc,
    id: GroupId,
    negate: bool,
  },
  LineSuffix(Doc),
  LineSuffixBoundary,
  Line(Line),
  BreakParent,
  Trim,
}

const _: () = assert!(std::mem::size_of::<Node>() == 16);

/// The buffers of a [`Docs`], kept from one run to the next.
#[derive(Default)]
pub struct Scratch {
  nodes: Vec<Node>,
  lists: Vec<Doc>,
  broken: Vec<bool>,
  texts: Vec<(&'static str, u32)>,
}

/// An empty vector with the buffer of `vec`, for another lifetime: collecting in place keeps the buffer.
pub(crate) fn recycle<T, U>(mut vec: Vec<T>) -> Vec<U> {
  vec.clear();
  vec.into_iter().map(|_| unreachable!()).collect()
}

/// The arena that owns every doc of one format run.
pub struct Docs<'a> {
  bump: &'a Bump,
  nodes: Vec<Node>,
  lists: Vec<Doc>,
  /// For each node, whether it breaks the group around it.
  broken: Vec<bool>,
  /// Each text and its width.
  texts: Vec<(&'a str, u32)>,
  /// The states of conditional groups.
  states: Vec<List>,
  groups: u32,
  /// Whether a `trim` was made, which `fits` then has to account for.
  pub(crate) trims: bool,
}

impl<'a> Docs<'a> {
  pub const EMPTY: Doc = Doc(0);
  /// `hardlineWithoutBreakParent`. Like Prettier's, the lines and `breakParent` are shared.
  pub(crate) const HARD: Doc = Doc(1);
  const LINE: Doc = Doc(2);
  const SOFT: Doc = Doc(3);
  const LITERAL: Doc = Doc(4);
  const BREAK_PARENT: Doc = Doc(5);
  const HARDLINE: Doc = Doc(6);
  const LITERALLINE: Doc = Doc(7);

  pub fn new(bump: &'a Bump) -> Self {
    Self::reuse(bump, &mut Scratch::default())
  }

  /// An arena that takes the buffers of `scratch`, which `into_scratch` gives back.
  pub fn reuse(bump: &'a Bump, scratch: &mut Scratch) -> Self {
    let mut nodes = std::mem::take(&mut scratch.nodes);
    nodes.clear();
    nodes.extend([
      Node::Text(0),
      Node::Line(Line::Hard),
      Node::Line(Line::Space),
      Node::Line(Line::Soft),
      Node::Line(Line::Literal),
      Node::BreakParent,
      Node::Concat(List { start: 0, len: 2 }),
      Node::Concat(List { start: 2, len: 2 }),
    ]);
    let mut lists = std::mem::take(&mut scratch.lists);
    lists.clear();
    lists.extend([Self::HARD, Self::BREAK_PARENT, Self::LITERAL, Self::BREAK_PARENT]);
    let mut broken = std::mem::take(&mut scratch.broken);
    broken.clear();
    broken.extend([false, false, false, false, false, true, true, true]);
    let mut texts = recycle(std::mem::take(&mut scratch.texts));
    texts.push(("", 0));
    Self { bump, nodes, lists, broken, texts, states: Vec::new(), groups: 0, trims: false }
  }

  /// The buffers of this arena, for the next one.
  pub fn into_scratch(self) -> Scratch {
    Scratch {
      nodes: self.nodes,
      lists: self.lists,
      broken: self.broken,
      texts: recycle(self.texts),
    }
  }

  /// Makes room for the docs of a text about `size` bytes long.
  pub fn reserve(&mut self, size: usize) {
    self.nodes.reserve(size / 2);
    self.broken.reserve(size / 2);
    self.lists.reserve(size / 2);
    self.texts.reserve(size / 8);
  }

  /// An arena with room for the docs of a text about `size` bytes long.
  pub fn with_capacity(bump: &'a Bump, size: usize) -> Self {
    let mut docs = Self::new(bump);
    docs.reserve(size);
    docs
  }

  fn push(&mut self, mut node: Node) -> Doc {
    // `propagateBreaks`, done as the docs are made: a `breakParent` breaks the group around it, a broken group breaks
    // the group around it, and a conditional group stops both.
    let breaks = match &mut node {
      Node::BreakParent => true,
      Node::Concat(list) | Node::Fill(list) => {
        self.parts(*list).iter().any(|&part| self.breaks(part))
      }
      Node::Group { broken, states, .. } if *states != NONE => *broken,
      Node::Group { contents, broken, .. } => {
        *broken |= self.breaks(*contents);
        *broken
      }
      Node::IfBreak { broken, flat, .. } => self.breaks(*broken) || self.breaks(*flat),
      Node::Indent(contents)
      | Node::Align(_, contents)
      | Node::IndentIfBreak { contents, .. }
      | Node::LineSuffix(contents) => self.breaks(*contents),
      _ => false,
    };
    self.broken.push(breaks);
    self.nodes.push(node);
    Doc(self.nodes.len() as u32 - 1)
  }

  /// Whether `doc` breaks the group around it.
  fn breaks(&self, doc: Doc) -> bool {
    self.broken[doc.0 as usize]
  }

  fn list(&mut self, parts: &[Doc]) -> List {
    let start = self.lists.len() as u32;
    self.lists.extend_from_slice(parts);
    List { start, len: parts.len() as u32 }
  }

  pub(crate) fn node(&self, doc: Doc) -> Node {
    self.nodes[doc.0 as usize]
  }

  pub(crate) fn parts(&self, list: List) -> &[Doc] {
    &self.lists[list.start as usize..(list.start + list.len) as usize]
  }

  /// A text and its width.
  pub(crate) fn text_at(&self, index: u32) -> (&'a str, u32) {
    self.texts[index as usize]
  }

  /// The states of a conditional group.
  pub(crate) fn states(&self, index: u32) -> &[Doc] {
    self.parts(self.states[index as usize])
  }

  pub fn group_id(&mut self) -> GroupId {
    self.groups += 1;
    GroupId(self.groups - 1)
  }

  pub fn text(&mut self, text: &'a str) -> Doc {
    if text.is_empty() {
      return Self::EMPTY;
    }
    self.texts.push((text, width(text) as u32));
    self.push(Node::Text(self.texts.len() as u32 - 1))
  }

  /// A text this arena keeps.
  pub fn owned(&mut self, text: &str) -> Doc {
    let text = self.bump.alloc_str(text);
    self.text(text)
  }

  /// The parts in order. Empty parts are left out: they print nothing and `fits` skips them.
  pub fn concat(&mut self, parts: &[Doc]) -> Doc {
    let start = self.lists.len();
    self.lists.extend(parts.iter().filter(|&&part| part != Self::EMPTY));
    match self.lists.len() - start {
      0 => Self::EMPTY,
      1 => self.lists.pop().unwrap(),
      len => self.push(Node::Concat(List { start: start as u32, len: len as u32 })),
    }
  }

  pub fn fill(&mut self, parts: &[Doc]) -> Doc {
    let list = self.list(parts);
    self.push(Node::Fill(list))
  }

  pub fn indent(&mut self, contents: Doc) -> Doc {
    self.push(Node::Indent(contents))
  }

  pub fn align(&mut self, align: Align<'a>, contents: Doc) -> Doc {
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
    self.push(Node::Align(margin, contents))
  }

  pub fn group(&mut self, contents: Doc) -> Doc {
    self.push(Node::Group { contents, id: NONE, states: NONE, broken: false })
  }

  pub fn group_with(&mut self, contents: Doc, id: Option<GroupId>, broken: bool) -> Doc {
    let id = id.map_or(NONE, |id| id.0);
    self.push(Node::Group { contents, id, states: NONE, broken })
  }

  pub fn conditional(&mut self, states: &[Doc]) -> Doc {
    let list = self.list(states);
    self.states.push(list);
    let index = self.states.len() as u32 - 1;
    self.push(Node::Group { contents: states[0], id: NONE, states: index, broken: false })
  }

  pub fn if_break(&mut self, broken: Doc, flat: Doc, id: Option<GroupId>) -> Doc {
    self.push(Node::IfBreak { broken, flat, id: id.map_or(NONE, |id| id.0) })
  }

  pub fn indent_if_break(&mut self, contents: Doc, id: GroupId, negate: bool) -> Doc {
    self.push(Node::IndentIfBreak { contents, id, negate })
  }

  pub fn line_suffix(&mut self, contents: Doc) -> Doc {
    self.push(Node::LineSuffix(contents))
  }

  pub fn line_suffix_boundary(&mut self) -> Doc {
    self.push(Node::LineSuffixBoundary)
  }

  pub fn line(&mut self) -> Doc {
    Self::LINE
  }

  pub fn softline(&mut self) -> Doc {
    Self::SOFT
  }

  pub fn hardline(&mut self) -> Doc {
    Self::HARDLINE
  }

  pub fn literalline(&mut self) -> Doc {
    Self::LITERALLINE
  }

  pub fn hardline_without_break_parent(&mut self) -> Doc {
    Self::HARD
  }

  pub fn literalline_without_break_parent(&mut self) -> Doc {
    Self::LITERAL
  }

  pub fn break_parent(&mut self) -> Doc {
    Self::BREAK_PARENT
  }

  pub fn trim(&mut self) -> Doc {
    self.trims = true;
    self.push(Node::Trim)
  }

  pub fn join(&mut self, separator: Doc, parts: &[Doc]) -> Doc {
    let mut joined = Vec::with_capacity(parts.len() * 2);
    for (index, part) in parts.iter().enumerate() {
      if index > 0 {
        joined.push(separator);
      }
      joined.push(*part);
    }
    self.concat(&joined)
  }

  /// `willBreak`.
  pub fn will_break(&self, doc: Doc) -> bool {
    self.find(doc, &mut |node| match node {
      Node::Group { broken, .. } => broken,
      Node::Line(line) => matches!(line, Line::Hard | Line::Literal),
      Node::BreakParent => true,
      _ => false,
    })
  }

  /// `canBreak`.
  pub fn can_break(&self, doc: Doc) -> bool {
    self.find(doc, &mut |node| matches!(node, Node::Line(_)))
  }

  /// `traverseDoc` without conditional states, stopping at the first doc `found` matches.
  fn find(&self, doc: Doc, found: &mut impl FnMut(Node) -> bool) -> bool {
    let mut stack = vec![doc];
    while let Some(doc) = stack.pop() {
      let node = self.node(doc);
      if found(node) {
        return true;
      }
      match node {
        Node::Concat(list) | Node::Fill(list) => stack.extend(self.parts(list).iter().rev()),
        Node::IfBreak { broken, flat, .. } => stack.extend([flat, broken]),
        Node::Group { contents, .. }
        | Node::Indent(contents)
        | Node::Align(_, contents)
        | Node::IndentIfBreak { contents, .. }
        | Node::LineSuffix(contents) => stack.push(contents),
        _ => {}
      }
    }
    false
  }
}
