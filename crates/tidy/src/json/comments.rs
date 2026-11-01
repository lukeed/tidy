//! Prettier's `attachComments` (`src/main/comments/attach.js`), with the handlers of `estree` that apply to JSON: each
//! comment becomes a leading, trailing or dangling comment of a node.

use super::ast::{Kind, NodeId, Tree};
use crate::strings;
use crate::text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
  Leading,
  Trailing,
  Dangling,
}

/// The node that holds dangling comments when there is no value: Prettier's `JsonRoot`.
pub(crate) const ROOT: NodeId = NodeId(u32::MAX);

/// A comment of a node, by its index in [`Tree::comments`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Attached {
  pub(crate) node: NodeId,
  pub(crate) comment: u32,
  pub(crate) place: Place,
}

/// The comments of each node, sorted by node and then in the order of the text.
#[derive(Default)]
pub(crate) struct Comments {
  pub(crate) attached: Vec<Attached>,
  decorated: Vec<Decorated>,
  ties: Vec<u32>,
  children: Vec<NodeId>,
}

#[derive(Clone, Copy)]
struct Decorated {
  enclosing: Option<NodeId>,
  preceding: Option<NodeId>,
  following: Option<NodeId>,
}

impl Comments {
  /// The comments of `node`.
  pub(crate) fn of(&self, node: NodeId) -> &[Attached] {
    if self.attached.is_empty() {
      return &[];
    }
    let start = self.attached.partition_point(|it| it.node < node);
    let end = start + self.attached[start..].partition_point(|it| it.node == node);
    &self.attached[start..end]
  }

  pub(crate) fn attach(&mut self, tree: &Tree, text: &[u8]) {
    self.attached.clear();
    self.decorated.clear();
    self.ties.clear();
    for comment in &tree.comments {
      let decorated = self.decorate(tree, comment.span.start, comment.span.end);
      self.decorated.push(decorated);
    }
    for index in 0..tree.comments.len() {
      let Decorated { enclosing, preceding, following } = self.decorated[index];
      let comment = index as u32;
      if self.is_own_line(tree, text, index) {
        match (following, preceding, enclosing) {
          (Some(node), ..) => self.add(node, comment, Place::Leading),
          (None, Some(node), _) => self.add(node, comment, Place::Trailing),
          (None, None, node) => self.add(node.unwrap_or(ROOT), comment, Place::Dangling),
        }
      } else if self.is_end_of_line(tree, text, index) {
        let property =
          enclosing.filter(|node| matches!(tree.node(*node).kind, Kind::Property { .. }));
        match (property, preceding, following, enclosing) {
          (Some(node), ..) => self.add(node, comment, Place::Leading),
          (None, Some(node), ..) => self.add(node, comment, Place::Trailing),
          (None, None, Some(node), _) => self.add(node, comment, Place::Leading),
          (None, None, None, node) => self.add(node.unwrap_or(ROOT), comment, Place::Dangling),
        }
      } else {
        match (preceding, following, enclosing) {
          (Some(_), Some(node), _) => {
            let last = self.ties.last().map(|it| self.decorated[*it as usize].following);
            if last.is_some_and(|last| last != Some(node)) {
              self.break_ties(tree, text);
            }
            self.ties.push(comment);
          }
          (Some(node), None, _) => self.add(node, comment, Place::Trailing),
          (None, Some(node), _) => self.add(node, comment, Place::Leading),
          (None, None, node) => self.add(node.unwrap_or(ROOT), comment, Place::Dangling),
        }
      }
    }
    self.break_ties(tree, text);
    self.attached.sort_by_key(|it| (it.node, it.comment));
  }

  fn add(&mut self, node: NodeId, comment: u32, place: Place) {
    self.attached.push(Attached { node, comment, place });
  }

