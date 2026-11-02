//! The tree that Prettier prints: `yaml-unist-parser`'s, in one flat list. Each node has a position, its children,
//! and the comments that are attached to it.

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

#[derive(Clone, Debug)]
pub struct Node {
  pub kind: Kind,
  pub position: Position,
  pub parent: Option<Id>,
  pub children: Vec<Id>,
  /// The value of a comment, tag, anchor, alias or scalar, and the name of a directive.
  pub value: String,
  pub parameters: Vec<String>,
  pub leading_comments: Vec<Id>,
  pub trailing_comment: Option<Id>,
  pub end_comments: Vec<Id>,
  pub middle_comments: Vec<Id>,
  pub indicator_comment: Option<Id>,
  pub tag: Option<Id>,
  pub anchor: Option<Id>,
  pub chomping: Chomping,
  pub indent: Option<u32>,
  pub directives_end_marker: bool,
  pub document_end_marker: bool,
}

impl Node {
  pub(crate) fn new(kind: Kind, position: Position) -> Node {
    Node {
      kind,
      position,
      parent: None,
      children: Vec::new(),
      value: String::new(),
      parameters: Vec::new(),
      leading_comments: Vec::new(),
      trailing_comment: None,
      end_comments: Vec::new(),
      middle_comments: Vec::new(),
      indicator_comment: None,
      tag: None,
      anchor: None,
      chomping: Chomping::Clip,
      indent: None,
      directives_end_marker: false,
      document_end_marker: false,
    }
  }
}

/// A parsed YAML stream: its documents, as children of the root, and its comments.
#[derive(Clone, Debug, Default)]
pub struct Root {
  pub nodes: Vec<Node>,
  /// All comments, in the order of the text.
  pub comments: Vec<Id>,
  pub root: Id,
}

impl Root {
  pub fn node(&self, id: Id) -> &Node {
    &self.nodes[id as usize]
  }

  pub(crate) fn node_mut(&mut self, id: Id) -> &mut Node {
    &mut self.nodes[id as usize]
  }

  /// Adds a node, and makes it the parent of the nodes it holds: `defineParents`, as the tree is built from its leaves.
  pub(crate) fn add(&mut self, node: Node) -> Id {
    let id = self.nodes.len() as Id;
    let owned = [&node.children, &node.middle_comments, &node.end_comments, &node.leading_comments];
    for child in owned.into_iter().flatten() {
      self.nodes[*child as usize].parent = Some(id);
    }
    for child in
      [node.tag, node.anchor, node.indicator_comment, node.trailing_comment].into_iter().flatten()
    {
      self.nodes[child as usize].parent = Some(id);
    }
    self.nodes.push(node);
    id
  }

  pub(crate) fn clear(&mut self) {
    self.nodes.clear();
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
    Lines { text, starts, ascii }
  }

  pub(crate) fn point(&self, offset: u32) -> Point {
    let line = self.starts.partition_point(|start| *start <= offset).max(1);
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
