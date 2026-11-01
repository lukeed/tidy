//! The syntax tree that taplo's parser builds with rowan, in one flat list: nodes, and the tokens in them, whitespace,
//! line breaks and comments included.

use super::lexer::Token;

/// What an element of a [`Tree`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  Token(Token),
  Root,
  Key,
  Value,
  TableHeader,
  TableArrayHeader,
  Entry,
  Array,
  InlineTable,
}

/// An element of the tree: a node, or a token and its text.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Element {
  pub(crate) kind: Kind,
  pub(crate) start: u32,
  pub(crate) end: u32,
  pub(crate) parent: u32,
  /// Its place among the children of its parent.
  pub(crate) index: u32,
  /// For a node, the range of [`Tree::children`] with its children.
  pub(crate) children: (u32, u32),
}

/// A parsed TOML document.
#[derive(Clone, Debug, Default)]
pub struct Tree {
  pub(crate) elements: Vec<Element>,
  pub(crate) children: Vec<u32>,
}

/// An element of a [`Tree`].
pub type Id = u32;

/// No parent.
pub(crate) const NONE: Id = u32::MAX;

impl Tree {
  pub(crate) fn clear(&mut self) {
    self.elements.clear();
    self.children.clear();
  }

  /// The document: a [`Kind::Root`].
  pub fn root(&self) -> Id {
    self.elements.len() as Id - 1
  }

  pub fn kind(&self, id: Id) -> Kind {
    self.elements[id as usize].kind
  }

  pub(crate) fn is(&self, id: Id, token: Token) -> bool {
    self.kind(id) == Kind::Token(token)
  }

  /// The text of the element, in the `text` it was parsed from.
  pub fn text<'t>(&self, id: Id, text: &'t str) -> &'t str {
    let element = self.elements[id as usize];
    &text[element.start as usize..element.end as usize]
  }

  /// The nodes and tokens in a node, in order. A token has none.
  pub fn children(&self, id: Id) -> &[Id] {
    let (start, end) = self.elements[id as usize].children;
    &self.children[start as usize..end as usize]
  }

  /// `children`: the nodes among them.
  pub(crate) fn nodes(&self, id: Id) -> impl Iterator<Item = Id> + '_ {
    self.children(id).iter().copied().filter(|child| !matches!(self.kind(*child), Kind::Token(_)))
  }

  /// `descendants_with_tokens`, without itself, which is a node.
  pub(crate) fn any_descendant(&self, id: Id, accept: &impl Fn(Kind) -> bool) -> bool {
    self.children(id).iter().any(|child| {
      accept(self.kind(*child))
        || (!matches!(self.kind(*child), Kind::Token(_)) && self.any_descendant(*child, accept))
    })
  }

  /// `prev_sibling_or_token` and `next_sibling_or_token`.
  pub(crate) fn sibling(&self, id: Id, offset: isize) -> Option<Id> {
    let element = self.elements[id as usize];
    if element.parent == NONE {
      return None;
    }
    let siblings = self.children(element.parent);
    let index = element.index as isize + offset;
    usize::try_from(index).ok().and_then(|index| siblings.get(index).copied())
  }
}
