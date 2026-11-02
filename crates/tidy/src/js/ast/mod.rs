//! The syntax tree as bun's printer reads it: the handles of bun's `bun_lint::ast`, over oxc's tree. See `../readme.md`.

pub(crate) mod comments;
mod decl;
mod expr;
mod hir;
mod key;
mod list;
mod name;
mod node;
mod pat;
mod span;
mod stmt;
mod ty;

pub(crate) use comments::*;
pub(crate) use decl::*;
pub(crate) use expr::*;
pub(crate) use hir::*;
pub(crate) use key::*;
pub(crate) use list::*;
pub(crate) use name::*;
pub(crate) use node::*;
pub(crate) use pat::*;
pub(crate) use span::{Span, Spanned};
pub(crate) use stmt::*;
pub(crate) use ty::*;

use oxc_ast::AstKind;
use oxc_ast::ast::Program;
use oxc_ast_visit::Visit;
use oxc_span::SourceType;

/// No node: the parent of the program.
pub(crate) const NONE: u32 = u32::MAX;

/// A parsed file, with tables of its nodes by id.
pub(crate) struct File<'a> {
  pub(crate) text: &'a str,
  /// Starts with a NUL byte for an expression in HTML.
  path: &'a [u8],
  source_type: SourceType,
  /// For the names that the source doesn't spell out, such as `1000` for the key `1e3`.
  allocator: &'a oxc_allocator::Allocator,
  pub(crate) program: &'a Program<'a>,
  kinds: Vec<Option<AstKind<'a>>>,
  parents: Vec<u32>,
  /// Each node's children in the order of oxc's visitor, which is the order of their fields, grouped by parent.
  children: Vec<u32>,
  /// Where each node's children start in `children`, and how many there are.
  ranges: Vec<(u32, u32)>,
  /// Each node's index in `children`.
  positions: Vec<u32>,
  /// The parentheses around each node that has some, by node id and then innermost first.
  parens: Vec<(u32, Span)>,
  /// The comments, in source order.
  records: Vec<Record>,
}

