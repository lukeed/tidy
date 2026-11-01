//! `yaml-unist-parser`'s `attachComments` and `updatePositions`: each comment that no transform placed goes to the node
//! it trails, ends or leads, by its line and column; then each node grows to cover its children and comments.

use super::ast::{Id, Kind, Point, Root};

/// What a line holds: `NodeTable`.
#[derive(Clone, Copy, Default)]
struct Line {
  leading: Option<Id>,
  trailing_attachable: Option<Id>,
  trailing: Option<Id>,
  comment: Option<Id>,
}

pub(crate) fn attach(root: &mut Root) {
  define_parents(root, root.root, None);
  let lines = node_table(root);
  let documents = root.node(root.root).children.clone();
  let mut rest = 0;
  let comments: Vec<Id> =
    root.comments.iter().copied().filter(|it| root.node(*it).parent.is_none()).collect();
  for comment in comments {
    let line = root.node(comment).position.start.line;
    while documents.len() - rest > 1 && line > root.node(documents[rest]).position.end.line {
      rest += 1;
    }
    attach_comment(root, &lines, comment, documents[rest]);
  }
}

/// `defineParents`
fn define_parents(root: &mut Root, id: Id, parent: Option<Id>) {
  let node = root.node(id);
  let mut owned: Vec<Id> = node.children.clone();
  owned.extend(node.anchor);
  owned.extend(node.tag);
  owned.extend(node.leading_comments.iter().copied());
  owned.extend(node.middle_comments.iter().copied());
  owned.extend(node.indicator_comment);
  owned.extend(node.trailing_comment);
  owned.extend(node.end_comments.iter().copied());
  for child in owned {
    define_parents(root, child, Some(id));
  }
  root.node_mut(id).parent = parent;
}

fn node_table(root: &Root) -> Vec<Line> {
  let end = root.node(root.root).position.end.line as usize;
  let mut lines = vec![Line::default(); end.max(1)];
  for &comment in &root.comments {
    let line = root.node(comment).position.start.line as usize;
    lines[line - 1].comment = Some(comment);
  }
  init(root, &mut lines, root.root);
  lines
}

/// `initNodeTable`
fn init(root: &Root, lines: &mut [Line], id: Id) {
  let node = root.node(id);
  let (start, end) = (node.position.start, node.position.end);
  if start.offset == end.offset {
    return;
  }
  if node.kind.leads() {
    let line = &mut lines[start.line as usize - 1];
    if line.leading.is_none_or(|it| start.column < root.node(it).position.start.column) {
      line.leading = Some(id);
    }
  }
  if node.kind.trails()
    && end.column > 1
    && !matches!(node.kind, Kind::Document | Kind::DocumentHead)
  {
    let line = &mut lines[end.line as usize - 1];
    if line.trailing_attachable.is_none_or(|it| end.column >= root.node(it).position.end.column) {
      line.trailing_attachable = Some(id);
    }
  }
  if !matches!(node.kind, Kind::Root | Kind::Document | Kind::DocumentHead | Kind::DocumentBody) {
    let mut numbers = vec![end.line];
    if start.line != end.line {
      numbers.push(start.line);
    }
    for number in numbers {
      let line = &mut lines[number as usize - 1];
      if line.trailing.is_none_or(|it| end.column >= root.node(it).position.end.column) {
        line.trailing = Some(id);
      }
    }
  }
  for &child in &node.children {
    init(root, lines, child);
  }
}

/// `attachComment`
fn attach_comment(root: &mut Root, lines: &[Line], comment: Id, document: Id) {
  let comment_line = root.node(comment).position.start.line;
  if let Some(node) = lines[comment_line as usize - 1].trailing_attachable {
    root.node_mut(comment).parent = Some(node);
    root.node_mut(node).trailing_comment = Some(comment);
    return;
  }
  let document_start = root.node(document).position.start.line;
  let mut line = comment_line;
  while line >= document_start && line >= 1 {
    let entry = lines[line as usize - 1];
    let mut current = match entry.trailing {
      Some(trailing) => trailing,
      None => match entry.comment {
        Some(other) if line != comment_line => match root.node(other).parent {
          Some(parent) => parent,
          None => {
            line -= 1;
            continue;
          }
        },
        _ => {
          line -= 1;
          continue;
        }
      },
    };
    if matches!(root.node(current).kind, Kind::Sequence | Kind::Mapping) {
      current = root.node(current).children[0];
    }
    if root.node(current).kind == Kind::MappingItem {
      let children = &root.node(current).children;
      let (key, value) = (children[0], children[1]);
      current = if is_explicit_mapping_key(root, key) { key } else { value };
    }
    loop {
      if should_own_end_comment(root, current, comment) {
        root.node_mut(comment).parent = Some(current);
        root.node_mut(current).end_comments.push(comment);
        return;
      }
      match root.node(current).parent {
        Some(parent) => current = parent,
        None => break,
      }
    }
    break;
  }
  let document_end = root.node(document).position.end.line;
  for line in comment_line + 1..=document_end {
    if let Some(node) = lines.get(line as usize - 1).and_then(|it| it.leading) {
      root.node_mut(comment).parent = Some(node);
      root.node_mut(node).leading_comments.push(comment);
      return;
    }
  }
  let body = root.node(document).children[1];
  root.node_mut(comment).parent = Some(body);
  root.node_mut(body).end_comments.push(comment);
}

