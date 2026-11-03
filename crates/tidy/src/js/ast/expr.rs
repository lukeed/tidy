//! Expressions: bun's `src/lint/ast/expr.rs` (MIT), over oxc's tree.
//!
//! Where bun's tree differs from oxc's: `a, b, c` is one [`ExprKind::Sequence`] rather than nested commas, and
//! `x!!` is two [`ExprKind::NonNull`]s. The `a.b` of `typeof a.b` is a `Dot`.

use super::{Class, Expr, File, Func, Handle, Ident, List, Name, Node, Prop, Span, Stmt, TypeNode};
use super::{argument_id, expression_id, type_id};
use oxc_ast::AstKind;
use oxc_ast::ast::{
  AssignmentOperator, AssignmentTarget, BinaryOperator, Expression, JSXElementName, JSXExpression,
  JSXMemberExpressionObject, LogicalOperator, MemberExpression, SimpleAssignmentTarget, TSType,
  UnaryOperator, UpdateOperator,
};

#[derive(Copy, Clone, Debug)]
#[allow(dead_code, reason = "bun's tree shape, which the printer matches on")]
pub(crate) enum ExprKind<'a> {
  /// A hole in an array literal, the empty `{}` of JSX, or the missing argument of `import()`.
  Missing,
  Ident(Name<'a>),
  /// The `#x` of `#x in a`. The name includes the `#`.
  PrivateIdentifier(Name<'a>),
  This,
  Super,
  Null,
  True,
  False,
  Number(f64),
  /// The value of `"a"` or `'a'`. In JSX it's also an attribute value, the text between elements
  /// ([`Expr::is_jsx_text`]), and a tag name such as `a-b` or `a:b`.
  String(Name<'a>),
  BigInt(Name<'a>),
  Regex(Regex<'a>),
  Template(Template<'a>),
  /// The tag is the callee and the substitutions are the arguments.
  TaggedTemplate(Call<'a>),
  /// A hole is `Missing`.
  Array(List<'a, Expr<'a>>),
  Object(List<'a, Prop<'a>>),
  /// A function expression or an arrow function. Also the value of an object literal's method or accessor.
  Fn(Func<'a>),
  Class(Class<'a>),
  /// `obj.name`, `obj?.name`, `obj.#name`
  Dot {
    obj: Expr<'a>,
    name: Ident<'a>,
    chain: Chain,
  },
  /// `obj[index]`, `obj?.[index]`
  Index {
    obj: Expr<'a>,
    index: Expr<'a>,
    chain: Chain,
  },
  Call(Call<'a>),
  New(Call<'a>),
  /// Includes `++` and `--`, `typeof`, `void` and `delete`.
  Unary {
    op: UnOp,
    operand: Expr<'a>,
  },
  /// Includes `&&`, `||`, `??`, `in` and `instanceof`.
  Binary {
    op: BinOp,
    left: Expr<'a>,
    right: Expr<'a>,
  },
  /// `a, b, c`
  Sequence(List<'a, Expr<'a>>),
  /// `op` is `None` for plain `=`. A destructuring target is an `Array` or an `Object`.
  Assign {
    op: Option<BinOp>,
    target: Expr<'a>,
    value: Expr<'a>,
  },
  Cond {
    test: Expr<'a>,
    yes: Expr<'a>,
    no: Expr<'a>,
  },
  Spread(Expr<'a>),
  Await(Expr<'a>),
  Yield {
    value: Option<Expr<'a>>,
    star: bool,
  },
  /// `expr as ty` or `<ty>expr`. [`Expr::is_angle_bracket_assertion`] distinguishes them.
  As {
    expr: Expr<'a>,
    ty: TypeNode<'a>,
  },
  Satisfies {
    expr: Expr<'a>,
    ty: TypeNode<'a>,
  },
  /// `expr as const`, `<const>expr`
  AsConst(Expr<'a>),
  /// `expr!`
  NonNull(Expr<'a>),
  /// `f<T>` without a call.
  Instantiation {
    expr: Expr<'a>,
    type_args: List<'a, TypeNode<'a>>,
  },
  Jsx(Jsx<'a>),
  /// `import(specifier, options)`
  ImportCall {
    args: List<'a, Expr<'a>>,
  },
  ImportMeta,
  NewTarget,
}

/// The kind of an expression, without its payload.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum ExprTag {
  Missing,
  Ident,
  PrivateIdentifier,
  This,
  Super,
  Null,
  True,
  False,
  Number,
  String,
  BigInt,
  Regex,
  Template,
  TaggedTemplate,
  Array,
  Object,
  Fn,
  Class,
  Dot,
  Index,
  Call,
  New,
  Unary,
  Binary,
  Sequence,
  Assign,
  Cond,
  Spread,
  Await,
  Yield,
  As,
  Satisfies,
  AsConst,
  NonNull,
  Instantiation,
  Jsx,
  ImportCall,
  ImportMeta,
  NewTarget,
}

/// Where an expression is in an optional chain.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Chain {
  No,
  /// `a?.b`
  Start,
  /// The `.c` of `a?.b.c`
  Continue,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum UnOp {
  Plus,
  Minus,
  BitNot,
  Not,
  Typeof,
  Void,
  Delete,
  PreInc,
  PreDec,
  PostInc,
  PostDec,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum BinOp {
  Add,
  Sub,
  Mul,
  Div,
  Rem,
  Pow,
  Shl,
  Shr,
  UShr,
  BitAnd,
  BitOr,
  BitXor,
  Lt,
  Le,
  Gt,
  Ge,
  EqEq,
  NotEq,
  EqEqEq,
  NotEqEq,
  In,
  Instanceof,
  And,
  Or,
  Nullish,
  Comma,
}

impl<'a> Handle<'a> for Expr<'a> {
  /// The expression in a JSX `{e}` stands for it.
  fn from_raw(file: &'a File<'a>, id: u32) -> Self {
    match file.kind(id) {
      Some(AstKind::JSXExpressionContainer(container)) => {
        let id = match &container.expression {
          JSXExpression::EmptyExpression(empty) => empty.node_id.get().index() as u32,
          expression => expression.as_expression().map_or(id, expression_id),
        };
        Expr::new(file, id)
      }
      _ => Expr::new(file, id),
    }
  }
}

impl<'a> Expr<'a> {
  fn at(self, id: u32) -> Expr<'a> {
    Expr::from_raw(self.file, id)
  }

  fn of(self, expression: &Expression) -> Expr<'a> {
    Expr::new(self.file, expression_id(expression))
  }

  fn list<T: Handle<'a>>(self, first: Option<u32>, len: usize) -> List<'a, T> {
    List::run(self.file, first, len)
  }

  pub(crate) fn kind(self) -> ExprKind<'a> {
    use AstKind as K;
    let Some(raw) = self.file.kind(self.id) else { return ExprKind::Missing };
    match raw {
      K::Elision(_) | K::JSXEmptyExpression(_) => ExprKind::Missing,
      K::IdentifierReference(it) => ExprKind::Ident(Name(it.name.as_str())),
      K::PrivateIdentifier(_) => ExprKind::PrivateIdentifier(Name(self.source())),
      K::ThisExpression(_) => ExprKind::This,
      K::Super(_) => ExprKind::Super,
      K::NullLiteral(_) => ExprKind::Null,
      K::BooleanLiteral(it) => match it.value {
        true => ExprKind::True,
        false => ExprKind::False,
      },
      K::NumericLiteral(it) => ExprKind::Number(it.value),
      K::StringLiteral(it) => {
        let text = self.source();
        let quoted = text.get(1..text.len().saturating_sub(1)).unwrap_or_default();
        ExprKind::String(Name(it.value.as_str().unwrap_or(quoted)))
      }
      K::JSXText(it) => ExprKind::String(Name(it.value.as_str())),
      K::BigIntLiteral(it) => ExprKind::BigInt(Name(it.raw.map_or("", |raw| raw.as_str()))),
      K::RegExpLiteral(_) => ExprKind::Regex(Regex { expr: self }),
      K::TemplateLiteral(_) => ExprKind::Template(Template { expr: self }),
      K::TaggedTemplateExpression(_) => ExprKind::TaggedTemplate(Call { expr: self }),
      K::ArrayExpression(it) => {
        let first = it.elements.first().map(|element| match element.as_expression() {
          Some(expression) => expression_id(expression),
          None => element.node_id().index() as u32,
        });
        ExprKind::Array(self.list(first, it.elements.len()))
      }
      K::ArrayAssignmentTarget(it) => ExprKind::Array(match self.file.holey(self.id) {
        Some(ids) => List::ids(self.file, ids),
        None => {
          let first = self.file.children(self.id).first().copied();
          self.list(first, it.elements.len() + usize::from(it.rest.is_some()))
        }
      }),
      K::ObjectExpression(it) => {
        let first = it.properties.first().map(|property| property.node_id().index() as u32);
        ExprKind::Object(self.list(first, it.properties.len()))
      }
      K::ObjectAssignmentTarget(it) => {
        let len = it.properties.len() + usize::from(it.rest.is_some());
        ExprKind::Object(self.list(self.file.children(self.id).first().copied(), len))
      }
      K::Function(_) | K::ArrowFunctionExpression(_) => ExprKind::Fn(Func::new(self.file, self.id)),
      K::Class(_) => ExprKind::Class(Class::new(self.file, self.id)),
      K::StaticMemberExpression(it) => {
        let name = Ident {
          name: Name(it.property.name.as_str()),
          span: span(it.property.span),
          string: false,
        };
        ExprKind::Dot { obj: self.of(&it.object), name, chain: self.chain() }
      }
      K::PrivateFieldExpression(it) => {
        let span = span(it.field.span);
        let name = Ident { name: Name(self.file.str(span)), span, string: false };
        ExprKind::Dot { obj: self.of(&it.object), name, chain: self.chain() }
      }
      K::ComputedMemberExpression(it) => ExprKind::Index {
        obj: self.of(&it.object),
        index: self.of(&it.expression),
        chain: self.chain(),
      },
      K::JSXMemberExpression(it) => {
        let obj = match &it.object {
          JSXMemberExpressionObject::IdentifierReference(it) => it.node_id.get(),
          JSXMemberExpressionObject::MemberExpression(it) => it.node_id.get(),
          JSXMemberExpressionObject::ThisExpression(it) => it.node_id.get(),
        };
        let name = Ident {
          name: Name(it.property.name.as_str()),
          span: span(it.property.span),
          string: false,
        };
        ExprKind::Dot { obj: self.at(obj.index() as u32), name, chain: Chain::No }
      }
      K::TSQualifiedName(it) => {
        let name =
          Ident { name: Name(it.right.name.as_str()), span: span(it.right.span), string: false };
        ExprKind::Dot { obj: self.at(it.left.node_id().index() as u32), name, chain: Chain::No }
      }
      K::JSXIdentifier(it) if it.name.contains('-') => ExprKind::String(Name(it.name.as_str())),
      K::JSXIdentifier(it) => ExprKind::Ident(Name(it.name.as_str())),
      K::JSXNamespacedName(_) => ExprKind::String(Name(self.source())),
      K::CallExpression(_) => ExprKind::Call(Call { expr: self }),
      K::NewExpression(_) => ExprKind::New(Call { expr: self }),
      K::UnaryExpression(it) => {
        let op = match it.operator {
          UnaryOperator::UnaryPlus => UnOp::Plus,
          UnaryOperator::UnaryNegation => UnOp::Minus,
          UnaryOperator::BitwiseNot => UnOp::BitNot,
          UnaryOperator::LogicalNot => UnOp::Not,
          UnaryOperator::Typeof => UnOp::Typeof,
          UnaryOperator::Void => UnOp::Void,
          UnaryOperator::Delete => UnOp::Delete,
        };
        ExprKind::Unary { op, operand: self.of(&it.argument) }
      }
      K::UpdateExpression(it) => {
        let op = match (it.operator, it.prefix) {
          (UpdateOperator::Increment, true) => UnOp::PreInc,
          (UpdateOperator::Decrement, true) => UnOp::PreDec,
          (UpdateOperator::Increment, false) => UnOp::PostInc,
          (UpdateOperator::Decrement, false) => UnOp::PostDec,
        };
        ExprKind::Unary { op, operand: self.at(simple_target_id(&it.argument)) }
      }
      K::BinaryExpression(it) => ExprKind::Binary {
        op: binary(it.operator),
        left: self.of(&it.left),
        right: self.of(&it.right),
      },
      K::LogicalExpression(it) => {
        let op = match it.operator {
          LogicalOperator::And => BinOp::And,
          LogicalOperator::Or => BinOp::Or,
          LogicalOperator::Coalesce => BinOp::Nullish,
        };
        ExprKind::Binary { op, left: self.of(&it.left), right: self.of(&it.right) }
      }
      K::PrivateInExpression(it) => {
        let left = self.at(it.left.node_id.get().index() as u32);
        ExprKind::Binary { op: BinOp::In, left, right: self.of(&it.right) }
      }
      K::SequenceExpression(it) => ExprKind::Sequence(
        self.list(it.expressions.first().map(expression_id), it.expressions.len()),
      ),
      K::AssignmentExpression(it) => {
        let op = assignment(it.operator);
        ExprKind::Assign { op, target: self.at(target_id(&it.left)), value: self.of(&it.right) }
      }
      K::AssignmentTargetWithDefault(it) => ExprKind::Assign {
        op: None,
        target: self.at(target_id(&it.binding)),
        value: self.of(&it.init),
      },
      K::ConditionalExpression(it) => ExprKind::Cond {
        test: self.of(&it.test),
        yes: self.of(&it.consequent),
        no: self.of(&it.alternate),
      },
      K::SpreadElement(it) => ExprKind::Spread(self.of(&it.argument)),
      K::JSXSpreadChild(it) => ExprKind::Spread(self.of(&it.expression)),
      K::AssignmentTargetRest(it) => ExprKind::Spread(self.at(target_id(&it.target))),
      K::AwaitExpression(it) => ExprKind::Await(self.of(&it.argument)),
      K::YieldExpression(it) => {
        ExprKind::Yield { value: it.argument.as_ref().map(|it| self.of(it)), star: it.delegate }
      }
      K::TSAsExpression(it) if is_const(&it.type_annotation) => {
        ExprKind::AsConst(self.of(&it.expression))
      }
      K::TSTypeAssertion(it) if is_const(&it.type_annotation) => {
        ExprKind::AsConst(self.of(&it.expression))
      }
      K::TSAsExpression(it) => {
        ExprKind::As { expr: self.of(&it.expression), ty: self.ty(&it.type_annotation) }
      }
      K::TSTypeAssertion(it) => {
        ExprKind::As { expr: self.of(&it.expression), ty: self.ty(&it.type_annotation) }
      }
      K::TSSatisfiesExpression(it) => {
        ExprKind::Satisfies { expr: self.of(&it.expression), ty: self.ty(&it.type_annotation) }
      }
      K::TSNonNullExpression(it) => ExprKind::NonNull(self.of(&it.expression)),
      K::TSInstantiationExpression(it) => {
        let params = &it.type_arguments.params;
        let first = params.first().map(type_id);
        ExprKind::Instantiation {
          expr: self.of(&it.expression),
          type_args: self.list(first, params.len()),
        }
      }
      K::JSXElement(_) | K::JSXFragment(_) => ExprKind::Jsx(Jsx { expr: self }),
      K::ImportExpression(it) => {
        let len = 1 + usize::from(it.options.is_some());
        ExprKind::ImportCall { args: self.list(Some(expression_id(&it.source)), len) }
      }
      K::ImportMeta(_) => ExprKind::ImportMeta,
      K::NewTarget(_) => ExprKind::NewTarget,
      _ => ExprKind::Missing,
    }
  }

  fn ty(self, ty: &TSType) -> TypeNode<'a> {
    TypeNode::of(self.file, ty)
  }

  pub(crate) fn tag(self) -> ExprTag {
    let memo = self.file.tags.get(self.id as usize);
    if let Some(tag) = memo.and_then(std::cell::Cell::get) {
      return tag;
    }
    let tag = self.tag_of_kind();
    if let Some(memo) = memo {
      memo.set(Some(tag));
    }
    tag
  }

  fn tag_of_kind(self) -> ExprTag {
    match self.kind() {
      ExprKind::Missing => ExprTag::Missing,
      ExprKind::Ident(_) => ExprTag::Ident,
      ExprKind::PrivateIdentifier(_) => ExprTag::PrivateIdentifier,
      ExprKind::This => ExprTag::This,
      ExprKind::Super => ExprTag::Super,
      ExprKind::Null => ExprTag::Null,
      ExprKind::True => ExprTag::True,
      ExprKind::False => ExprTag::False,
      ExprKind::Number(_) => ExprTag::Number,
      ExprKind::String(_) => ExprTag::String,
      ExprKind::BigInt(_) => ExprTag::BigInt,
      ExprKind::Regex(_) => ExprTag::Regex,
      ExprKind::Template(_) => ExprTag::Template,
      ExprKind::TaggedTemplate(_) => ExprTag::TaggedTemplate,
      ExprKind::Array(_) => ExprTag::Array,
      ExprKind::Object(_) => ExprTag::Object,
      ExprKind::Fn(_) => ExprTag::Fn,
      ExprKind::Class(_) => ExprTag::Class,
      ExprKind::Dot { .. } => ExprTag::Dot,
      ExprKind::Index { .. } => ExprTag::Index,
      ExprKind::Call(_) => ExprTag::Call,
      ExprKind::New(_) => ExprTag::New,
      ExprKind::Unary { .. } => ExprTag::Unary,
      ExprKind::Binary { .. } => ExprTag::Binary,
      ExprKind::Sequence(_) => ExprTag::Sequence,
      ExprKind::Assign { .. } => ExprTag::Assign,
      ExprKind::Cond { .. } => ExprTag::Cond,
      ExprKind::Spread(_) => ExprTag::Spread,
      ExprKind::Await(_) => ExprTag::Await,
      ExprKind::Yield { .. } => ExprTag::Yield,
      ExprKind::As { .. } => ExprTag::As,
      ExprKind::Satisfies { .. } => ExprTag::Satisfies,
      ExprKind::AsConst(_) => ExprTag::AsConst,
      ExprKind::NonNull(_) => ExprTag::NonNull,
      ExprKind::Instantiation { .. } => ExprTag::Instantiation,
      ExprKind::Jsx(_) => ExprTag::Jsx,
      ExprKind::ImportCall { .. } => ExprTag::ImportCall,
      ExprKind::ImportMeta => ExprTag::ImportMeta,
      ExprKind::NewTarget => ExprTag::NewTarget,
    }
  }

  pub(crate) fn parent(self) -> Node<'a> {
    Node::Expr(self).parent()
  }

  /// Text between a JSX element's tags. Its kind is `String`.
  pub(crate) fn is_jsx_text(self) -> bool {
    matches!(self.file.kind(self.id), Some(AstKind::JSXText(_)))
  }

  /// The parentheses around it, innermost first.
  pub(crate) fn parens(self) -> impl DoubleEndedIterator<Item = Span> + ExactSizeIterator + 'a {
    self.file.parens(self.id).iter().map(|it| it.1)
  }

  pub(crate) fn is_parenthesized(self) -> bool {
    !self.file.parens(self.id).is_empty()
  }

  /// Includes all the parentheses around it.
  pub(crate) fn outer_span(self) -> Span {
    self.parens().next_back().unwrap_or_else(|| self.span())
  }

  pub(crate) fn is_missing(self) -> bool {
    self.tag() == ExprTag::Missing
  }

  pub(crate) fn as_ident(self) -> Option<Name<'a>> {
    match self.kind() {
      ExprKind::Ident(name) => Some(name),
      _ => None,
    }
  }

  pub(crate) fn as_call(self) -> Option<Call<'a>> {
    match self.kind() {
      ExprKind::Call(call) => Some(call),
      _ => None,
    }
  }

  pub(crate) fn as_fn(self) -> Option<Func<'a>> {
    match self.kind() {
      ExprKind::Fn(func) => Some(func),
      _ => None,
    }
  }

  pub(crate) fn as_class(self) -> Option<Class<'a>> {
    match self.kind() {
      ExprKind::Class(class) => Some(class),
      _ => None,
    }
  }

  pub(crate) fn as_dot(self) -> Option<(Expr<'a>, Ident<'a>, Chain)> {
    match self.kind() {
      ExprKind::Dot { obj, name, chain } => Some((obj, name, chain)),
      _ => None,
    }
  }

  pub(crate) fn as_index(self) -> Option<(Expr<'a>, Expr<'a>, Chain)> {
    match self.kind() {
      ExprKind::Index { obj, index, chain } => Some((obj, index, chain)),
      _ => None,
    }
  }

  pub(crate) fn as_unary(self) -> Option<(UnOp, Expr<'a>)> {
    match self.kind() {
      ExprKind::Unary { op, operand } => Some((op, operand)),
      _ => None,
    }
  }

  pub(crate) fn as_binary(self) -> Option<(BinOp, Expr<'a>, Expr<'a>)> {
    match self.kind() {
      ExprKind::Binary { op, left, right } => Some((op, left, right)),
      _ => None,
    }
  }

  pub(crate) fn as_assign(self) -> Option<(Option<BinOp>, Expr<'a>, Expr<'a>)> {
    match self.kind() {
      ExprKind::Assign { op, target, value } => Some((op, target, value)),
      _ => None,
    }
  }

  pub(crate) fn as_object(self) -> Option<List<'a, Prop<'a>>> {
    match self.kind() {
      ExprKind::Object(props) => Some(props),
      _ => None,
    }
  }

  /// The import, export or import type whose attributes are this `{ .. }`: the `with` clause of a declaration, or the
  /// `{ .. }` of `with: { .. }` in an import type.
  pub(crate) fn owner_of_import_attributes(self) -> Option<Node<'a>> {
    let file = self.file;
    let parent = file.parent_id(self.id);
    if let Some(AstKind::WithClause(_)) = file.kind(self.id) {
      return Some(Node::Stmt(Stmt::new(file, parent)));
    }
    let ty = TypeNode::new(file, file.parent_id(file.parent_id(parent)));
    ty.import_attributes().filter(|it| it.object() == self).map(|_| Node::Type(ty))
  }

  /// The call, if it's a `Call` or a `New`.
  pub(crate) fn as_call_or_new(self) -> Option<Call<'a>> {
    match self.kind() {
      ExprKind::Call(call) | ExprKind::New(call) => Some(call),
      _ => None,
    }
  }

  /// The call, if it's a `Call`, a `New` or a `TaggedTemplate`.
  pub(crate) fn as_call_like(self) -> Option<Call<'a>> {
    match self.kind() {
      ExprKind::Call(call) | ExprKind::New(call) | ExprKind::TaggedTemplate(call) => Some(call),
      _ => None,
    }
  }

  /// The callee of a `Call` or a `New`.
  pub(crate) fn callee(self) -> Option<Expr<'a>> {
    self.as_call_or_new().map(Call::callee)
  }

  /// The `obj` of a `Dot` or an `Index`.
  pub(crate) fn object(self) -> Option<Expr<'a>> {
    match self.kind() {
      ExprKind::Dot { obj, .. } | ExprKind::Index { obj, .. } => Some(obj),
      _ => None,
    }
  }

  pub(crate) fn index(self) -> Option<Expr<'a>> {
    self.as_index().map(|it| it.1)
  }

  pub(crate) fn member_name(self) -> Option<Ident<'a>> {
    self.as_dot().map(|it| it.1)
  }

  pub(crate) fn member_name_start(self) -> Option<u32> {
    self.member_name().map(Ident::start)
  }

  /// `obj.#name`
  pub(crate) fn is_private_member(self) -> bool {
    matches!(self.file.kind(self.id), Some(AstKind::PrivateFieldExpression(_)))
  }

  pub(crate) fn binary_op(self) -> Option<BinOp> {
    self.as_binary().map(|it| it.0)
  }

  pub(crate) fn unary_op(self) -> Option<UnOp> {
    self.as_unary().map(|it| it.0)
  }

  /// The operator of an `Assign`. `Some(None)` for plain `=`.
  pub(crate) fn assign_op(self) -> Option<Option<BinOp>> {
    self.as_assign().map(|it| it.0)
  }

  /// The `left` of a `Binary`, the `target` of an `Assign`.
  pub(crate) fn left(self) -> Option<Expr<'a>> {
    match self.kind() {
      ExprKind::Binary { left, .. } | ExprKind::Assign { target: left, .. } => Some(left),
      _ => None,
    }
  }

  /// The `right` of a `Binary`, the `value` of an `Assign`.
  pub(crate) fn right(self) -> Option<Expr<'a>> {
    match self.kind() {
      ExprKind::Binary { right, .. } | ExprKind::Assign { value: right, .. } => Some(right),
      _ => None,
    }
  }

  /// The single operand of a `Unary`, an `Await`, a `Spread`, a `NonNull`, an `As`, an `AsConst`, a `Satisfies` or an
  /// `Instantiation`.
  pub(crate) fn operand(self) -> Option<Expr<'a>> {
    match self.kind() {
      ExprKind::Unary { operand, .. }
      | ExprKind::Await(operand)
      | ExprKind::Spread(operand)
      | ExprKind::NonNull(operand)
      | ExprKind::AsConst(operand)
      | ExprKind::As { expr: operand, .. }
      | ExprKind::Satisfies { expr: operand, .. }
      | ExprKind::Instantiation { expr: operand, .. } => Some(operand),
      _ => None,
    }
  }

  /// `<T>e` or `<const>e`, as opposed to `e as T`.
  pub(crate) fn is_angle_bracket_assertion(self) -> bool {
    matches!(self.file.kind(self.id), Some(AstKind::TSTypeAssertion(_)))
  }

  /// `a?.b`, `a?.[b]`, `a?.()`: this expression itself has the `?.`.
  pub(crate) fn is_optional(self) -> bool {
    self.chain() == Chain::Start
  }

  /// Its position in an optional chain: it has a `?.`, or a `?.` to its left, not across parentheses, has one.
  pub(crate) fn chain(self) -> Chain {
    let known = |e: Expr<'a>| match e.file.chains.get(e.id as usize).map(std::cell::Cell::get) {
      Some(1) => Some(Chain::No),
      Some(2) => Some(Chain::Start),
      Some(3) => Some(Chain::Continue),
      _ => None,
    };
    if let Some(chain) = known(self) {
      return chain;
    }
    // Down the chain to a link whose position is known or doesn't depend on its operand, then back up.
    let mut links: smallvec::SmallVec<[Expr<'a>; 8]> = smallvec::smallvec![self];
    let mut chain = loop {
      let at = *links.last().unwrap_or(&self);
      if at.has_question_dot() {
        break Chain::Start;
      }
      match at.chain_operand() {
        None => break Chain::No,
        Some(inner) if inner.is_parenthesized() => break Chain::No,
        Some(inner) => match known(inner) {
          Some(Chain::No) => break Chain::No,
          Some(_) => break Chain::Continue,
          None => links.push(inner),
        },
      }
    };
    // The links above the last one have no `?.` and no parentheses around their operand, so each continues the chain
    // of the link below it, if that is in one.
    while let Some(link) = links.pop() {
      if let Some(memo) = link.file.chains.get(link.id as usize) {
        memo.set(chain as u8 + 1);
      }
      if chain == Chain::Start {
        chain = Chain::Continue;
      }
    }
    known(self).unwrap_or(Chain::No)
  }

  fn has_question_dot(self) -> bool {
    match self.file.kind(self.id) {
      Some(AstKind::StaticMemberExpression(it)) => it.optional,
      Some(AstKind::ComputedMemberExpression(it)) => it.optional,
      Some(AstKind::PrivateFieldExpression(it)) => it.optional,
      Some(AstKind::CallExpression(it)) => it.optional,
      _ => false,
    }
  }

  /// The object of a member, the callee of a call, the operand of a `!`: what an optional chain continues from.
  fn chain_operand(self) -> Option<Expr<'a>> {
    match self.file.kind(self.id)? {
      AstKind::StaticMemberExpression(it) => Some(self.of(&it.object)),
      AstKind::ComputedMemberExpression(it) => Some(self.of(&it.object)),
      AstKind::PrivateFieldExpression(it) => Some(self.of(&it.object)),
      AstKind::CallExpression(it) => Some(self.of(&it.callee)),
      AstKind::TSNonNullExpression(it) => Some(self.of(&it.expression)),
      _ => None,
    }
  }

  /// The span of the JSX `{e}` that wraps it, or of the `{...e}` that it is.
  pub(crate) fn jsx_container_span(self) -> Option<Span> {
    if let Some(AstKind::JSXSpreadChild(it)) = self.file.kind(self.id) {
      return Some(span(it.span));
    }
    match self.file.kind(self.file.parent_id(self.id))? {
      AstKind::JSXExpressionContainer(it) => Some(span(it.span)),
      _ => None,
    }
  }

  /// True if it's part of an optional chain, meaning it isn't evaluated when its own `?.` or one to its left finds
  /// `null` or `undefined`. In `(a?.b).c` it's only `a?.b`, because parentheses end a chain.
  pub(crate) fn is_in_optional_chain(self) -> bool {
    let mut at = self;
    while let ExprKind::NonNull(operand) = at.kind() {
      at = operand;
      if at.is_parenthesized() {
        return false;
      }
    }
    at.chain() != Chain::No
  }

  /// True if it's a whole optional chain, which oxc wraps in a `ChainExpression`.
  pub(crate) fn is_chain_root(self) -> bool {
    self.is_in_optional_chain() && self.ends_optional_chain()
  }

  fn ends_optional_chain(self) -> bool {
    let Node::Expr(parent) = self.parent() else { return true };
    let continues = match parent.tag() {
      ExprTag::Dot | ExprTag::Index => parent.object() == Some(self),
      ExprTag::Call => parent.callee() == Some(self),
      ExprTag::NonNull => true,
      _ => false,
    };
    !continues || self.is_parenthesized()
  }

  /// True if it's a destructuring assignment target or part of one.
  pub(crate) fn is_assignment_target(self) -> bool {
    let mut at = Node::Expr(self);
    loop {
      if let Some(
        AstKind::ArrayAssignmentTarget(_)
        | AstKind::ObjectAssignmentTarget(_)
        | AstKind::AssignmentTargetWithDefault(_)
        | AstKind::AssignmentTargetRest(_)
        | AstKind::AssignmentTargetPropertyIdentifier(_)
        | AstKind::AssignmentTargetPropertyProperty(_),
      ) = at.file().kind(at.id())
      {
        return true;
      }
      if matches!(at, Node::Expr(e) if e.is_parenthesized()) {
        return false;
      }
      match at.parent() {
        parent @ (Node::Expr(_) | Node::Prop(_)) => at = parent,
        _ => return false,
      }
    }
  }

  /// bun's ranges of the `NonNull`s inside one `x!!`. Always empty: here each `!` is a `NonNull` of its own.
  pub(crate) fn inner_non_null_spans(
    self,
  ) -> impl DoubleEndedIterator<Item = Span> + ExactSizeIterator + 'a {
    std::iter::empty()
  }

  /// The number of `!` in a chain of `NonNull`s, so 2 for `x!!`. 0 for anything else.
  pub(crate) fn non_null_count(self) -> usize {
    let mut count = 0;
    let mut at = self;
    while let ExprKind::NonNull(operand) = at.kind() {
      count += 1;
      if operand.is_parenthesized() {
        break;
      }
      at = operand;
    }
    count
  }

  /// `"defer"` for `import.defer(..)`, `"source"` for `import.source(..)`.
  pub(crate) fn import_call_phase(self) -> Option<&'static str> {
    match self.file.kind(self.id)? {
      AstKind::ImportExpression(it) => it.phase.map(|phase| phase.as_str()),
      _ => None,
    }
  }

  /// The comma operator's operands: `a`, `b` and `c` for `a, b, c`. Anything else gives just itself.
  pub(crate) fn sequence(self) -> smallvec::SmallVec<[Expr<'a>; 4]> {
    match self.kind() {
      ExprKind::Sequence(expressions) => expressions.iter().collect(),
      _ => smallvec::smallvec![self],
    }
  }
}

