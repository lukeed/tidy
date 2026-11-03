//! Comments, with the neighboring tokens and nodes of each: bun's `src/lint/ast/comments.rs` (MIT). bun's parser
//! records the nodes; here they come from Prettier's `decorateComment` (`src/main/comments/attach.js`) over oxc's nodes,
//! which are ESTree's but for the ones [`attachable`] skips.

use super::{Expr, ExprKind, ExprTag, File, NONE, Node, Span, expression_id};
use oxc_ast::AstKind;
use oxc_span::GetSpan;

/// What the parser knows about a comment.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Record {
  pub(crate) span: Span,
  pub(crate) block: bool,
  pub(crate) multiline: bool,
  /// A line break between the comment and the token or comment before it, or only white space before it.
  pub(crate) newline_before: bool,
  /// A line break between the comment and the token or comment after it. Always true for a `//` comment.
  pub(crate) newline_after: bool,
  /// The end of the token before it, or 0.
  pub(crate) prev: u32,
  /// The start of the token after it, or the length of the text.
  pub(crate) next: u32,
  /// The smallest node that contains it, [`NONE`] at the top level.
  pub(crate) enclosing: u32,
  /// The closest child of `enclosing` that ends before it.
  pub(crate) preceding: u32,
  /// The closest child of `enclosing` that starts after it.
  pub(crate) following: u32,
}

/// Prettier's `canAttachComment` for ESTree, and the nodes that oxc has and bun's tree doesn't, such as an array's holes
/// and a type annotation's `:`. A `;` is only attachable where a statement is required: `isMeaningfulEmptyStatement`.
fn attachable(kind: AstKind, parent: Option<AstKind>) -> bool {
  match kind {
    AstKind::EmptyStatement(_) => matches!(
      parent,
      Some(
        AstKind::IfStatement(_)
          | AstKind::ForStatement(_)
          | AstKind::ForInStatement(_)
          | AstKind::ForOfStatement(_)
          | AstKind::WhileStatement(_)
          | AstKind::DoWhileStatement(_)
          | AstKind::WithStatement(_)
          | AstKind::LabeledStatement(_)
      )
    ),
    _ => !matches!(
      kind,
      AstKind::TemplateElement(_)
        | AstKind::FormalParameters(_)
        | AstKind::CatchParameter(_)
        | AstKind::WithClause(_)
        | AstKind::Elision(_)
        | AstKind::TSTypeAnnotation(_)
    ),
  }
}

