//! Binding patterns: bun's `src/lint/ast/pat.rs` (MIT), over oxc's tree. They are what `var`, `let`, `const`, a
//! parameter or a `catch` clause declare. A destructuring assignment's target is an `Array` or `Object` expression.
//!
//! Where bun's tree differs from oxc's: an element of an array pattern has no node of its own. A [`PatElem`] is the
//! element's node: `a`, `a = 1` or `...a`, so a plain `a` is both a [`Pat`] and a [`PatElem`], and its parent is the
//! array pattern. A hole has no node: its [`PatElem`] has the id [`NONE`](super::NONE), and no [`PatElem::pat`]. A
//! [`PatProp`] is a property or the `...rest` of an object pattern, and the `a = 1` of a property or an element isn't a
//! [`Pat`].

use super::{Expr, Key, List, Name, Node, Pat, PatElem, PatProp, expression_id};
use oxc_ast::AstKind;
use oxc_ast::ast::{BindingPattern, BindingProperty};

#[derive(Copy, Clone, Debug)]
pub(crate) enum PatKind<'a> {
  /// A hole in an array pattern.
  Missing,
  Ident(Name<'a>),
  Object(List<'a, PatProp<'a>>),
  Array(List<'a, PatElem<'a>>),
}

/// The kind of a pattern, without its payload.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum PatTag {
  Missing,
  Ident,
  Object,
  Array,
}

impl<'a> Pat<'a> {
  pub(crate) fn kind(self) -> PatKind<'a> {
    let file = self.file;
    let first = || file.children(self.id).first().copied();
    match file.kind(self.id) {
      Some(AstKind::BindingIdentifier(it)) => PatKind::Ident(Name(it.name.as_str())),
      Some(AstKind::TSThisParameter(_)) => PatKind::Ident(Name("this")),
      Some(AstKind::TSIndexSignatureName(it)) => PatKind::Ident(Name(it.name.as_str())),
      Some(AstKind::ObjectPattern(it)) => {
        let len = it.properties.len() + usize::from(it.rest.is_some());
        PatKind::Object(List::run(file, first(), len))
      }
      Some(AstKind::ArrayPattern(it)) => PatKind::Array(match file.holey(self.id) {
        Some(ids) => List::ids(file, ids),
        None => List::run(file, first(), it.elements.len() + usize::from(it.rest.is_some())),
      }),
      _ => PatKind::Missing,
    }
  }

  pub(crate) fn tag(self) -> PatTag {
    match self.kind() {
      PatKind::Missing => PatTag::Missing,
      PatKind::Ident(_) => PatTag::Ident,
      PatKind::Object(_) => PatTag::Object,
      PatKind::Array(_) => PatTag::Array,
    }
  }

  pub(crate) fn as_ident(self) -> Option<Name<'a>> {
    match self.kind() {
      PatKind::Ident(name) => Some(name),
      _ => None,
    }
  }

  /// A `VarDecl`, a `Param`, a `PatProp`, a `PatElem`, or the array pattern of a plain element.
  pub(crate) fn parent(self) -> Node<'a> {
    Node::Pat(self).parent()
  }
}

impl<'a> PatProp<'a> {
  fn property(self) -> Option<&'a BindingProperty<'a>> {
    match self.file.kind(self.id)? {
      AstKind::BindingProperty(it) => Some(it),
      _ => None,
    }
  }

  /// `None` for `...rest`. For the shorthand `{ a }` it's the `a`.
  pub(crate) fn key(self) -> Option<Key<'a>> {
    self.property().map(|it| Key::of(self.file, &it.key, it.computed))
  }

  /// `{ a }`, `{ a = 1 }`: the name is both the key and the binding.
  pub(crate) fn is_shorthand(self) -> bool {
    self.property().is_some_and(|it| it.shorthand)
  }

  pub(crate) fn value(self) -> Pat<'a> {
    let id = match self.file.kind(self.id) {
      Some(AstKind::BindingProperty(it)) => value_id(&it.value),
      Some(AstKind::BindingRestElement(it)) => pattern_id(&it.argument),
      _ => super::NONE,
    };
    Pat::new(self.file, id)
  }

  pub(crate) fn default(self) -> Option<Expr<'a>> {
    match &self.property()?.value {
      BindingPattern::AssignmentPattern(it) => Some(Expr::new(self.file, expression_id(&it.right))),
      _ => None,
    }
  }

  pub(crate) fn is_rest(self) -> bool {
    matches!(self.file.kind(self.id), Some(AstKind::BindingRestElement(_)))
  }

  /// The object pattern.
  pub(crate) fn parent(self) -> Node<'a> {
    Node::PatProp(self).parent()
  }
}

impl<'a> PatElem<'a> {
  /// The `a` of `a`, `a = 1` and `...a`. `None` for a hole.
  pub(crate) fn pat(self) -> Option<Pat<'a>> {
    let id = match self.file.kind(self.id)? {
      AstKind::AssignmentPattern(it) => pattern_id(&it.left),
      AstKind::BindingRestElement(it) => pattern_id(&it.argument),
      _ => self.id,
    };
    Some(Pat::new(self.file, id))
  }

  pub(crate) fn default(self) -> Option<Expr<'a>> {
    match self.file.kind(self.id)? {
      AstKind::AssignmentPattern(it) => Some(Expr::new(self.file, expression_id(&it.right))),
      _ => None,
    }
  }

  pub(crate) fn is_rest(self) -> bool {
    matches!(self.file.kind(self.id), Some(AstKind::BindingRestElement(_)))
  }
}

fn pattern_id(pattern: &BindingPattern) -> u32 {
  pattern.node_id().index() as u32
}

/// The id of a property's value: the `b` of `b = 1`.
fn value_id(pattern: &BindingPattern) -> u32 {
  match pattern {
    BindingPattern::AssignmentPattern(it) => pattern_id(&it.left),
    _ => pattern_id(pattern),
  }
}
