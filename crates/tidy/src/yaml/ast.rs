//! The tree that Prettier prints: `yaml-unist-parser`'s, in one flat list. Each node has a position, its children,
//! and the comments that are attached to it.

use std::cell::Cell;

/// A point in the text: its line and column, from 1, and its offset in bytes. The column counts UTF-16 code units, as
/// `yaml`'s `LineCounter` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Point {
  pub line: u32,
  pub column: u32,
  pub offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Position {
  pub start: Point,
  pub end: Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  Root,
  Document,
  DocumentHead,
  DocumentBody,
  Directive,
  Comment,
  Tag,
  Anchor,
  Alias,
  BlockLiteral,
  BlockFolded,
  Plain,
  QuoteSingle,
  QuoteDouble,
  Mapping,
  MappingItem,
  MappingKey,
  MappingValue,
  Sequence,
  SequenceItem,
  FlowMapping,
  FlowMappingItem,
  FlowSequence,
  FlowSequenceItem,
}

impl Kind {
  /// The node has `leadingComments`.
  pub(crate) fn leads(self) -> bool {
    use Kind::*;
    matches!(
      self,
      Directive
        | Alias
        | BlockLiteral
        | BlockFolded
        | Plain
        | QuoteSingle
        | QuoteDouble
        | Mapping
        | MappingItem
        | MappingValue
        | Sequence
        | SequenceItem
        | FlowMapping
        | FlowMappingItem
        | FlowSequence
    )
  }

  /// The node has a `trailingComment`.
  pub(crate) fn trails(self) -> bool {
    use Kind::*;
    matches!(
      self,
      Document
        | DocumentHead
        | Directive
        | Alias
        | Plain
        | QuoteSingle
        | QuoteDouble
        | MappingKey
        | MappingValue
        | SequenceItem
        | FlowMapping
        | FlowSequence
    )
  }
}

/// How a block scalar's header chomps its trailing line breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Chomping {
  #[default]
  Clip,
  Keep,
  Strip,
}

pub type Id = u32;

/// What the tree stores of a node. Its lists and text are in the [`Root`], which [`Node`] reads.
#[derive(Clone, Debug)]
pub struct Entry {
  pub kind: Kind,
  pub position: Position,
  pub parent: Option<Id>,
  pub trailing_comment: Option<Id>,
  pub tag: Option<Id>,
  pub anchor: Option<Id>,
  pub chomping: Chomping,
  pub indent: Option<u32>,
  pub directives_end_marker: bool,
  pub document_end_marker: bool,
  /// A range of [`Root::lists`].
  children: (u32, u32),
  /// A range of [`Root::strings`].
  value: (u32, u32),
  /// An index of [`Root::extras`], if it has one.
  extra: Option<u32>,
}

impl Entry {
  pub(crate) fn new(kind: Kind, position: Position) -> Entry {
    Entry {
      kind,
      position,
      parent: None,
      trailing_comment: None,
      tag: None,
      anchor: None,
      chomping: Chomping::Clip,
      indent: None,
      directives_end_marker: false,
      document_end_marker: false,
      children: (0, 0),
      value: (0, 0),
      extra: None,
    }
  }
}

/// What few nodes hold: comments before, inside and at the end of them, and a directive's parameters.
#[derive(Clone, Debug, Default)]
pub(crate) struct Extra {
  pub(crate) leading: Vec<Id>,
  pub(crate) end: Vec<Id>,
  pub(crate) middle: Vec<Id>,
  pub(crate) indicator: Option<Id>,
  pub(crate) parameters: Vec<String>,
}

/// A node of a [`Root`], with what it holds.
#[derive(Clone, Copy, Debug)]
pub struct Node<'r> {
  root: &'r Root,
  entry: &'r Entry,
}

impl std::ops::Deref for Node<'_> {
  type Target = Entry;

  fn deref(&self) -> &Entry {
    self.entry
  }
}

impl<'r> Node<'r> {
  pub fn children(&self) -> &'r [Id] {
    let (start, end) = self.entry.children;
    &self.root.lists[start as usize..end as usize]
  }

  /// The value of a comment, tag, anchor, alias or scalar, and the name of a directive.
  pub fn value(&self) -> &'r str {
    let (start, end) = self.entry.value;
    &self.root.strings[start as usize..end as usize]
  }

  pub fn leading_comments(&self) -> &'r [Id] {
    self.extra().map_or(&[], |extra| &extra.leading)
  }

  pub fn end_comments(&self) -> &'r [Id] {
    self.extra().map_or(&[], |extra| &extra.end)
  }

  pub fn middle_comments(&self) -> &'r [Id] {
    self.extra().map_or(&[], |extra| &extra.middle)
  }

  pub fn indicator_comment(&self) -> Option<Id> {
    self.extra().and_then(|extra| extra.indicator)
  }

  pub fn parameters(&self) -> &'r [String] {
    self.extra().map_or(&[], |extra| &extra.parameters)
  }

  fn extra(&self) -> Option<&'r Extra> {
    self.entry.extra.map(|index| &self.root.extras[index as usize])
  }
}