fn span(span: oxc_span::Span) -> Span {
  Span::new(span.start, span.end)
}

pub(crate) fn is_const(ty: &TSType) -> bool {
  matches!(ty, TSType::TSTypeReference(it) if it.type_arguments.is_none()
    && matches!(&it.type_name, oxc_ast::ast::TSTypeName::IdentifierReference(name) if name.name == "const"))
}

/// The id of the expression that an assignment target is.
pub(crate) fn target_id(target: &AssignmentTarget) -> u32 {
  match target.as_simple_assignment_target() {
    Some(simple) => simple_target_id(simple),
    None => target.node_id().index() as u32,
  }
}

fn simple_target_id(target: &SimpleAssignmentTarget) -> u32 {
  match target {
    SimpleAssignmentTarget::TSAsExpression(it) => it.node_id.get().index() as u32,
    SimpleAssignmentTarget::TSSatisfiesExpression(it) => it.node_id.get().index() as u32,
    SimpleAssignmentTarget::TSNonNullExpression(it) => it.node_id.get().index() as u32,
    SimpleAssignmentTarget::TSTypeAssertion(it) => it.node_id.get().index() as u32,
    _ => match target.as_member_expression() {
      Some(member) => member_id(member),
      None => target.node_id().index() as u32,
    },
  }
}