  /// `decorateComment`: the innermost node around the comment, and its children right before and after it.
  fn decorate(&mut self, tree: &Tree, start: u32, end: u32) -> Decorated {
    let mut decorated = Decorated { enclosing: None, preceding: None, following: None };
    let mut parent = None;
    loop {
      self.children.clear();
      match parent {
        None => self.children.extend(tree.root),
        Some(node) => children(tree, node, &mut self.children),
      }
      let (mut left, mut right) = (0, self.children.len());
      let mut inside = None;
      while left < right {
        let middle = (left + right) / 2;
        let child = self.children[middle];
        let span = tree.node(child).span;
        if span.start <= start && end <= span.end {
          inside = Some(child);
          break;
        }
        if span.end <= start {
          decorated.preceding = Some(child);
          left = middle + 1;
        } else {
          decorated.following = Some(child);
          right = middle;
        }
      }
      match inside {
        Some(child) => {
          decorated = Decorated { enclosing: Some(child), preceding: None, following: None };
          parent = Some(child);
        }
        None => return decorated,
      }
    }
  }

  /// `isOwnLineComment`: only whitespace, and comments after the same node, are before it on its line.
  fn is_own_line(&self, tree: &Tree, text: &[u8], index: usize) -> bool {
    let preceding = self.decorated[index].preceding;
    let mut start = tree.comments[index].span.start as usize;
    if preceding.is_some() {
      for before in (0..index).rev() {
        let comment = tree.comments[before].span;
        let between = &text[comment.end as usize..start];
        if self.decorated[before].preceding != preceding || !is_blank_without_break(between) {
          break;
        }
        start = comment.start as usize;
      }
    }
    text::has_newline_backwards(text, start)
  }

  /// `isEndOfLineComment`: only whitespace, and comments before the same node, are after it on its line.
  fn is_end_of_line(&self, tree: &Tree, text: &[u8], index: usize) -> bool {
    let following = self.decorated[index].following;
    let mut end = tree.comments[index].span.end as usize;
    if following.is_some() {
      for after in index + 1..tree.comments.len() {
        let comment = tree.comments[after].span;
        let between = &text[end..comment.start as usize];
        if self.decorated[after].following != following || !is_blank_without_break(between) {
          break;
        }
        end = comment.end as usize;
      }
    }
    text::has_newline(text, end)
  }

  /// `breakTies`: of the comments between the same two nodes, those that only whitespace separates from the next node
  /// lead it, and the others trail the node before.
  fn break_ties(&mut self, tree: &Tree, text: &[u8]) {
    let Some(&first) = self.ties.first() else { return };
    let Decorated { preceding, following, .. } = self.decorated[first as usize];
    let (Some(preceding), Some(following)) = (preceding, following) else { return };
    let mut gap_end = tree.node(following).span.start as usize;
    let mut leading = self.ties.len();
    while leading > 0 {
      let comment = tree.comments[self.ties[leading - 1] as usize].span;
      if !is_gap(&text[comment.end as usize..gap_end]) {
        break;
      }
      gap_end = comment.start as usize;
      leading -= 1;
    }
    for index in 0..self.ties.len() {
      let comment = self.ties[index];
      match index < leading {
        true => self.add(preceding, comment, Place::Trailing),
        false => self.add(following, comment, Place::Leading),
      }
    }
    self.ties.clear();
  }
}

/// `getSortedChildNodes`: the nodes a comment can attach to, right inside `node`.
fn children(tree: &Tree, node: NodeId, out: &mut Vec<NodeId>) {
  match tree.node(node).kind {
    Kind::Object(items) | Kind::Array(items) => out.extend(tree.items(items).iter().flatten()),
    Kind::Property { key, value } => out.extend([key, value]),
    Kind::Sign { argument, .. } => out.push(argument),
    _ => {}
  }
}

/// `!/[\S\n  ]/.test(text)`
fn is_blank_without_break(mut text: &[u8]) -> bool {
  while !text.is_empty() {
    let length = strings::js_whitespace_len(text);
    if length == 0 || text[0] == b'\n' || strings::js_line_break_len(text) == 3 {
      return false;
    }
    text = &text[length..];
  }
  true
}

/// `/^[\s(]*$/.test(gap)`
fn is_gap(mut text: &[u8]) -> bool {
  while !text.is_empty() {
    let length = match text[0] {
      b'(' => 1,
      _ => strings::js_whitespace_len(text),
    };
    if length == 0 {
      return false;
    }
    text = &text[length..];
  }
  true
}
