//! Names: bun's `src/lint/ast/name.rs` (MIT), over oxc's strings.

use super::{Span, Spanned};

/// The text of an identifier, a property name or a string, with its escapes resolved: `a` is `a`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Name<'a>(pub(crate) &'a str);

impl<'a> Name<'a> {
  #[inline]
  pub(crate) fn bytes(self) -> &'a [u8] {
    self.0.as_bytes()
  }

  #[inline]
  pub(crate) fn is(self, text: &str) -> bool {
    self.0 == text
  }
}

impl PartialEq<str> for Name<'_> {
  fn eq(&self, other: &str) -> bool {
    self.0 == other
  }
}

impl PartialEq<&str> for Name<'_> {
  fn eq(&self, other: &&str) -> bool {
    self.0 == *other
  }
}

impl PartialEq<[u8]> for Name<'_> {
  fn eq(&self, other: &[u8]) -> bool {
    self.0.as_bytes() == other
  }
}

impl PartialEq<&[u8]> for Name<'_> {
  fn eq(&self, other: &&[u8]) -> bool {
    self.0.as_bytes() == *other
  }
}

impl std::fmt::Display for Name<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.0)
  }
}

/// A name where it's written: a property name, a label, the name of a declaration.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Ident<'a> {
  pub(crate) name: Name<'a>,
  pub(crate) span: Span,
  /// The name is a string literal: `"a"` in `{ "a": 1 }`.
  pub(crate) string: bool,
}

impl<'a> Ident<'a> {
  #[inline]
  pub(crate) fn name(self) -> Name<'a> {
    self.name
  }

  #[inline]
  pub(crate) fn bytes(self) -> &'a [u8] {
    self.name.bytes()
  }

  #[inline]
  pub(crate) fn start(self) -> u32 {
    self.span.start
  }

  #[inline]
  pub(crate) fn is_string(self) -> bool {
    self.string
  }

  #[inline]
  pub(crate) fn span(self) -> Span {
    self.span
  }
}

impl Spanned for Ident<'_> {
  fn span(&self) -> Span {
    self.span
  }
}