fn member_id(member: &MemberExpression) -> u32 {
  member.node_id().index() as u32
}

/// The tag name of a JSX element as an id.
pub(crate) fn element_name_id(name: &JSXElementName) -> u32 {
  match name {
    JSXElementName::Identifier(it) => it.node_id.get().index() as u32,
    JSXElementName::IdentifierReference(it) => it.node_id.get().index() as u32,
    JSXElementName::NamespacedName(it) => it.node_id.get().index() as u32,
    JSXElementName::MemberExpression(it) => it.node_id.get().index() as u32,
    JSXElementName::ThisExpression(it) => it.node_id.get().index() as u32,
  }
}

fn binary(op: BinaryOperator) -> BinOp {
  match op {
    BinaryOperator::Equality => BinOp::EqEq,
    BinaryOperator::Inequality => BinOp::NotEq,
    BinaryOperator::StrictEquality => BinOp::EqEqEq,
    BinaryOperator::StrictInequality => BinOp::NotEqEq,
    BinaryOperator::LessThan => BinOp::Lt,
    BinaryOperator::LessEqualThan => BinOp::Le,
    BinaryOperator::GreaterThan => BinOp::Gt,
    BinaryOperator::GreaterEqualThan => BinOp::Ge,
    BinaryOperator::Addition => BinOp::Add,
    BinaryOperator::Subtraction => BinOp::Sub,
    BinaryOperator::Multiplication => BinOp::Mul,
    BinaryOperator::Division => BinOp::Div,
    BinaryOperator::Remainder => BinOp::Rem,
    BinaryOperator::Exponential => BinOp::Pow,
    BinaryOperator::ShiftLeft => BinOp::Shl,
    BinaryOperator::ShiftRight => BinOp::Shr,
    BinaryOperator::ShiftRightZeroFill => BinOp::UShr,
    BinaryOperator::BitwiseOR => BinOp::BitOr,
    BinaryOperator::BitwiseXOR => BinOp::BitXor,
    BinaryOperator::BitwiseAnd => BinOp::BitAnd,
    BinaryOperator::In => BinOp::In,
    BinaryOperator::Instanceof => BinOp::Instanceof,
  }
}

