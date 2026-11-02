//! The tree of a JSON text: the nodes of Babel's AST that JSON can have, in one flat list.

/// A byte range of the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
  pub start: u32,
  pub end: u32,
}

impl Span {
  pub fn of(self, text: &str) -> &str {
    &text[self.start as usize..self.end as usize]
  }
}

/// A node of a [`Tree`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub(crate) u32);

/// A range of [`Tree::items`]: the properties of an object, or the elements of an array.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Items {
  pub(crate) start: u32,
  pub(crate) end: u32,
}

impl Items {
  pub fn len(self) -> usize {
    (self.end - self.start) as usize
  }

  pub fn is_empty(self) -> bool {
    self.start == self.end
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  /// Its items are properties.
  Object(Items),
  Property {
    key: NodeId,
    value: NodeId,
  },
  /// Its items are values, and `None` for a hole: `[1, , 2]`.
  Array(Items),
  /// With its quotes.
  String,
  Number,
  /// `Infinity`, `NaN` or `undefined` as a value, and any name as a key.
  Identifier,
  /// A template literal without expressions, with its backticks.
  Template,
  Null,
  True,
  False,
  /// `+` or `-` before a number, `Infinity` or `NaN`.
  Sign {
    minus: bool,
    argument: NodeId,
  },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
  pub kind: Kind,
  pub span: Span,
}

/// A `//` or `/* */` comment. The span covers its delimiters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Comment {
  pub span: Span,
  pub block: bool,
}

impl Comment {
  /// The text between its delimiters.
  pub fn value(self, text: &str) -> &str {
    let text = self.span.of(text);
    match self.block {
      true => &text[2..text.len() - 2],
      false => &text[2..],
    }
  }
}

/// A parsed JSON text.
#[derive(Clone, Debug, Default)]
pub struct Tree {
  pub(crate) nodes: Vec<Node>,
  pub(crate) items: Vec<Option<NodeId>>,
  pub(crate) comments: Vec<Comment>,
  pub(crate) root: Option<NodeId>,
}

impl Tree {
  /// The value of the text, or `None` if it only has comments.
  pub fn root(&self) -> Option<NodeId> {
    self.root
  }

  pub fn node(&self, id: NodeId) -> Node {
    self.nodes[id.0 as usize]
  }

  pub fn items(&self, items: Items) -> &[Option<NodeId>] {
    &self.items[items.start as usize..items.end as usize]
  }

  /// In the order of the text.
  pub fn comments(&self) -> &[Comment] {
    &self.comments
  }

  /// A tree with room for a text of `bytes`, which has a node in about each 8.
  pub(crate) fn sized(bytes: usize) -> Tree {
    let nodes = bytes / 8;
    Tree {
      nodes: Vec::with_capacity(nodes),
      items: Vec::with_capacity(nodes / 2),
      ..Tree::default()
    }
  }

  pub(crate) fn clear(&mut self) {
    self.nodes.clear();
    self.items.clear();
    self.comments.clear();
    self.root = None;
  }

  pub(crate) fn push(&mut self, kind: Kind, span: Span) -> NodeId {
    self.nodes.push(Node { kind, span });
    NodeId(self.nodes.len() as u32 - 1)
  }
}