/// `shouldOwnEndComment`
fn should_own_end_comment(root: &Root, id: Id, comment: Id) -> bool {
  let node = root.node(id);
  let comment = root.node(comment).position;
  if node.position.start.offset < comment.start.offset
    && node.position.end.offset > comment.end.offset
  {
    match node.kind {
      Kind::FlowMapping | Kind::FlowSequence => {
        return node
          .children
          .last()
          .is_none_or(|last| comment.start.line > root.node(*last).position.end.line);
      }
      _ => {}
    }
  }
  if comment.end.offset < node.position.end.offset {
    return false;
  }
  match node.kind {
    Kind::SequenceItem => comment.start.column > node.position.start.column,
    Kind::MappingKey | Kind::MappingValue => {
      let parent = root.node(node.parent.expect("a key or value has a parent"));
      let child = node.children.first().map(|it| root.node(*it).kind);
      comment.start.column > parent.position.start.column
        && child.is_none_or(|kind| !matches!(kind, Kind::BlockFolded | Kind::BlockLiteral))
        && (node.kind == Kind::MappingValue || is_explicit_mapping_key(root, id))
    }
    _ => false,
  }
}

/// `isExplicitMappingKey`: a key that starts before its content, at `?`.
fn is_explicit_mapping_key(root: &Root, key: Id) -> bool {
  let node = root.node(key);
  node
    .children
    .first()
    .is_none_or(|child| node.position.start.offset != root.node(*child).position.start.offset)
}

/// `updatePositions`
pub(crate) fn update_positions(root: &mut Root, id: Id) {
  let children = root.node(id).children.clone();
  for &child in &children {
    update_positions(root, child);
  }
  if root.node(id).kind == Kind::Document {
    let (head, body) = (children[0], children[1]);
    let (head_position, body_position) = (root.node(head).position, root.node(body).position);
    if head_position.start.offset == head_position.end.offset {
      let start = body_position.start;
      root.node_mut(head).position = super::ast::Position { start, end: start };
    } else if body_position.start.offset == body_position.end.offset {
      let end = head_position.end;
      root.node_mut(body).position = super::ast::Position { start: end, end };
    }
  }
  let mut start = root.node(id).position.start;
  let mut end = root.node(id).position.end;
  let start_at = |point: Point, start: &mut Point| {
    if point.offset < start.offset {
      *start = point;
    }
  };
  let end_at = |point: Point, end: &mut Point| {
    if point.offset > end.offset {
      *end = point;
    }
  };
  let node = root.node(id);
  if let (Some(first), Some(last)) = (node.end_comments.first(), node.end_comments.last()) {
    start_at(root.node(*first).position.start, &mut start);
    end_at(root.node(*last).position.end, &mut end);
  }
  if let (Some(first), Some(last)) = (children.first(), children.last()) {
    let (first, last) = (root.node(*first), root.node(*last));
    start_at(first.position.start, &mut start);
    end_at(last.position.end, &mut end);
    if let Some(comment) = first.leading_comments.first() {
      start_at(root.node(*comment).position.start, &mut start);
    }
    if let Some(tag) = first.tag {
      start_at(root.node(tag).position.start, &mut start);
    }
    if let Some(anchor) = first.anchor {
      start_at(root.node(anchor).position.start, &mut start);
    }
    if let Some(comment) = last.trailing_comment {
      end_at(root.node(comment).position.end, &mut end);
    }
  }
  let node = root.node_mut(id);
  node.position.start = start;
  node.position.end = end;
}
