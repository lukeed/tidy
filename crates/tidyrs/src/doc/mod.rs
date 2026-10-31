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

#[derive(Clone, Copy, Debug)]
pub(crate) enum Node<'a> {
  /// A text and its width.
  Text(&'a str, u32),
  Concat(List),
  Fill(List),
  Indent(Doc),
  Align(Align<'a>, Doc),
  Group {
    contents: Doc,
    id: Option<GroupId>,
    broken: bool,
    states: Option<List>,
  },
  IfBreak {
    broken: Doc,
    flat: Doc,
    id: Option<GroupId>,
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
  Label(&'static str, Doc),
}

/// The buffers of a [`Docs`], kept from one run to the next.
#[derive(Default)]
pub struct Scratch {
  nodes: Vec<Node<'static>>,
  lists: Vec<Doc>,
  broken: Vec<bool>,
}

/// An empty vector with the buffer of `vec`, for another lifetime: collecting in place keeps the buffer.
pub(crate) fn recycle<T, U>(mut vec: Vec<T>) -> Vec<U> {
  vec.clear();
  vec.into_iter().map(|_| unreachable!()).collect()
}

/// The arena that owns every doc of one format run.
pub struct Docs<'a> {
  bump: &'a Bump,
  nodes: Vec<Node<'a>>,
  lists: Vec<Doc>,
  /// For each node, whether it breaks the group around it.
  broken: Vec<bool>,
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
  /// How many nodes and list entries the shared docs take.
  const SHARED: (usize, usize) = (8, 4);

  pub fn new(bump: &'a Bump) -> Self {
    Self::reuse(bump, &mut Scratch::default())
  }

  /// An arena that takes the buffers of `scratch`, which `into_scratch` gives back.
  pub fn reuse(bump: &'a Bump, scratch: &mut Scratch) -> Self {
    let mut nodes = recycle(std::mem::take(&mut scratch.nodes));
    nodes.extend([
      Node::Text("", 0),
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
    Self { bump, nodes, lists, broken, groups: 0, trims: false }
  }

  /// The buffers of this arena, for the next one.
  pub fn into_scratch(self) -> Scratch {
    Scratch { nodes: recycle(self.nodes), lists: self.lists, broken: self.broken }
  }

  /// Makes room for the docs of a text about `size` bytes long.
  pub fn reserve(&mut self, size: usize) {
    self.nodes.reserve(size / 2);
    self.broken.reserve(size / 2);
    self.lists.reserve(size / 2);
  }

  /// An arena with room for the docs of a text about `size` bytes long.
  pub fn with_capacity(bump: &'a Bump, size: usize) -> Self {
    let mut docs = Self::new(bump);
    docs.reserve(size);
    docs
  }

  pub fn clear(&mut self) {
    self.nodes.truncate(Self::SHARED.0);
    self.broken.truncate(Self::SHARED.0);
    self.lists.truncate(Self::SHARED.1);
    self.groups = 0;
  }

  fn push(&mut self, mut node: Node<'a>) -> Doc {
    // `propagateBreaks`, done as the docs are made: a `breakParent` breaks the group around it, a broken group breaks
    // the group around it, and a conditional group stops both.
    let breaks = match &mut node {
      Node::BreakParent => true,
      Node::Concat(list) | Node::Fill(list) => {
        self.parts(*list).iter().any(|&part| self.breaks(part))
      }
      Node::Group { broken, states: Some(_), .. } => *broken,
      Node::Group { contents, broken, .. } => {
        *broken |= self.breaks(*contents);
        *broken
      }
      Node::IfBreak { broken, flat, .. } => self.breaks(*broken) || self.breaks(*flat),
      Node::Indent(contents)
      | Node::Align(_, contents)
      | Node::IndentIfBreak { contents, .. }
      | Node::Label(_, contents)
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

  pub(crate) fn node(&self, doc: Doc) -> Node<'a> {
    self.nodes[doc.0 as usize]
  }

  pub(crate) fn parts(&self, list: List) -> &[Doc] {
    &self.lists[list.start as usize..(list.start + list.len) as usize]
  }

  pub fn group_id(&mut self) -> GroupId {
    self.groups += 1;
    GroupId(self.groups - 1)
  }

  pub fn text(&mut self, text: &'a str) -> Doc {
    if text.is_empty() {
      return Self::EMPTY;
    }
    self.push(Node::Text(text, width(text) as u32))
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
    self.push(Node::Align(align, contents))
  }

  pub fn group(&mut self, contents: Doc) -> Doc {
    self.push(Node::Group { contents, id: None, broken: false, states: None })
  }

  pub fn group_with(&mut self, contents: Doc, id: Option<GroupId>, broken: bool) -> Doc {
    self.push(Node::Group { contents, id, broken, states: None })
  }

  pub fn conditional(&mut self, states: &[Doc]) -> Doc {
    let list = self.list(states);
    self.push(Node::Group { contents: states[0], id: None, broken: false, states: Some(list) })
  }

  pub fn if_break(&mut self, broken: Doc, flat: Doc, id: Option<GroupId>) -> Doc {
    self.push(Node::IfBreak { broken, flat, id })
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

  pub fn label(&mut self, label: &'static str, contents: Doc) -> Doc {
    self.push(Node::Label(label, contents))
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

  /// `replaceEndOfLine`: each line break of `text` becomes `separator`.
  pub fn lines(&mut self, text: &'a str, separator: Doc) -> Doc {
    if !text.contains('\n') {
      return self.text(text);
    }
    let parts: Vec<Doc> = text.split('\n').map(|line| self.text(line)).collect();
    self.join(separator, &parts)
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
  fn find(&self, doc: Doc, found: &mut impl FnMut(Node<'a>) -> bool) -> bool {
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
        | Node::Label(_, contents)
        | Node::LineSuffix(contents) => stack.push(contents),
        _ => {}
      }
    }
    false
  }
}