/// A parsed YAML stream: its documents, as children of the root, and its comments.
#[derive(Clone, Debug, Default)]
pub struct Root {
  entries: Vec<Entry>,
  /// The children of every node.
  lists: Vec<Id>,
  /// The values of every node.
  strings: String,
  extras: Vec<Extra>,
  /// All comments, in the order of the text.
  pub comments: Vec<Id>,
  pub root: Id,
}

impl Root {
  pub fn node(&self, id: Id) -> Node<'_> {
    Node { root: self, entry: &self.entries[id as usize] }
  }

  pub fn len(&self) -> usize {
    self.entries.len()
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  pub(crate) fn entry_mut(&mut self, id: Id) -> &mut Entry {
    &mut self.entries[id as usize]
  }

  /// Adds a node, and makes it the parent of the nodes it holds: `defineParents`, as the tree is built from its leaves.
  pub(crate) fn add(&mut self, mut entry: Entry, children: &[Id]) -> Id {
    let id = self.entries.len() as Id;
    let start = self.lists.len() as u32;
    self.lists.extend_from_slice(children);
    entry.children = (start, self.lists.len() as u32);
    for child in
      children.iter().chain([entry.tag, entry.anchor, entry.trailing_comment].iter().flatten())
    {
      self.entries[*child as usize].parent = Some(id);
    }
    self.entries.push(entry);
    id
  }

  /// Gives a node its value.
  pub(crate) fn set_value(&mut self, id: Id, value: &str) {
    let start = self.strings.len() as u32;
    self.strings.push_str(value);
    self.entries[id as usize].value = (start, self.strings.len() as u32);
  }

  /// Gives a node what few nodes hold, and makes it the parent of its comments.
  pub(crate) fn set_extra(&mut self, id: Id, extra: Extra) {
    let comments = [&extra.leading, &extra.end, &extra.middle];
    for comment in comments.into_iter().flatten().chain(&extra.indicator) {
      self.entries[*comment as usize].parent = Some(id);
    }
    match self.entries[id as usize].extra {
      Some(index) => self.extras[index as usize] = extra,
      None => {
        self.entries[id as usize].extra = Some(self.extras.len() as u32);
        self.extras.push(extra);
      }
    }
  }

  /// What few nodes hold, to add to.
  pub(crate) fn extra_mut(&mut self, id: Id) -> &mut Extra {
    let index = match self.entries[id as usize].extra {
      Some(index) => index,
      None => {
        self.extras.push(Extra::default());
        let index = self.extras.len() as u32 - 1;
        self.entries[id as usize].extra = Some(index);
        index
      }
    };
    &mut self.extras[index as usize]
  }

  /// Makes room for the nodes of a text of `bytes`.
  pub(crate) fn reserve(&mut self, bytes: usize) {
    self.entries.reserve(bytes / 8);
    self.lists.reserve(bytes / 8);
    self.strings.reserve(bytes / 2);
  }

  pub(crate) fn clear(&mut self) {
    self.entries.clear();
    self.lists.clear();
    self.strings.clear();
    self.extras.clear();
    self.comments.clear();
    self.root = 0;
  }
}

/// `LineCounter`: the line and column of an offset.
pub(crate) struct Lines<'l> {
  text: &'l str,
  starts: Vec<u32>,
  /// Whether each line is ASCII, so its columns are its byte offsets.
  ascii: Vec<bool>,
  /// The line of the last point, as the next is most often on it or the one after.
  last: Cell<usize>,
}

impl<'l> Lines<'l> {
  pub(crate) fn new(text: &'l str) -> Lines<'l> {
    let mut starts = vec![0];
    starts.extend(memchr::memchr_iter(b'\n', text.as_bytes()).map(|at| at as u32 + 1));
    let bytes = text.as_bytes();
    let ascii = (0..starts.len())
      .map(|line| {
        let end = starts.get(line + 1).map_or(bytes.len(), |end| *end as usize);
        bytes[starts[line] as usize..end].is_ascii()
      })
      .collect();
    Lines { text, starts, ascii, last: Cell::new(1) }
  }

  pub(crate) fn point(&self, offset: u32) -> Point {
    let has = |line: usize| {
      self.starts[line - 1] <= offset && self.starts.get(line).is_none_or(|next| offset < *next)
    };
    let last = self.last.get();
    let line = match () {
      _ if has(last) => last,
      _ if last < self.starts.len() && has(last + 1) => last + 1,
      _ => self.starts.partition_point(|start| *start <= offset).max(1),
    };
    self.last.set(line);
    let start = self.starts[line - 1] as usize;
    let end = (offset as usize).min(self.text.len());
    let column = match self.ascii[line - 1] {
      true => end.saturating_sub(start),
      false => self.text.get(start..end).unwrap_or_default().encode_utf16().count(),
    };
    Point { line: line as u32, column: column as u32 + 1, offset }
  }

  pub(crate) fn position(&self, start: u32, end: u32) -> Position {
    Position { start: self.point(start), end: self.point(end) }
  }
}