fn assignment(op: AssignmentOperator) -> Option<BinOp> {
  Some(match op {
    AssignmentOperator::Assign => return None,
    AssignmentOperator::Addition => BinOp::Add,
    AssignmentOperator::Subtraction => BinOp::Sub,
    AssignmentOperator::Multiplication => BinOp::Mul,
    AssignmentOperator::Division => BinOp::Div,
    AssignmentOperator::Remainder => BinOp::Rem,
    AssignmentOperator::Exponential => BinOp::Pow,
    AssignmentOperator::ShiftLeft => BinOp::Shl,
    AssignmentOperator::ShiftRight => BinOp::Shr,
    AssignmentOperator::ShiftRightZeroFill => BinOp::UShr,
    AssignmentOperator::BitwiseOR => BinOp::BitOr,
    AssignmentOperator::BitwiseXOR => BinOp::BitXor,
    AssignmentOperator::BitwiseAnd => BinOp::BitAnd,
    AssignmentOperator::LogicalAnd => BinOp::And,
    AssignmentOperator::LogicalOr => BinOp::Or,
    AssignmentOperator::LogicalNullish => BinOp::Nullish,
  })
}

/// The operator's source text.
pub(crate) fn bin_op_text(op: BinOp) -> &'static str {
  match op {
    BinOp::Add => "+",
    BinOp::Sub => "-",
    BinOp::Mul => "*",
    BinOp::Div => "/",
    BinOp::Rem => "%",
    BinOp::Pow => "**",
    BinOp::Shl => "<<",
    BinOp::Shr => ">>",
    BinOp::UShr => ">>>",
    BinOp::BitAnd => "&",
    BinOp::BitOr => "|",
    BinOp::BitXor => "^",
    BinOp::Lt => "<",
    BinOp::Le => "<=",
    BinOp::Gt => ">",
    BinOp::Ge => ">=",
    BinOp::EqEq => "==",
    BinOp::NotEq => "!=",
    BinOp::EqEqEq => "===",
    BinOp::NotEqEq => "!==",
    BinOp::In => "in",
    BinOp::Instanceof => "instanceof",
    BinOp::And => "&&",
    BinOp::Or => "||",
    BinOp::Nullish => "??",
    BinOp::Comma => ",",
  }
}