impl<'a> File<'a> {
  /// The comment with `id`, its index in source order.
  pub(crate) fn comment(&'a self, id: u32) -> Comment<'a> {
    Comment { file: self, id, record: self.records[id as usize] }
  }

  pub(crate) fn comment_count(&self) -> usize {
    self.records.len()
  }

  /// All comments in source order.
  pub(crate) fn comments_by_id(&'a self) -> impl ExactSizeIterator<Item = Comment<'a>> {
    (0..self.records.len() as u32).map(move |id| self.comment(id))
  }

  /// The comments inside `span`, in source order.
  pub(crate) fn comments_in(&'a self, span: Span) -> impl DoubleEndedIterator<Item = Comment<'a>> {
    let start = self.records.partition_point(|it| it.span.start < span.start);
    let end = self.records.partition_point(|it| it.span.end <= span.end).max(start);
    (start as u32..end as u32).map(move |id| self.comment(id))
  }

  /// The `#!` line, without its line break.
  pub(crate) fn hashbang(&self) -> Option<Span> {
    self.program.hashbang.as_ref().map(|it| Span::new(it.span.start, it.span.end))
  }

  /// The records of the comments, in source order.
  pub(crate) fn decorate(&self, typescript: bool) -> Vec<Record> {
    let text = self.text.as_bytes();
    let spans: Vec<Span> =
      self.program.comments.iter().map(|it| Span::new(it.span.start, it.span.end)).collect();
    let mut records = Vec::with_capacity(spans.len());
    let mut sorted = rustc_hash::FxHashMap::default();
    for (index, comment) in self.program.comments.iter().enumerate() {
      let span = spans[index];
      let prev = self.token_before(span.start);
      let next = self.token_after(span.end);
      let gap = |from: u32, to: u32| text.get(from as usize..to as usize).unwrap_or_default();
      let newline_before = prev == 0 && gap(0, span.start).iter().all(u8::is_ascii_whitespace)
        || breaks(gap(
          spans.get(index.wrapping_sub(1)).map_or(prev, |it| it.end.max(prev)),
          span.start,
        ));
      let after = spans.get(index + 1).map_or(next, |it| it.start.min(next));
      let newline_after = comment.is_line() || breaks(gap(span.end, after));
      let (mut enclosing, mut preceding, mut following) =
        self.surrounding(span, typescript, &mut sorted);
      // A comment after an expression, inside the parentheses around it, is printed after them, and the next
      // formatting finds it there. So its nodes are those there, where the output is stable.
      let (mut next, mut at) = (next, span.end);
      while following == NONE
        && !newline_before
        && preceding != NONE
        && let Some(Node::Expr(e)) = Node::of(self, preceding)
        && let Some(paren) = e
          .parens()
          .find(|paren| paren.start < span.start && span.end <= paren.end && at < paren.end)
        // The parentheses around an expression of a template aren't in its source.
        && !(paren.start == 0 && self.path.first() == Some(&0))
        && !keeps_comments_in_parentheses(self, e.id)
      {
        let after = Span::new(paren.end, paren.end + 1);
        (next, at) = (self.token_after(paren.end), paren.end);
        (enclosing, preceding, following) = self.surrounding(after, typescript, &mut sorted);
      }
      let mut record = Record {
        span,
        block: comment.is_block(),
        multiline: comment.is_multiline_block(),
        newline_before,
        newline_after,
        prev,
        next,
        enclosing,
        preceding,
        following,
      };
      self.lead_left_side(&mut record, typescript, &mut sorted);
      records.push(record);
    }
    records
  }

  /// A comment before the left side of an expression, inside the parentheses around it, is printed before the
  /// expression, and the next formatting finds it there. A member chain prints it after the left side of its link.
  fn lead_left_side(
    &self,
    record: &mut Record,
    typescript: bool,
    sorted: &mut rustc_hash::FxHashMap<u32, Vec<u32>>,
  ) {
    while record.preceding == NONE
      && let Some(Node::Expr(left)) = Node::of(self, record.following)
      && let Some(Node::Expr(e)) = Node::of(self, record.enclosing)
      && e.has_naked_left_side()
      && e.left_side().is_some_and(|it| it.id == left.id)
      // Prettier prints the comments of a function that is called or tagged inside its parentheses.
      && !(left.as_fn().is_some() && matches!(e.tag(), ExprTag::Call | ExprTag::TaggedTemplate))
      && let start = self.decorated_span(e.id).start
      && 0 < start
      && start < record.span.start
      && !is_ignore(&self.text.as_bytes()[record.span.start as usize..record.span.end as usize])
    {
      if is_in_member_chain(e) {
        if is_link(left)
          && let Some(side) = left.left_side()
        {
          let end = side.parens().last().map_or_else(|| side.span().end, |it| it.end);
          (record.newline_before, record.prev, record.next) = (false, end, self.token_after(end));
          let after = Span::new(end, end + 1);
          (record.enclosing, record.preceding, record.following) =
            self.surrounding(after, typescript, sorted);
        }
        return;
      }
      record.prev = self.token_before(start);
      let before = Span::new(start - 1, start);
      (record.enclosing, record.preceding, record.following) =
        self.surrounding(before, typescript, sorted);
    }
  }

  /// The end of the token before `at`: white space and comments are skipped.
  fn token_before(&self, mut at: u32) -> u32 {
    let (text, comments) = (self.text.as_bytes(), &self.program.comments);
    loop {
      while at > 0 && is_space(text, at - 1) {
        at -= 1;
      }
      let index = comments.partition_point(|it| it.span.end < at);
      match comments.get(index) {
        Some(comment) if comment.span.end == at => at = comment.span.start,
        _ => return at,
      }
    }
  }

  /// The start of the token after `at`.
  fn token_after(&self, mut at: u32) -> u32 {
    let (text, comments) = (self.text.as_bytes(), &self.program.comments);
    loop {
      while (at as usize) < text.len() && is_space(text, at) {
        at += 1;
      }
      let index = comments.partition_point(|it| it.span.start < at);
      match comments.get(index) {
        Some(comment) if comment.span.start == at => at = comment.span.end,
        _ => return at,
      }
    }
  }

  /// Prettier's `decorateComment`: the innermost node that contains `comment`, and that node's children right before and
  /// after it.
  /// `sorted` keeps the sorted children of each node that a comment was in, for the comments after it.
  fn surrounding(
    &self,
    comment: Span,
    typescript: bool,
    sorted: &mut rustc_hash::FxHashMap<u32, Vec<u32>>,
  ) -> (u32, u32, u32) {
    let mut enclosing = 0;
    'descend: loop {
      let children = sorted.entry(enclosing).or_insert_with(|| {
        let mut children = Vec::new();
        self.sorted_children(enclosing, typescript, &mut children);
        children
      });
      let (mut preceding, mut following) = (NONE, NONE);
      let (mut left, mut right) = (0, children.len());
      while left < right {
        let middle = (left + right) / 2;
        let child = children[middle];
        let span = self.decorated_span(child);
        if span.start <= comment.start && comment.end <= span.end {
          enclosing = child;
          continue 'descend;
        }
        if span.end <= comment.start {
          preceding = child;
          left = middle + 1;
        } else if comment.end <= span.start {
          following = child;
          right = middle;
        } else {
          break;
        }
      }
      if let Some(AstKind::TemplateLiteral(template)) = self.kind(enclosing) {
        let index =
          |span: Span| template.quasis.iter().take_while(|it| it.span.start < span.start).count();
        let at = index(comment);
        if preceding != NONE && index(self.span_of(preceding)) != at {
          preceding = NONE;
        }
        if following != NONE && index(self.span_of(following)) != at {
          following = NONE;
        }
      }
      let enclosing = if enclosing == 0 { NONE } else { enclosing };
      return (enclosing, preceding, following);
    }
  }

  /// Prettier's `getSortedChildNodes`: the children that a comment can attach to, with those of the others in their
  /// place, by position.
  fn sorted_children(&self, id: u32, typescript: bool, out: &mut Vec<u32>) {
    let start = out.len();
    match self.method_without_parameters(id, typescript) {
      Some(children) => out.extend(children),
      None => self.attachable_children(id, out),
    }
    out[start..].sort_by_key(|&child| {
      let span = self.span_of(child);
      (span.start, span.end)
    });
  }

  fn attachable_children(&self, id: u32, out: &mut Vec<u32>) {
    let parent = self.kind(id);
    // The `const` of `as const` isn't bun's node.
    let constant = match parent {
      Some(AstKind::TSAsExpression(it)) if super::expr::is_const(&it.type_annotation) => {
        Some(it.type_annotation.node_id().index() as u32)
      }
      Some(AstKind::TSTypeAssertion(it)) if super::expr::is_const(&it.type_annotation) => {
        Some(it.type_annotation.node_id().index() as u32)
      }
      _ => None,
    };
    for &child in self.children(id).iter().filter(|&&child| Some(child) != constant) {
      match self.kind(child) {
        Some(kind) if attachable(kind, parent) => out.push(child),
        Some(_) => self.attachable_children(child, out),
        None => {}
      }
    }
  }

  /// `getCommentChildNodes` of TypeScript: a method without parameters, return type or type parameters has its
  /// decorators, its key and its body as children, so that a comment before the body isn't the function's.
  fn method_without_parameters(&self, id: u32, typescript: bool) -> Option<Vec<u32>> {
    let Some(AstKind::MethodDefinition(method)) = self.kind(id).filter(|_| typescript) else {
      return None;
    };
    let function = &method.value;
    let body = function.body.as_ref()?;
    if !function.params.items.is_empty()
      || function.params.rest.is_some()
      || function.this_param.is_some()
      || function.return_type.is_some()
      || function.type_parameters.is_some()
    {
      return None;
    }
    let mut children: Vec<u32> =
      method.decorators.iter().map(|it| it.node_id.get().index() as u32).collect();
    children.push(method.key.node_id().index() as u32);
    children.push(body.node_id.get().index() as u32);
    Some(children)
  }

  /// Prettier's `locEnd` leaves out the `;` that ends a statement, so a comment before it comes after the statement.
  /// Its `locStart` of a class, and of an `export` of one, is the first decorator, which can come before the `export`.
  fn decorated_span(&self, id: u32) -> Span {
    let span = self.span_of(id);
    let span = match self.ends_with_its_semicolon(id) {
      true => self.without_semicolon(span),
      false => span,
    };
    match self.first_decorator(id) {
      Some(start) if start < span.start => Span::new(start, span.end),
      _ => span,
    }
  }

  /// Where the first decorator of a class, or of the class an `export` declares, starts.
  fn first_decorator(&self, id: u32) -> Option<u32> {
    let class = match self.kind(id)? {
      AstKind::Class(class) => class,
      AstKind::ExportDeclaration(it) => match &it.declaration {
        oxc_ast::ast::Declaration::ClassDeclaration(class) => class,
        _ => return None,
      },
      AstKind::ExportDefaultDeclaration(it) => match &it.declaration {
        oxc_ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => class,
        _ => return None,
      },
      _ => return None,
    };
    class.decorators.first().map(|it| it.span.start)
  }

  /// True for a statement that ends with its own `;`, or with that of its last statement: `if (a) b;`.
  fn ends_with_its_semicolon(&self, mut id: u32) -> bool {
    loop {
      match self.kind(id) {
        Some(
          AstKind::ExpressionStatement(_)
          | AstKind::Directive(_)
          | AstKind::VariableDeclaration(_)
          | AstKind::ReturnStatement(_)
          | AstKind::DoWhileStatement(_)
          | AstKind::ThrowStatement(_)
          | AstKind::BreakStatement(_)
          | AstKind::ContinueStatement(_)
          | AstKind::DebuggerStatement(_)
          | AstKind::ImportDeclaration(_)
          | AstKind::ExportNamedDeclaration(_)
          | AstKind::ExportFromDeclaration(_)
          | AstKind::ExportAllDeclaration(_)
          | AstKind::ExportDefaultDeclaration(_),
        ) => return true,
        Some(
          AstKind::IfStatement(_)
          | AstKind::ForStatement(_)
          | AstKind::ForInStatement(_)
          | AstKind::ForOfStatement(_)
          | AstKind::WhileStatement(_)
          | AstKind::LabeledStatement(_)
          | AstKind::WithStatement(_)
          | AstKind::ExportDeclaration(_),
        ) => match self.children(id).last() {
          Some(&last) => id = last,
          None => return false,
        },
        _ => return false,
      }
    }
  }

  pub(crate) fn span_of(&self, id: u32) -> Span {
    self.kind(id).map_or(Span::default(), |kind| {
      let span = kind.span();
      Span::new(span.start, span.end)
    })
  }

  /// Trims the trailing `;` off the `span` of a statement or class member, along with any white space and comments
  /// before it.
  pub(crate) fn without_semicolon(&self, span: Span) -> Span {
    if span.is_empty() || self.text.as_bytes().get(span.end as usize - 1) != Some(&b';') {
      return span;
    }
    Span::new(span.start, self.token_before(span.end - 1).max(span.start))
  }
}

fn is_space(text: &[u8], at: u32) -> bool {
  matches!(text.get(at as usize), Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c'))
}

fn breaks(gap: &[u8]) -> bool {
  gap.iter().any(|&byte| byte == b'\n' || byte == b'\r')
}

/// `// prettier-ignore`, which ignores the node that it leads.
fn is_ignore(text: &[u8]) -> bool {
  let content = match text {
    [b'/', b'*', content @ .., b'*', b'/'] | [b'/', b'/', content @ ..] => content,
    content => content,
  };
  matches!(crate::js::strings::trim_js_whitespace(content), b"prettier-ignore" | b"oxfmt-ignore")
}

/// A call or a member access, which a member chain prints as one of its links.
fn is_link(e: Expr<'_>) -> bool {
  matches!(e.tag(), ExprTag::Call | ExprTag::Dot | ExprTag::Index)
}

/// A link of a call or member access that ends in a call of a member, which Prettier prints as a member chain.
fn is_in_member_chain(mut e: Expr<'_>) -> bool {
  while is_link(e) {
    if let ExprKind::Call(call) = e.kind()
      && is_link(call.callee())
    {
      return true;
    }
    match e.parent() {
      Node::Expr(parent) if parent.left_side().is_some_and(|it| it.id == e.id) => e = parent,
      _ => return false,
    }
  }
  false
}

/// True if the expression with `id` keeps a comment after it inside its parentheses when they're printed: an operand
/// of `!` or `-` (Prettier's `printUnaryExpression`), JSX, which prints its own parentheses, and a sequence or an
/// assignment that is the body of an arrow function, the value of a declaration or of an assignment, or what a `return`
/// returns (`handleParenthesizedExpressionTrailingComment`). An assignment to the right of another loses its
/// parentheses: `a = (b = c)` is `a = b = c`.
fn keeps_comments_in_parentheses(file: &File, id: u32) -> bool {
  let parent = file.kind(file.parent_id(id));
  match (file.kind(id), parent) {
    (_, Some(AstKind::UnaryExpression(_))) => true,
    (Some(AstKind::JSXElement(_) | AstKind::JSXFragment(_)), _) => true,
    (Some(AstKind::AssignmentExpression(_)), Some(AstKind::AssignmentExpression(_))) => false,
    (Some(AstKind::SequenceExpression(_) | AstKind::AssignmentExpression(_)), _) => matches!(
      parent,
      Some(
        AstKind::ArrowFunctionExpression(_)
          | AstKind::VariableDeclarator(_)
          | AstKind::ReturnStatement(_)
          | AstKind::AssignmentExpression(_)
      )
    ),
    _ => false,
  }
}

/// A `//` or `/* */` comment.
#[derive(Copy, Clone)]
pub(crate) struct Comment<'a> {
  pub(crate) file: &'a File<'a>,
  pub(crate) id: u32,
  pub(crate) record: Record,
}

/// A node, or an identifier or literal key by its start, which bun's tree has as fields rather than nodes.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum NodeOrName<'a> {
  Node(Node<'a>),
  Name(u32),
}

impl<'a> Comment<'a> {
  /// Index into the file's comments.
  pub(crate) fn id(self) -> u32 {
    self.id
  }

  pub(crate) fn span(self) -> Span {
    self.record.span
  }

  /// The source text, with the `//` or the `/*` and `*/`.
  pub(crate) fn text(self) -> &'a [u8] {
    self.file.slice(self.span())
  }

  /// The text between `/*` and `*/`, or after `//`.
  pub(crate) fn content(self) -> &'a [u8] {
    match self.text() {
      [b'/', b'*', content @ .., b'*', b'/'] | [b'/', b'/', content @ ..] => content,
      text => text,
    }
  }

