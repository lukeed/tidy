//! Lists of nodes: bun's `src/lint/ast/list.rs` (MIT). A list is a run of siblings in the file's table of children,
//! since the elements of one of oxc's list fields are consecutive children of the node that has the field.

use super::{File, Spanned};
use oxc_ast::ast::{Argument, ChainElement, Expression};
use std::marker::PhantomData;

/// A handle that a list can hold.
pub(crate) trait Handle<'a>: Copy {
  /// The handle for the child with `id`, which is the inner node for a wrapper that bun's tree doesn't have.
  fn from_raw(file: &'a File<'a>, id: u32) -> Self;
}

/// A node's list of children: arguments, statements, members.
#[derive(Copy, Clone)]
pub(crate) struct List<'a, T> {
  file: &'a File<'a>,
  start: u32,
  len: u32,
  /// Elements that bun's tree doesn't have, which every operation skips: JSX text of only white space and line breaks.
  skip: Option<fn(&File, u32) -> bool>,
  of: PhantomData<T>,
}

impl<'a, T: Handle<'a>> List<'a, T> {
  /// The list whose first element is the child with `first`, if any, and which has `len` elements.
  pub(crate) fn run(file: &'a File<'a>, first: Option<u32>, len: usize) -> Self {
    match first {
      Some(first) if len > 0 => {
        List { file, start: file.position(first), len: len as u32, skip: None, of: PhantomData }
      }
      _ => List::empty(file),
    }
  }

  pub(crate) fn empty(file: &'a File<'a>) -> Self {
    List { file, start: 0, len: 0, skip: None, of: PhantomData }
  }

  /// The list without the elements for which `skip` is true.
  pub(crate) fn without(self, skip: fn(&File, u32) -> bool) -> Self {
    List { skip: Some(skip), ..self }
  }

  fn id_at(self, i: u32) -> u32 {
    self.file.children_at(self.start + i, 1).first().copied().unwrap_or(super::NONE)
  }

  #[inline]
  fn at(self, i: u32) -> T {
    T::from_raw(self.file, self.id_at(i))
  }

  pub(crate) fn iter(self) -> Iter<'a, T> {
    Iter { list: self, front: 0, back: self.len }
  }

  pub(crate) fn len(self) -> usize {
    match self.skip {
      Some(_) => self.iter().count(),
      None => self.len as usize,
    }
  }

  pub(crate) fn is_empty(self) -> bool {
    self.len() == 0
  }

  pub(crate) fn get(self, i: usize) -> Option<T> {
    match self.skip {
      Some(_) => self.iter().nth(i),
      None => (i < self.len as usize).then(|| self.at(i as u32)),
    }
  }

  pub(crate) fn first(self) -> Option<T> {
    self.get(0)
  }

  pub(crate) fn last(self) -> Option<T> {
    self.iter().next_back()
  }
}

/// For a list whose elements are in source order and don't overlap. Each lookup is O(log n).
impl<'a, T: Handle<'a> + Spanned> List<'a, T> {
  /// The number of elements that start at or before `offset`.
  fn count_until(self, offset: u32) -> u32 {
    let (mut from, mut to) = (0, self.len);
    while from < to {
      let middle = from + (to - from) / 2;
      match self.at(middle).span().start <= offset {
        true => from = middle + 1,
        false => to = middle,
      }
    }
    from
  }

  /// The element that contains `offset`.
  pub(crate) fn around(self, offset: u32) -> Option<T> {
    let it = self.at(self.count_until(offset).checked_sub(1)?);
    (offset < it.span().end).then_some(it)
  }

  /// [`List::around`], plus the element before it.
  pub(crate) fn around_with_previous(self, offset: u32) -> Option<(T, Option<T>)> {
    let at = self.count_until(offset).checked_sub(1)?;
    let it = self.at(at);
    if offset >= it.span().end {
      return None;
    }
    Some((it, at.checked_sub(1).map(|before| self.at(before))))
  }

  /// The index of the element that starts at `start`.
  pub(crate) fn index_of_start(self, start: u32) -> Option<usize> {
    let at = self.count_until(start).checked_sub(1)?;
    (self.at(at).span().start == start).then_some(at as usize)
  }

  /// The element before the one that starts at `start`.
  pub(crate) fn before(self, start: u32) -> Option<T> {
    let at = self.count_until(start).checked_sub(1)?;
    at.checked_sub(1).map(|before| self.at(before))
  }

  /// The element after the one that starts at `start`.
  pub(crate) fn after(self, start: u32) -> Option<T> {
    let at = self.count_until(start);
    (at < self.len).then(|| self.at(at))
  }
}

impl<'a, T: Handle<'a> + std::fmt::Debug> std::fmt::Debug for List<'a, T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_list().entries(self.iter()).finish()
  }
}

impl<'a, T: Handle<'a>> IntoIterator for List<'a, T> {
  type Item = T;
  type IntoIter = Iter<'a, T>;

  fn into_iter(self) -> Iter<'a, T> {
    self.iter()
  }
}

#[derive(Copy, Clone)]
pub(crate) struct Iter<'a, T> {
  list: List<'a, T>,
  front: u32,
  back: u32,
}

impl<'a, T: Handle<'a>> Iterator for Iter<'a, T> {
  type Item = T;

  fn next(&mut self) -> Option<T> {
    while self.front < self.back {
      self.front += 1;
      if !self.list.skips(self.front - 1) {
        return Some(self.list.at(self.front - 1));
      }
    }
    None
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    let rest = (self.back - self.front) as usize;
    (if self.list.skip.is_some() { 0 } else { rest }, Some(rest))
  }
}

impl<'a, T: Handle<'a>> DoubleEndedIterator for Iter<'a, T> {
  fn next_back(&mut self) -> Option<T> {
    while self.front < self.back {
      self.back -= 1;
      if !self.list.skips(self.back) {
        return Some(self.list.at(self.back));
      }
    }
    None
  }
}

impl<'a, T: Handle<'a>> List<'a, T> {
  fn skips(self, i: u32) -> bool {
    self.skip.is_some_and(|skip| skip(self.file, self.id_at(i)))
  }
}

/// The id in the tables of an expression: parentheses and a chain around it are skipped.
pub(crate) fn expression_id(mut expression: &Expression) -> u32 {
  loop {
    match expression {
      Expression::ParenthesizedExpression(parens) => expression = &parens.expression,
      Expression::ChainExpression(chain) => return chain_id(&chain.expression),
      _ => return expression.node_id().index() as u32,
    }
  }
}

pub(crate) fn chain_id(element: &ChainElement) -> u32 {
  match element {
    ChainElement::TSNonNullExpression(it) => it.node_id.get().index() as u32,
    _ => element.node_id().index() as u32,
  }
}

pub(crate) fn argument_id(argument: &Argument) -> u32 {
  match argument.as_expression() {
    Some(expression) => expression_id(expression),
    None => argument.node_id().index() as u32,
  }
}