/// The operator's source text.
pub(crate) fn un_op_text(op: UnOp) -> &'static str {
  match op {
    UnOp::Plus => "+",
    UnOp::Minus => "-",
    UnOp::BitNot => "~",
    UnOp::Not => "!",
    UnOp::Typeof => "typeof",
    UnOp::Void => "void",
    UnOp::Delete => "delete",
    UnOp::PreInc | UnOp::PostInc => "++",
    UnOp::PreDec | UnOp::PostDec => "--",
  }
}

/// `=`, `+=`, `&&=`, ..
pub(crate) fn assign_op_text(op: Option<BinOp>) -> &'static str {
  match op {
    None => "=",
    Some(BinOp::Add) => "+=",
    Some(BinOp::Sub) => "-=",
    Some(BinOp::Mul) => "*=",
    Some(BinOp::Div) => "/=",
    Some(BinOp::Rem) => "%=",
    Some(BinOp::Pow) => "**=",
    Some(BinOp::Shl) => "<<=",
    Some(BinOp::Shr) => ">>=",
    Some(BinOp::UShr) => ">>>=",
    Some(BinOp::BitAnd) => "&=",
    Some(BinOp::BitOr) => "|=",
    Some(BinOp::BitXor) => "^=",
    Some(BinOp::And) => "&&=",
    Some(BinOp::Or) => "||=",
    Some(BinOp::Nullish) => "??=",
    Some(_) => "=",
  }
}