  pub(crate) fn is_line(self) -> bool {
    !self.record.block
  }

  pub(crate) fn is_block(self) -> bool {
    self.record.block
  }

  pub(crate) fn is_multiline_block(self) -> bool {
    self.record.multiline
  }

  pub(crate) fn has_newline_before(self) -> bool {
    self.record.newline_before
  }

  pub(crate) fn has_newline_after(self) -> bool {
    self.record.newline_after
  }

  pub(crate) fn prev_token_end(self) -> u32 {
    self.record.prev
  }

  pub(crate) fn next_token_start(self) -> u32 {
    self.record.next
  }

  /// A decorator, a `catch` clause and `<..>` aren't bun's nodes. As bun's parser records them, a decorator stands for
  /// its expression, a `catch` clause for its body, and `<..>` for its last parameter or argument before the comment and
  /// its first after it. The key of a mapped type is a name. The
  /// `{ .. }` of a function, a class, an interface, an enum or a module is neither.
  fn node_or_name(self, mut id: u32, last: bool) -> Option<NodeOrName<'a>> {
    if id == NONE {
      return None;
    }
    match self.file.kind(id) {
      Some(
        AstKind::ClassBody(_)
        | AstKind::FunctionBody(_)
        | AstKind::TSInterfaceBody(_)
        | AstKind::TSEnumBody(_)
        | AstKind::TSModuleBlock(_),
      ) => return None,
      Some(AstKind::Decorator(it)) => id = expression_id(&it.expression),
      Some(AstKind::CatchClause(it)) => id = it.body.node_id.get().index() as u32,
      Some(AstKind::BindingIdentifier(it))
        if matches!(self.file.kind(self.file.parent_id(id)), Some(AstKind::TSMappedType(_))) =>
      {
        return Some(NodeOrName::Name(it.span.start));
      }
      Some(AstKind::TSTypeParameterDeclaration(_) | AstKind::TSTypeParameterInstantiation(_)) => {
        let children = self.file.children(id);
        id = *(if last { children.last() } else { children.first() }).unwrap_or(&id);
      }
      _ => {}
    }
    match Node::of(self.file, id) {
      Some(node) => Some(NodeOrName::Node(node)),
      None => Some(NodeOrName::Name(self.file.span_of(id).start)),
    }
  }

  /// The smallest node that contains it. `None` at the top level of the file.
  pub(crate) fn enclosing_node(self) -> Option<Node<'a>> {
    let id = self.record.enclosing;
    (id != NONE).then(|| Node::of(self.file, id).unwrap_or_else(|| Node::around(self.file, id)))
  }

  /// The closest child of the enclosing node that ends before it.
  pub(crate) fn preceding_node(self) -> Option<NodeOrName<'a>> {
    self.node_or_name(self.record.preceding, true)
  }

  /// The closest child of the enclosing node that starts after it.
  pub(crate) fn following_node(self) -> Option<NodeOrName<'a>> {
    self.node_or_name(self.record.following, false)
  }
}