impl<'a> File<'a> {
  pub(crate) fn new(
    text: &'a str,
    path: &'a [u8],
    source_type: SourceType,
    program: &'a Program<'a>,
    allocator: &'a oxc_allocator::Allocator,
  ) -> File<'a> {
    let mut walk = Walk::default();
    walk.visit_program(program);
    let Walk { kinds, parents, edges, mut parens, .. } = walk;
    parens.sort_by_key(|it| it.0);
    let mut ranges = vec![(0, 0); kinds.len()];
    for &(parent, _) in &edges {
      ranges[parent as usize].1 += 1;
    }
    let mut start = 0;
    for range in &mut ranges {
      range.0 = start;
      start += range.1;
    }
    let mut filled = vec![0; kinds.len()];
    let mut children = vec![NONE; edges.len()];
    let mut positions = vec![NONE; kinds.len()];
    for (parent, child) in edges {
      let at = ranges[parent as usize].0 + filled[parent as usize];
      filled[parent as usize] += 1;
      children[at as usize] = child;
      positions[child as usize] = at;
    }
    let mut file = File {
      text,
      path,
      source_type,
      allocator,
      program,
      kinds,
      parents,
      children,
      ranges,
      positions,
      parens,
      records: Vec::new(),
    };
    file.records = file.decorate(source_type.is_typescript());
    file
  }

  pub(crate) fn path(&self) -> &'a [u8] {
    self.path
  }

  /// `.js`, `.jsx`, `.mjs`, `.cjs`
  pub(crate) fn is_javascript(&self) -> bool {
    !self.source_type.is_typescript()
  }

  /// The top-level statements, with the directives first.
  pub(crate) fn body(&'a self) -> List<'a, Stmt<'a>> {
    decl::block(self, &self.program.directives, &self.program.body)
  }

  /// The size of the tables: every [`Node::index`] is below it.
  pub(crate) fn node_count(&self) -> u32 {
    self.kinds.len() as u32
  }

  /// The node with `id`, unless it's skipped: parentheses, a chain, or no node at all.
  pub(crate) fn kind(&self, id: u32) -> Option<AstKind<'a>> {
    self.kinds.get(id as usize).copied().flatten()
  }

  /// The id of the parent of the node with `id`, or [`NONE`] for the program. Parentheses and chains are skipped.
  pub(crate) fn parent_id(&self, id: u32) -> u32 {
    self.parents.get(id as usize).copied().unwrap_or(NONE)
  }

  /// The ids of the children of the node with `id`.
  pub(crate) fn children(&self, id: u32) -> &[u32] {
    let (start, len) = self.ranges.get(id as usize).copied().unwrap_or_default();
    &self.children[start as usize..(start + len) as usize]
  }

  /// The index of the node with `id` in [`File::children_at`].
  pub(crate) fn position(&self, id: u32) -> u32 {
    self.positions.get(id as usize).copied().unwrap_or(NONE)
  }

  /// The ids from index `start` of the table of children: a run of siblings.
  pub(crate) fn children_at(&self, start: u32, len: u32) -> &[u32] {
    self.children.get(start as usize..(start + len) as usize).unwrap_or_default()
  }

  /// The parentheses around the node with `id`, innermost first.
  pub(crate) fn parens(&self, id: u32) -> &[(u32, Span)] {
    let first = self.parens.partition_point(|it| it.0 < id);
    let count = self.parens[first..].iter().take_while(|it| it.0 == id).count();
    &self.parens[first..first + count]
  }

  /// A copy of `text` that lives as long as the tree.
  pub(crate) fn keep(&self, text: &str) -> &'a str {
    self.allocator.alloc_str(text)
  }

  pub(crate) fn slice(&self, span: Span) -> &'a [u8] {
    self.text.as_bytes().get(span.start as usize..span.end as usize).unwrap_or_default()
  }

  pub(crate) fn str(&self, span: Span) -> &'a str {
    self.text.get(span.start as usize..span.end as usize).unwrap_or_default()
  }

  /// The whole source text.
  pub(crate) fn text(&self) -> &'a [u8] {
    self.text.as_bytes()
  }
}

/// True for the nodes that bun's tree doesn't have: their children belong to their parent.
fn skipped(kind: AstKind) -> bool {
  matches!(
    kind,
    AstKind::ParenthesizedExpression(_)
      | AstKind::ChainExpression(_)
      | AstKind::TSParenthesizedType(_)
  )
}

#[derive(Default)]
struct Walk<'a> {
  kinds: Vec<Option<AstKind<'a>>>,
  parents: Vec<u32>,
  edges: Vec<(u32, u32)>,
  /// The nodes being walked, innermost last.
  stack: Vec<u32>,
  parens: Vec<(u32, Span)>,
  /// The parentheses entered since the last node, outermost first: they wrap the next node.
  pending: Vec<Span>,
}

impl<'a> Visit<'a> for Walk<'a> {
  fn enter_node(&mut self, kind: AstKind<'a>) {
    match kind {
      AstKind::ParenthesizedExpression(parens) => {
        self.pending.push(Span::new(parens.span.start, parens.span.end));
      }
      AstKind::TSParenthesizedType(parens) => {
        self.pending.push(Span::new(parens.span.start, parens.span.end));
      }
      _ => {}
    }
    if skipped(kind) {
      return;
    }
    let id = kind.node_id().index();
    self.parens.extend(self.pending.drain(..).rev().map(|span| (id as u32, span)));
    if self.kinds.len() <= id {
      self.kinds.resize(id + 1, None);
      self.parents.resize(id + 1, NONE);
    }
    self.kinds[id] = Some(kind);
    let id = id as u32;
    if let Some(&parent) = self.stack.last() {
      self.parents[id as usize] = parent;
      self.edges.push((parent, id));
    }
    self.stack.push(id);
  }

  fn leave_node(&mut self, kind: AstKind<'a>) {
    if !skipped(kind) {
      self.stack.pop();
    }
  }
}