/// The offset of the first token at or after `at`: white space and comments are skipped.
pub(crate) fn skip_trivia(text: &[u8], mut at: u32) -> u32 {
  loop {
    let rest = text.get(at as usize..).unwrap_or_default();
    match rest {
      [b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c', ..] => at += 1,
      [b'/', b'/', ..] => {
        at += crate::strings::index_of_any(rest, b"\r\n").unwrap_or(rest.len()) as u32;
      }
      [b'/', b'*', ..] => match crate::strings::index_of(&rest[2..], b"*/") {
        Some(end) => at += end as u32 + 4,
        None => return text.len() as u32,
      },
      _ if crate::strings::js_whitespace_len(rest) > 0 => {
        at += crate::strings::js_whitespace_len(rest) as u32;
      }
      _ => return at,
    }
  }
}

/// `/pattern/flags`
#[derive(Copy, Clone, Debug)]
pub(crate) struct Regex<'a> {
  expr: Expr<'a>,
}

impl<'a> Regex<'a> {
  pub(crate) fn pattern(self) -> &'a [u8] {
    match self.expr.file.kind(self.expr.id) {
      Some(AstKind::RegExpLiteral(it)) => it.regex.pattern.text.as_bytes(),
      _ => b"",
    }
  }

  pub(crate) fn flags(self) -> &'a [u8] {
    let text = self.expr.text();
    let slash = text.iter().rposition(|&byte| byte == b'/').map_or(text.len(), |at| at + 1);
    &text[slash..]
  }
}

/// `` `a${b}c` ``
#[derive(Copy, Clone, Debug)]
pub(crate) struct Template<'a> {
  expr: Expr<'a>,
}

impl<'a> Template<'a> {
  fn raw_template(self) -> Option<&'a oxc_ast::ast::TemplateLiteral<'a>> {
    match self.expr.file.kind(self.expr.id)? {
      AstKind::TemplateLiteral(it) => Some(it),
      _ => None,
    }
  }

  /// The substitutions.
  pub(crate) fn exprs(self) -> List<'a, Expr<'a>> {
    match self.raw_template() {
      Some(it) => {
        List::run(self.expr.file, it.expressions.first().map(expression_id), it.expressions.len())
      }
      None => List::empty(self.expr.file),
    }
  }

  /// The number of text pieces, which is one more than the number of substitutions.
  pub(crate) fn quasi_count(self) -> usize {
    self.raw_template().map_or(1, |it| it.quasis.len())
  }

  /// The cooked value of text piece `i`. `None` if it has an invalid escape, which a tagged template allows.
  pub(crate) fn cooked(self, i: usize) -> Option<Name<'a>> {
    let quasi = self.raw_template()?.quasis.get(i)?;
    quasi.value.cooked.and_then(|cooked| cooked.as_str()).map(Name)
  }

  /// Text piece `i` with its delimiters: `` `a${ ``, `}b${`, `` }c` ``.
  pub(crate) fn quasi_span(self, i: usize) -> Span {
    let Some(quasi) = self.raw_template().and_then(|it| it.quasis.get(i)) else {
      return Span::default();
    };
    let end = if quasi.tail { quasi.span.end + 1 } else { quasi.span.end + 2 };
    Span::new(quasi.span.start - 1, end)
  }

  /// The source text of piece `i`, without its delimiters.
  pub(crate) fn raw(self, i: usize) -> &'a [u8] {
    match self.raw_template().and_then(|it| it.quasis.get(i)) {
      Some(quasi) => quasi.value.raw.as_str().as_bytes(),
      None => b"",
    }
  }
}

/// `callee(args)`, `new callee(args)`, `` tag`template` ``
#[derive(Copy, Clone, Debug)]
pub(crate) struct Call<'a> {
  expr: Expr<'a>,
}

impl<'a> Call<'a> {
  pub(crate) fn callee(self) -> Expr<'a> {
    let file = self.expr.file;
    let id = match file.kind(self.expr.id) {
      Some(AstKind::CallExpression(it)) => expression_id(&it.callee),
      Some(AstKind::NewExpression(it)) => expression_id(&it.callee),
      Some(AstKind::TaggedTemplateExpression(it)) => expression_id(&it.tag),
      _ => super::NONE,
    };
    Expr::new(file, id)
  }

  pub(crate) fn args(self) -> List<'a, Expr<'a>> {
    let file = self.expr.file;
    match file.kind(self.expr.id) {
      Some(AstKind::CallExpression(it)) => {
        List::run(file, it.arguments.first().map(argument_id), it.arguments.len())
      }
      Some(AstKind::NewExpression(it)) => {
        List::run(file, it.arguments.first().map(argument_id), it.arguments.len())
      }
      Some(AstKind::TaggedTemplateExpression(it)) => {
        let exprs = &it.quasi.expressions;
        List::run(file, exprs.first().map(expression_id), exprs.len())
      }
      _ => List::empty(file),
    }
  }

  pub(crate) fn type_args(self) -> List<'a, TypeNode<'a>> {
    let file = self.expr.file;
    let args = match file.kind(self.expr.id) {
      Some(AstKind::CallExpression(it)) => it.type_arguments.as_deref(),
      Some(AstKind::NewExpression(it)) => it.type_arguments.as_deref(),
      Some(AstKind::TaggedTemplateExpression(it)) => it.type_arguments.as_deref(),
      _ => None,
    };
    match args {
      Some(args) => List::run(file, args.params.first().map(type_id), args.params.len()),
      None => List::empty(file),
    }
  }

  pub(crate) fn chain(self) -> Chain {
    self.expr.chain()
  }

  /// `callee?.(args)`
  pub(crate) fn is_optional(self) -> bool {
    self.chain() == Chain::Start
  }

  /// The position of the `)`. `None` for `new C` and for a tagged template.
  pub(crate) fn close_paren(self) -> Option<u32> {
    let text = self.expr.file.text.as_bytes();
    let end = self.expr.span().end;
    match self.expr.file.kind(self.expr.id)? {
      AstKind::CallExpression(_) => Some(end - 1),
      AstKind::NewExpression(_) if text.get(end as usize - 1) == Some(&b')') => Some(end - 1),
      _ => None,
    }
  }

  /// The template of a tagged template.
  pub(crate) fn template(self) -> Option<Expr<'a>> {
    match self.expr.file.kind(self.expr.id)? {
      AstKind::TaggedTemplateExpression(it) => {
        Some(Expr::new(self.expr.file, it.quasi.node_id.get().index() as u32))
      }
      _ => None,
    }
  }
}

/// A child between a JSX element's tags.
#[derive(Copy, Clone, Debug)]
pub(crate) enum JsxChild<'a> {
  Expr(Expr<'a>),
  /// Whitespace containing a line break.
  Whitespace(Span),
}

/// See [`Jsx::children_with_whitespace`].
#[derive(Copy, Clone)]
pub(crate) struct JsxChildren<'a> {
  /// The end of what's been returned so far.
  at: u32,
  /// The start of the closing tag.
  end: u32,
  children: super::Iter<'a, Expr<'a>>,
  /// The child that follows the whitespace just returned.
  pending: Option<Expr<'a>>,
}

impl<'a> Iterator for JsxChildren<'a> {
  type Item = JsxChild<'a>;

  fn next(&mut self) -> Option<JsxChild<'a>> {
    if let Some(child) = self.pending.take() {
      return Some(JsxChild::Expr(child));
    }
    let Some(child) = self.children.next() else {
      let rest = Span::new(self.at, self.end);
      self.at = self.end;
      return (!rest.is_empty()).then_some(JsxChild::Whitespace(rest));
    };
    let span = child.jsx_container_span().unwrap_or_else(|| child.span());
    let before = Span::before(self.at, span);
    self.at = span.end;
    if before.is_empty() {
      return Some(JsxChild::Expr(child));
    }
    self.pending = Some(child);
    Some(JsxChild::Whitespace(before))
  }
}

/// `<tag attrs>children</tag>`, `<tag attrs />`, `<>children</>`
#[derive(Copy, Clone, Debug)]
pub(crate) struct Jsx<'a> {
  expr: Expr<'a>,
}

impl<'a> Jsx<'a> {
  fn element(self) -> Option<&'a oxc_ast::ast::JSXElement<'a>> {
    match self.expr.file.kind(self.expr.id)? {
      AstKind::JSXElement(it) => Some(it),
      _ => None,
    }
  }

  /// The name in the opening tag: an `Ident`, a `Dot`, `This`, or a `String` for `a-b` and `a:b`. `None` for a
  /// fragment.
  pub(crate) fn tag(self) -> Option<Expr<'a>> {
    let element = self.element()?;
    Some(Expr::new(self.expr.file, element_name_id(&element.opening_element.name)))
  }

  /// The name in the closing tag.
  pub(crate) fn close_tag(self) -> Option<Expr<'a>> {
    let closing = self.element()?.closing_element.as_ref()?;
    Some(Expr::new(self.expr.file, element_name_id(&closing.name)))
  }

  pub(crate) fn is_fragment(self) -> bool {
    self.element().is_none()
  }

  pub(crate) fn is_self_closing(self) -> bool {
    self.element().is_some_and(|it| it.closing_element.is_none())
  }

  /// `name="value"`, `name={value}`, `name`, `{...value}`
  pub(crate) fn attrs(self) -> List<'a, Prop<'a>> {
    let file = self.expr.file;
    match self.element() {
      Some(element) => {
        let attributes = &element.opening_element.attributes;
        List::run(file, attributes.first().map(|it| it.node_id().index() as u32), attributes.len())
      }
      None => List::empty(file),
    }
  }

  /// Text is a `String` ([`Expr::is_jsx_text`]). A child in braces has a [`Expr::jsx_container_span`]: `{}` is
  /// `Missing`, `{...e}` is a `Spread`. Whitespace-only text containing a line break is skipped.
  pub(crate) fn children(self) -> List<'a, Expr<'a>> {
    let file = self.expr.file;
    let children = match file.kind(self.expr.id) {
      Some(AstKind::JSXElement(it)) => &it.children,
      Some(AstKind::JSXFragment(it)) => &it.children,
      _ => return List::empty(file),
    };
    let first = children.first().map(|it| it.node_id().index() as u32);
    List::run(file, first, children.len()).without(|file, id| match file.kind(id) {
      Some(AstKind::JSXText(text)) => {
        text.value.trim_matches([' ', '\t', '\n', '\r']).is_empty()
          && text.value.contains(['\n', '\r'])
      }
      _ => false,
    })
  }

  /// The children plus the whitespace between them that [`Jsx::children`] skips.
  pub(crate) fn children_with_whitespace(self) -> JsxChildren<'a> {
    let at = self.opening_span().end;
    JsxChildren {
      at,
      end: self.closing_span().map_or(at, |it| it.start),
      children: self.children().iter(),
      pending: None,
    }
  }

  pub(crate) fn type_args(self) -> List<'a, TypeNode<'a>> {
    let file = self.expr.file;
    match self.element().and_then(|it| it.opening_element.type_arguments.as_deref()) {
      Some(args) => List::run(file, args.params.first().map(type_id), args.params.len()),
      None => List::empty(file),
    }
  }

  /// `<tag attrs>`, `<tag attrs />`, `<>`
  pub(crate) fn opening_span(self) -> Span {
    match self.expr.file.kind(self.expr.id) {
      Some(AstKind::JSXElement(it)) => span(it.opening_element.span),
      Some(AstKind::JSXFragment(it)) => span(it.opening_fragment.span),
      _ => Span::default(),
    }
  }

  /// `</tag>`, `</>`
  pub(crate) fn closing_span(self) -> Option<Span> {
    match self.expr.file.kind(self.expr.id)? {
      AstKind::JSXElement(it) => it.closing_element.as_ref().map(|it| span(it.span)),
      AstKind::JSXFragment(it) => Some(span(it.closing_fragment.span)),
      _ => None,
    }
  }
}

impl<'a> Func<'a> {}
