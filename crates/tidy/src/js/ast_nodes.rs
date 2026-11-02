//! The syntax tree as oxc and Prettier see it, for questions about a node's surroundings.
//!
//! Much of what decides how a node is printed depends on its parent: does it need parentheses there, does it start a
//! statement, is it the callee of a call? The printer asks those questions in terms of oxc's nodes, which are ESTree's
//! with a few more (`FunctionBody`, `FormalParameters`), and which `js::ast`'s handles don't all have.
//!
//! [`AstNodes`] is that tree: an oxc node, with the handle that the printer formats it from. A value is a kind plus a
//! handle, and [`AstNodes::parent`] computes the next one from the node tables. Every variant has exactly one field, so
//! `AstNodes::X(_)` is always a valid pattern. Ported from bun's `src/format/js/ast_nodes.rs` (MIT), which computes
//! the same over bun's tree.
//!
//! - `expr.as_ast_nodes()`, `stmt.as_ast_nodes()`, ..: the node for a handle.
//! - `expr.ast_parent()`, ..: its parent. Same as `expr.as_ast_nodes().parent()`.
//! - `expr.as_chain_element()`: `a?.b` is two nodes, a `ChainExpression` and the `MemberExpression` inside it, but one
//!   `Expr`. `as_ast_nodes()` returns the outer node and this returns the inner one.
//! - An arrow function with an expression body has a `FunctionBody` and an `ExpressionStatement` around the expression,
//!   as in oxc's formatter, though oxc's parser has neither.
//! - `nodes.span()`: the node's range in oxc.
//! - `nodes.ancestors()`: the node itself, its parent, and so on up to `Program`.

use crate::js::ast::{
  Case, Class, EnumMember, ExportSpec, Expr, File, FnBody, Func, ImportSpec, Member, Node, Param,
  Pat, PatProp, Prop, Span, Spanned, Stmt, StmtKind, TupleElem, TypeNode, TypeParam, VarDecl,
  expression_id,
};
use oxc_ast::AstKind as K;
use oxc_ast::ast::{
  ArrowFunctionBody, Declaration, ExportDefaultDeclarationKind, JSXExpression,
  TSNamespaceDeclarationBody,
};

/// An expression statement, or an arrow function's expression body, which oxc's formatter represents as an
/// `ExpressionStatement` in a `FunctionBody`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum ExpressionStatement<'a> {
  Stmt(Stmt<'a>),
  ArrowBody(Func<'a>),
}

impl<'a> ExpressionStatement<'a> {
  /// `() => expression`
  pub(crate) fn is_arrow_function_body(self) -> bool {
    matches!(self, ExpressionStatement::ArrowBody(_))
  }

  pub(crate) fn expression(self) -> Option<Expr<'a>> {
    match self {
      ExpressionStatement::Stmt(statement) => match statement.kind() {
        StmtKind::Expr(e) => Some(e),
        _ => None,
      },
      ExpressionStatement::ArrowBody(func) => match func.body() {
        FnBody::Expr(e) => Some(e),
        _ => None,
      },
    }
  }

  pub(crate) fn span(self) -> Span {
    match self {
      ExpressionStatement::Stmt(statement) => statement.span(),
      ExpressionStatement::ArrowBody(_) => self.expression().map_or_else(Span::default, Expr::span),
    }
  }
}

impl<'a> From<ExpressionStatement<'a>> for Node<'a> {
  fn from(statement: ExpressionStatement<'a>) -> Node<'a> {
    match statement {
      ExpressionStatement::Stmt(statement) => Node::Stmt(statement),
      ExpressionStatement::ArrowBody(func) => Node::Func(func),
    }
  }
}

macro_rules! ast_nodes {
  ($($payload:ty { $($variant:ident)* })*) => {
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    pub(crate) enum AstNodes<'a> {
      Program(Program<'a>),
      $($($variant($payload),)*)*
    }

    /// The variant of an [`AstNodes`].
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    #[repr(u8)]
    pub(crate) enum AstKind {
      Program,
      $($($variant,)*)*
    }

    /// The [`AstKind`] variant names, indexed by discriminant.
    pub(crate) const AST_KIND_NAMES: &[&str] = &["Program", $($(stringify!($variant),)*)*];

    impl<'a> AstNodes<'a> {
      pub(crate) fn kind(self) -> AstKind {
        match self {
          AstNodes::Program(_) => AstKind::Program,
          $($(AstNodes::$variant(_) => AstKind::$variant,)*)*
        }
      }

      /// Returns the underlying `js::ast` node.
      pub(crate) fn raw(self) -> Node<'a> {
        match self {
          AstNodes::Program(file) => Node::File(file.0),
          $($(AstNodes::$variant(it) => Node::from(it),)*)*
        }
      }
    }
  };
}

#[derive(Copy, Clone)]
pub(crate) struct Program<'a>(pub(crate) &'a File<'a>);

impl PartialEq for Program<'_> {
  fn eq(&self, other: &Self) -> bool {
    std::ptr::eq(self.0, other.0)
  }
}

impl Eq for Program<'_> {}

impl std::fmt::Debug for Program<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("Program")
  }
}

ast_nodes! {
  Expr<'a> {
    IdentifierReference BooleanLiteral NullLiteral NumericLiteral BigIntLiteral RegExpLiteral
    StringLiteral TemplateLiteral ThisExpression Super ArrayExpression ObjectExpression
    StaticMemberExpression ComputedMemberExpression PrivateFieldExpression CallExpression
    NewExpression ImportExpression MetaProperty TaggedTemplateExpression UnaryExpression
    UpdateExpression BinaryExpression LogicalExpression PrivateInExpression
    AssignmentExpression ConditionalExpression AwaitExpression YieldExpression TSAsExpression
    TSSatisfiesExpression TSTypeAssertion TSNonNullExpression TSInstantiationExpression
    JSXElement JSXFragment JSXText JSXEmptyExpression Elision PrivateIdentifier
    // All of `a, b, c`.
    SequenceExpression
    // All of `a?.b.c`. The field is also the node inside it.
    ChainExpression
    // The `{e}` around `e`.
    JSXExpressionContainer
    // `{...e}` as a child of an element. The field is the `Spread`.
    JSXSpreadChild
    // `<a b>`. The field is the element.
    JSXOpeningElement
    // The `@e` around `e`.
    Decorator
    // An array literal, an object literal, `a = 1` and `...a` inside an assignment target.
    ArrayAssignmentTarget ObjectAssignmentTarget AssignmentTargetWithDefault
    AssignmentTargetRest
  }
  // `...e` in an array literal or in arguments is an `Expr`, in an object literal a `Prop`.
  Node<'a> { SpreadElement }
  ExpressionStatement<'a> { ExpressionStatement }
  Func<'a> {
    Function ArrowFunctionExpression
    // The `{ .. }`, or the expression after `=>`.
    FunctionBody
    // The `( .. )`.
    FormalParameters
  }
  Class<'a> { Class ClassBody }
  Param<'a> { FormalParameter FormalParameterRest TSThisParameter }
  Member<'a> {
    MethodDefinition PropertyDefinition AccessorProperty StaticBlock TSIndexSignature
    TSPropertySignature TSMethodSignature TSCallSignatureDeclaration
    TSConstructSignatureDeclaration
  }
  Prop<'a> {
    ObjectProperty JSXAttribute JSXSpreadAttribute AssignmentTargetPropertyIdentifier
    AssignmentTargetPropertyProperty
  }
  VarDecl<'a> { VariableDeclarator CatchParameter }
  Stmt<'a> {
    Directive BlockStatement EmptyStatement DebuggerStatement VariableDeclaration
    ReturnStatement IfStatement ForStatement ForInStatement ForOfStatement WhileStatement
    DoWhileStatement WithStatement SwitchStatement TryStatement ThrowStatement BreakStatement
    ContinueStatement LabeledStatement ImportDeclaration ExportAllDeclaration
    TSInterfaceDeclaration TSTypeAliasDeclaration TSEnumDeclaration TSModuleDeclaration
    TSGlobalDeclaration TSImportEqualsDeclaration TSExportAssignment
    TSNamespaceExportDeclaration
    // `export { a }`, or the `export` around a declaration. The field is also the declaration.
    ExportNamedDeclaration
    // `export default e`, or the `export default` around a declaration.
    ExportDefaultDeclaration
    // `catch (e) { .. }`. The field is the `try` statement.
    CatchClause
    // The `{ .. }` of the declaration.
    TSInterfaceBody TSEnumBody TSModuleBlock
  }
  Case<'a> { SwitchCase }
  EnumMember<'a> { TSEnumMember }
  ImportSpec<'a> { ImportSpecifier }
  ExportSpec<'a> { ExportSpecifier }
  Pat<'a> { BindingIdentifier ObjectPattern ArrayPattern }
  PatProp<'a> { BindingProperty }
  // `a = 1` and `...a` in a pattern: a `PatElem` or a `PatProp`.
  Node<'a> { AssignmentPattern BindingRestElement }
  TypeNode<'a> {
    // `any`, `string`, ..
    TSKeywordType
    TSThisType TSTypeReference TSLiteralType TSTemplateLiteralType TSArrayType TSTupleType
    TSUnionType TSIntersectionType TSFunctionType TSConstructorType TSTypeLiteral
    TSConditionalType TSInferType TSMappedType TSIndexedAccessType TSTypeOperator TSTypeQuery
    TSImportType TSTypePredicate
    // An element of `implements` or of the `extends` of an interface.
    TSClassImplements TSInterfaceHeritage
    // The `: T` around `T`.
    TSTypeAnnotation
  }
  TupleElem<'a> { TSNamedTupleMember TSOptionalType TSRestType }
  TypeParam<'a> { TSTypeParameter }
  // The `<..>`. The field is the node that has the type arguments or parameters.
  Node<'a> { TSTypeParameterInstantiation TSTypeParameterDeclaration }
}

/// `a, b, c` is one node in oxc, so no comma is inside another.
pub(crate) fn is_inner_comma(_: Expr<'_>) -> bool {
  false
}

pub(crate) trait AsAstNodes<'a>: Copy {
  fn as_ast_nodes(self) -> AstNodes<'a>;

  fn ast_parent(self) -> AstNodes<'a> {
    self.as_ast_nodes().parent()
  }

  /// The parent, its parent, and so on up to `Program`.
  fn ast_ancestors(self) -> impl Iterator<Item = AstNodes<'a>> {
    self.ast_parent().ancestors()
  }
}

impl<'a> AsAstNodes<'a> for Expr<'a> {
  /// For a whole optional chain, the `ChainExpression`. See [`ChainElement::as_chain_element`].
  fn as_ast_nodes(self) -> AstNodes<'a> {
    match self.is_chain_root() {
      true => AstNodes::ChainExpression(self),
      false => self.as_chain_element(),
    }
  }

  /// Skips the `ChainExpression`, if there is one.
  fn ast_parent(self) -> AstNodes<'a> {
    match self.is_chain_root() {
      true => AstNodes::ChainExpression(self).parent(),
      false => self.as_chain_element().parent(),
    }
  }
}

pub(crate) trait ChainElement<'a> {
  /// The member access, call or `!` itself, even if it's a whole optional chain. In that case its parent is the
  /// `ChainExpression`. For any other expression this is the same as [`AsAstNodes::as_ast_nodes`].
  #[allow(clippy::wrong_self_convention, reason = "handles and comments are Copy")]
  fn as_chain_element(self) -> AstNodes<'a>;
}

impl<'a> ChainElement<'a> for Expr<'a> {
  fn as_chain_element(self) -> AstNodes<'a> {
    AstNodes::at(self.file, self.id).unwrap_or(AstNodes::Elision(self))
  }
}

macro_rules! by_id {
  ($($handle:ident),*) => {
    $(impl<'a> AsAstNodes<'a> for $handle<'a> {
      fn as_ast_nodes(self) -> AstNodes<'a> {
        AstNodes::at(self.file, self.id).unwrap_or_else(|| AstNodes::around(self.file, self.id))
      }
    })*
  };
}

by_id!(
  Func, Class, Param, Member, Prop, VarDecl, TypeNode, TupleElem, Pat, PatProp, TypeParam, Case,
  EnumMember
);

impl<'a> AsAstNodes<'a> for Stmt<'a> {
  fn as_ast_nodes(self) -> AstNodes<'a> {
    match Node::of(self.file, self.id) {
      Some(Node::Stmt(_)) => {
        AstNodes::at(self.file, self.id).unwrap_or(AstNodes::EmptyStatement(self))
      }
      _ => AstNodes::ExpressionStatement(ExpressionStatement::Stmt(self)),
    }
  }
}

/// The node of a statement's declaration: the declaration of an `export`, or the statement itself.
pub(crate) fn inner_of_stmt<'a>(statement: Stmt<'a>) -> AstNodes<'a> {
  let file = statement.file;
  let inner = match file.kind(statement.id) {
    Some(K::ExportDeclaration(it)) => declaration_id(&it.declaration),
    Some(K::ExportDefaultDeclaration(it)) => match &it.declaration {
      ExportDefaultDeclarationKind::FunctionDeclaration(it) => it.node_id.get().index() as u32,
      ExportDefaultDeclarationKind::ClassDeclaration(it) => it.node_id.get().index() as u32,
      ExportDefaultDeclarationKind::TSInterfaceDeclaration(it) => it.node_id.get().index() as u32,
      _ => return statement.as_ast_nodes(),
    },
    _ => return statement.as_ast_nodes(),
  };
  AstNodes::at(file, inner).unwrap_or_else(|| statement.as_ast_nodes())
}

fn declaration_id(declaration: &Declaration) -> u32 {
  declaration.node_id().index() as u32
}

/// The node of any handle.
pub(crate) fn node_as_ast_nodes<'a>(node: Node<'a>) -> AstNodes<'a> {
  match node {
    Node::File(file) => AstNodes::Program(Program(file)),
    Node::Expr(e) => e.as_ast_nodes(),
    Node::Stmt(statement) => statement.as_ast_nodes(),
    Node::PatProp(prop) => AstNodes::BindingProperty(prop),
    node => AstNodes::at(node.file(), node.id())
      .unwrap_or_else(|| AstNodes::around(node.file(), node.id())),
  }
}

/// The function whose expression body `e` is, if it's one.
fn arrow_of_body<'a>(e: Expr<'a>) -> Option<Func<'a>> {
  let parent = e.file.parent_id(e.id);
  match e.file.kind(parent)? {
    K::ArrowFunctionExpression(arrow)
      if !matches!(arrow.body, ArrowFunctionBody::FunctionBody(_)) =>
    {
      Some(Func::new(e.file, parent))
    }
    _ => None,
  }
}

impl<'a> AstNodes<'a> {
  /// The node with oxc's id `id`, or `None` if it isn't one of these kinds. A whole optional chain is its member
  /// access or call, whose parent is the `ChainExpression`.
  pub(crate) fn at(file: &'a File<'a>, id: u32) -> Option<AstNodes<'a>> {
    use AstNodes as N;
    let parent = || file.parent_id(id);
    let e = || Expr::new(file, id);
    let statement = |id: u32| match file.kind(file.parent_id(id)) {
      Some(K::ExportDeclaration(_) | K::ExportDefaultDeclaration(_)) => {
        Stmt::new(file, file.parent_id(id))
      }
      _ => Stmt::new(file, id),
    };
    let ty = || TypeNode::new(file, id);
    let node = match file.kind(id)? {
      K::Program(_) => N::Program(Program(file)),
      K::IdentifierReference(_) => N::IdentifierReference(e()),
      K::BooleanLiteral(_) => N::BooleanLiteral(e()),
      K::NullLiteral(_) => N::NullLiteral(e()),
      K::NumericLiteral(_) => N::NumericLiteral(e()),
      K::BigIntLiteral(_) => N::BigIntLiteral(e()),
      K::RegExpLiteral(_) => N::RegExpLiteral(e()),
      K::StringLiteral(_) => N::StringLiteral(e()),
      K::TemplateLiteral(_) => N::TemplateLiteral(e()),
      K::ThisExpression(_) => N::ThisExpression(e()),
      K::Super(_) => N::Super(e()),
      K::ArrayExpression(_) => N::ArrayExpression(e()),
      K::ObjectExpression(_) => N::ObjectExpression(e()),
      K::StaticMemberExpression(_) => N::StaticMemberExpression(e()),
      K::ComputedMemberExpression(_) => N::ComputedMemberExpression(e()),
      K::PrivateFieldExpression(_) => N::PrivateFieldExpression(e()),
      K::CallExpression(_) => N::CallExpression(e()),
      K::NewExpression(_) => N::NewExpression(e()),
      K::ImportExpression(_) => N::ImportExpression(e()),
      K::ImportMeta(_) | K::NewTarget(_) => N::MetaProperty(e()),
      K::TaggedTemplateExpression(_) => N::TaggedTemplateExpression(e()),
      K::UnaryExpression(_) => N::UnaryExpression(e()),
      K::UpdateExpression(_) => N::UpdateExpression(e()),
      K::BinaryExpression(_) => N::BinaryExpression(e()),
      K::LogicalExpression(_) => N::LogicalExpression(e()),
      K::PrivateInExpression(_) => N::PrivateInExpression(e()),
      K::AssignmentExpression(_) => N::AssignmentExpression(e()),
      K::ConditionalExpression(_) => N::ConditionalExpression(e()),
      K::AwaitExpression(_) => N::AwaitExpression(e()),
      K::YieldExpression(_) => N::YieldExpression(e()),
      K::TSAsExpression(_) => N::TSAsExpression(e()),
      K::TSSatisfiesExpression(_) => N::TSSatisfiesExpression(e()),
      K::TSTypeAssertion(_) => N::TSTypeAssertion(e()),
      K::TSNonNullExpression(_) => N::TSNonNullExpression(e()),
      K::TSInstantiationExpression(_) => N::TSInstantiationExpression(e()),
      K::JSXElement(_) => N::JSXElement(e()),
      K::JSXFragment(_) => N::JSXFragment(e()),
      K::JSXText(_) => N::JSXText(e()),
      K::JSXEmptyExpression(_) => N::JSXEmptyExpression(e()),
      K::Elision(_) => N::Elision(e()),
      K::PrivateIdentifier(_) => N::PrivateIdentifier(e()),
      K::SequenceExpression(_) => N::SequenceExpression(e()),
      K::JSXExpressionContainer(it) => N::JSXExpressionContainer(Expr::new(
        file,
        match &it.expression {
          JSXExpression::EmptyExpression(empty) => empty.node_id.get().index() as u32,
          expression => expression.as_expression().map_or(id, expression_id),
        },
      )),
      K::JSXSpreadChild(_) => N::JSXSpreadChild(e()),
      K::JSXOpeningElement(_) => N::JSXOpeningElement(Expr::new(file, parent())),
      K::Decorator(it) => N::Decorator(Expr::new(file, expression_id(&it.expression))),
      K::ArrayAssignmentTarget(_) => N::ArrayAssignmentTarget(e()),
      K::ObjectAssignmentTarget(_) => N::ObjectAssignmentTarget(e()),
      K::AssignmentTargetWithDefault(_) => N::AssignmentTargetWithDefault(e()),
      K::AssignmentTargetRest(_) => match Node::of(file, id)? {
        Node::Prop(_) => N::SpreadElement(Node::of(file, id)?),
        _ => N::AssignmentTargetRest(e()),
      },
      K::SpreadElement(_) => N::SpreadElement(Node::of(file, id)?),
      K::ExpressionStatement(_)
        if file.path().first() == Some(&0)
          && matches!(file.kind(parent()), Some(K::Program(_))) =>
      {
        N::Program(Program(file))
      }
      K::ExpressionStatement(_) => {
        N::ExpressionStatement(ExpressionStatement::Stmt(Stmt::new(file, id)))
      }
      K::Function(_) => N::Function(Func::new(file, id)),
      K::ArrowFunctionExpression(_) => N::ArrowFunctionExpression(Func::new(file, id)),
      K::FunctionBody(_) => N::FunctionBody(Func::new(file, parent())),
      K::FormalParameters(_) => N::FormalParameters(Func::new(file, parent())),
      K::Class(_) => N::Class(Class::new(file, id)),
      K::ClassBody(_) => N::ClassBody(Class::new(file, parent())),
      K::FormalParameter(_) => N::FormalParameter(Param::new(file, id)),
      K::FormalParameterRest(_) => N::FormalParameterRest(Param::new(file, id)),
      K::TSThisParameter(_) => N::TSThisParameter(Param::new(file, id)),
      K::MethodDefinition(_) => N::MethodDefinition(Member::new(file, id)),
      K::PropertyDefinition(_) => N::PropertyDefinition(Member::new(file, id)),
      K::AccessorProperty(_) => N::AccessorProperty(Member::new(file, id)),
      K::StaticBlock(_) => N::StaticBlock(Member::new(file, id)),
      K::TSIndexSignature(_) => N::TSIndexSignature(Member::new(file, id)),
      K::TSPropertySignature(_) => N::TSPropertySignature(Member::new(file, id)),
      K::TSMethodSignature(_) => N::TSMethodSignature(Member::new(file, id)),
      K::TSCallSignatureDeclaration(_) => N::TSCallSignatureDeclaration(Member::new(file, id)),
      K::TSConstructSignatureDeclaration(_) => {
        N::TSConstructSignatureDeclaration(Member::new(file, id))
      }
      K::ObjectProperty(_) | K::ImportAttribute(_) => N::ObjectProperty(Prop::new(file, id)),
      K::JSXAttribute(_) => N::JSXAttribute(Prop::new(file, id)),
      K::JSXSpreadAttribute(_) => N::JSXSpreadAttribute(Prop::new(file, id)),
      K::AssignmentTargetPropertyIdentifier(_) => {
        N::AssignmentTargetPropertyIdentifier(Prop::new(file, id))
      }
      K::AssignmentTargetPropertyProperty(_) => {
        N::AssignmentTargetPropertyProperty(Prop::new(file, id))
      }
      K::VariableDeclarator(_) => N::VariableDeclarator(VarDecl::new(file, id)),
      K::CatchParameter(_) => N::CatchParameter(VarDecl::new(file, id)),
      K::Directive(_) => N::Directive(Stmt::new(file, id)),
      K::BlockStatement(_) => N::BlockStatement(Stmt::new(file, id)),
      K::EmptyStatement(_) => N::EmptyStatement(Stmt::new(file, id)),
      K::DebuggerStatement(_) => N::DebuggerStatement(Stmt::new(file, id)),
      K::VariableDeclaration(_) => N::VariableDeclaration(statement(id)),
      K::ReturnStatement(_) => N::ReturnStatement(Stmt::new(file, id)),
      K::IfStatement(_) => N::IfStatement(Stmt::new(file, id)),
      K::ForStatement(_) => N::ForStatement(Stmt::new(file, id)),
      K::ForInStatement(_) => N::ForInStatement(Stmt::new(file, id)),
      K::ForOfStatement(_) => N::ForOfStatement(Stmt::new(file, id)),
      K::WhileStatement(_) => N::WhileStatement(Stmt::new(file, id)),
      K::DoWhileStatement(_) => N::DoWhileStatement(Stmt::new(file, id)),
      K::WithStatement(_) => N::WithStatement(Stmt::new(file, id)),
      K::SwitchStatement(_) => N::SwitchStatement(Stmt::new(file, id)),
      K::TryStatement(_) => N::TryStatement(Stmt::new(file, id)),
      K::ThrowStatement(_) => N::ThrowStatement(Stmt::new(file, id)),
      K::BreakStatement(_) => N::BreakStatement(Stmt::new(file, id)),
      K::ContinueStatement(_) => N::ContinueStatement(Stmt::new(file, id)),
      K::LabeledStatement(_) => N::LabeledStatement(Stmt::new(file, id)),
      K::ImportDeclaration(_) => N::ImportDeclaration(Stmt::new(file, id)),
      K::ExportAllDeclaration(_) => N::ExportAllDeclaration(Stmt::new(file, id)),
      K::TSInterfaceDeclaration(_) => N::TSInterfaceDeclaration(statement(id)),
      K::TSTypeAliasDeclaration(_) => N::TSTypeAliasDeclaration(statement(id)),
      K::TSEnumDeclaration(_) => N::TSEnumDeclaration(statement(id)),
      K::TSNamespaceDeclaration(_) | K::TSExternalModuleDeclaration(_) => {
        N::TSModuleDeclaration(statement(id))
      }
      K::TSGlobalDeclaration(_) => N::TSGlobalDeclaration(statement(id)),
      K::TSImportEqualsDeclaration(_) => N::TSImportEqualsDeclaration(statement(id)),
      K::TSExportAssignment(_) => N::TSExportAssignment(Stmt::new(file, id)),
      K::TSNamespaceExportDeclaration(_) => N::TSNamespaceExportDeclaration(Stmt::new(file, id)),
      K::ExportNamedDeclaration(_) | K::ExportFromDeclaration(_) | K::ExportDeclaration(_) => {
        N::ExportNamedDeclaration(Stmt::new(file, id))
      }
      K::ExportDefaultDeclaration(_) => N::ExportDefaultDeclaration(Stmt::new(file, id)),
      K::CatchClause(_) => N::CatchClause(Stmt::new(file, parent())),
      K::TSInterfaceBody(_) | K::TSEnumBody(_) | K::TSModuleBlock(_) => {
        let owner = statement(module_root(file, parent()));
        match file.kind(id)? {
          K::TSInterfaceBody(_) => N::TSInterfaceBody(owner),
          K::TSEnumBody(_) => N::TSEnumBody(owner),
          _ => N::TSModuleBlock(owner),
        }
      }
      K::SwitchCase(_) => N::SwitchCase(Case::new(file, id)),
      K::TSEnumMember(_) => N::TSEnumMember(EnumMember::new(file, id)),
      K::ImportSpecifier(_) | K::ImportDefaultSpecifier(_) | K::ImportNamespaceSpecifier(_) => {
        N::ImportSpecifier(ImportSpec::new(file, id))
      }
      K::ExportSpecifier(_) => N::ExportSpecifier(ExportSpec::new(file, id)),
      K::BindingIdentifier(_) => match Node::of(file, id)? {
        Node::Pat(_) => N::BindingIdentifier(Pat::new(file, id)),
        _ => return None,
      },
      K::ObjectPattern(_) => N::ObjectPattern(Pat::new(file, id)),
      K::ArrayPattern(_) => N::ArrayPattern(Pat::new(file, id)),
      K::BindingProperty(_) => N::BindingProperty(PatProp::new(file, id)),
      K::AssignmentPattern(_) => {
        N::AssignmentPattern(Node::of(file, id).unwrap_or_else(|| Node::around(file, id)))
      }
      K::BindingRestElement(_) => {
        N::BindingRestElement(Node::of(file, id).unwrap_or_else(|| Node::around(file, id)))
      }
      K::TSAnyKeyword(_)
      | K::TSStringKeyword(_)
      | K::TSBooleanKeyword(_)
      | K::TSNumberKeyword(_)
      | K::TSNeverKeyword(_)
      | K::TSIntrinsicKeyword(_)
      | K::TSUnknownKeyword(_)
      | K::TSNullKeyword(_)
      | K::TSUndefinedKeyword(_)
      | K::TSVoidKeyword(_)
      | K::TSSymbolKeyword(_)
      | K::TSObjectKeyword(_)
      | K::TSBigIntKeyword(_) => N::TSKeywordType(ty()),
      K::TSThisType(_) => N::TSThisType(ty()),
      K::TSTypeReference(_) => N::TSTypeReference(ty()),
      K::TSLiteralType(_) => N::TSLiteralType(ty()),
      K::TSTemplateLiteralType(_) => N::TSTemplateLiteralType(ty()),
      K::TSArrayType(_) => N::TSArrayType(ty()),
      K::TSTupleType(_) => N::TSTupleType(ty()),
      K::TSUnionType(_) => N::TSUnionType(ty()),
      K::TSIntersectionType(_) => N::TSIntersectionType(ty()),
      K::TSFunctionType(_) => N::TSFunctionType(ty()),
      K::TSConstructorType(_) => N::TSConstructorType(ty()),
      K::TSTypeLiteral(_) => N::TSTypeLiteral(ty()),
      K::TSConditionalType(_) => N::TSConditionalType(ty()),
      K::TSInferType(_) => N::TSInferType(ty()),
      K::TSMappedType(_) => N::TSMappedType(ty()),
      K::TSIndexedAccessType(_) => N::TSIndexedAccessType(ty()),
      K::TSTypeOperator(_) => N::TSTypeOperator(ty()),
      K::TSTypeQuery(_) => N::TSTypeQuery(ty()),
      K::TSImportType(_) => N::TSImportType(ty()),
      K::TSTypePredicate(_) => N::TSTypePredicate(ty()),
      K::TSClassImplements(_) => N::TSClassImplements(ty()),
      K::TSInterfaceHeritage(_) => N::TSInterfaceHeritage(ty()),
      K::TSTypeAnnotation(it) => {
        N::TSTypeAnnotation(TypeNode::new(file, crate::js::ast::type_id(&it.type_annotation)))
      }
      K::TSNamedTupleMember(_) => N::TSNamedTupleMember(TupleElem::new(file, id)),
      K::TSOptionalType(_) => N::TSOptionalType(TupleElem::new(file, id)),
      K::TSRestType(_) => N::TSRestType(TupleElem::new(file, id)),
      K::TSTypeParameter(_) => N::TSTypeParameter(TypeParam::new(file, id)),
      K::TSTypeParameterInstantiation(_) => N::TSTypeParameterInstantiation(owner(file, parent())),
      K::TSTypeParameterDeclaration(_) => N::TSTypeParameterDeclaration(owner(file, parent())),
      _ => return None,
    };
    Some(node)
  }

  /// The innermost node of these kinds around the node with oxc's id `id`.
  pub(crate) fn around(file: &'a File<'a>, mut id: u32) -> AstNodes<'a> {
    loop {
      id = file.parent_id(id);
      if id == crate::js::ast::NONE {
        return AstNodes::Program(Program(file));
      }
      if let Some(node) = AstNodes::at(file, id) {
        return node;
      }
    }
  }

  /// oxc's id of the node. `None` where oxc has no node: a `ChainExpression`, and the body of an arrow function with an
  /// expression body and the statement in it.
  fn id(self) -> Option<u32> {
    use AstNodes as N;
    let id = match self {
      N::Program(_) => 0,
      N::ChainExpression(_) => return None,
      N::JSXExpressionContainer(e) | N::Decorator(e) => e.file.parent_id(e.id),
      N::JSXOpeningElement(e) => match e.file.kind(e.id)? {
        K::JSXElement(it) => it.opening_element.node_id.get().index() as u32,
        _ => return None,
      },
      N::ExpressionStatement(ExpressionStatement::ArrowBody(_)) => return None,
      N::FunctionBody(func) => match func.file.kind(func.id)? {
        K::Function(it) => it.body.as_ref()?.node_id.get().index() as u32,
        K::ArrowFunctionExpression(it) => match &it.body {
          ArrowFunctionBody::FunctionBody(body) => body.node_id.get().index() as u32,
          _ => return None,
        },
        _ => return None,
      },
      N::FormalParameters(func) => match func.file.kind(func.id)? {
        K::Function(it) => it.params.node_id.get().index() as u32,
        K::ArrowFunctionExpression(it) => it.params.node_id.get().index() as u32,
        K::TSMethodSignature(it) => it.params.node_id.get().index() as u32,
        K::TSCallSignatureDeclaration(it) => it.params.node_id.get().index() as u32,
        K::TSConstructSignatureDeclaration(it) => it.params.node_id.get().index() as u32,
        K::TSFunctionType(it) => it.params.node_id.get().index() as u32,
        K::TSConstructorType(it) => it.params.node_id.get().index() as u32,
        _ => return None,
      },
      N::ClassBody(class) => match class.file.kind(class.id)? {
        K::Class(it) => it.body.node_id.get().index() as u32,
        _ => return None,
      },
      N::CatchClause(statement) => match statement.file.kind(statement.id)? {
        K::TryStatement(it) => it.handler.as_ref()?.node_id.get().index() as u32,
        _ => return None,
      },
      N::TSInterfaceBody(statement) | N::TSEnumBody(statement) | N::TSModuleBlock(statement) => {
        let inner = inner_of_stmt(statement).id()?;
        let file = statement.file;
        match file.kind(inner)? {
          K::TSInterfaceDeclaration(it) => it.body.node_id.get().index() as u32,
          K::TSEnumDeclaration(it) => it.body.node_id.get().index() as u32,
          K::TSNamespaceDeclaration(_) => module_block(file, inner)?,
          K::TSExternalModuleDeclaration(it) => it.body.as_ref()?.node_id.get().index() as u32,
          K::TSGlobalDeclaration(it) => it.body.node_id.get().index() as u32,
          _ => return None,
        }
      }
      N::VariableDeclaration(statement)
      | N::TSInterfaceDeclaration(statement)
      | N::TSTypeAliasDeclaration(statement)
      | N::TSEnumDeclaration(statement)
      | N::TSModuleDeclaration(statement)
      | N::TSGlobalDeclaration(statement)
      | N::TSImportEqualsDeclaration(statement) => match statement.file.kind(statement.id)? {
        K::ExportDeclaration(it) => declaration_id(&it.declaration),
        _ => statement.id,
      },
      N::TSTypeAnnotation(ty) => ty.file.parent_id(ty.id),
      N::TSTypeParameterInstantiation(owner) => type_arguments_id(owner)?,
      N::TSTypeParameterDeclaration(owner) => type_parameters_id(owner)?,
      node => node.raw().id(),
    };
    Some(id)
  }

  /// Returns the expression if this is one, meaning something that can have parentheses around it.
  pub(crate) fn as_expression(self) -> Option<Expr<'a>> {
    use AstNodes as N;
    match self {
      N::Function(func) | N::ArrowFunctionExpression(func) => func.owner().as_expr(),
      N::Class(class) => class.owner().as_expr(),
      N::SpreadElement(_)
      | N::JSXExpressionContainer(_)
      | N::JSXSpreadChild(_)
      | N::JSXOpeningElement(_)
      | N::Decorator(_)
      | N::ChainExpression(_)
      | N::AssignmentTargetRest(_) => None,
      _ => match self.raw() {
        Node::Expr(e) => Some(e),
        _ => None,
      },
    }
  }

  /// Returns the direct parent. `Program` is its own parent.
  pub(crate) fn parent(self) -> AstNodes<'a> {
    use AstNodes as N;
    match self {
      N::Program(_) => self,
      N::ChainExpression(e) => match arrow_of_body(e) {
        Some(func) => N::ExpressionStatement(ExpressionStatement::ArrowBody(func)),
        None => AstNodes::around(e.file, e.id),
      },
      N::ExpressionStatement(ExpressionStatement::ArrowBody(func)) => N::FunctionBody(func),
      N::FunctionBody(func) if self.id().is_none() => N::ArrowFunctionExpression(func),
      _ => match self.id() {
        Some(id) => {
          if let Some(e) = self.as_expression() {
            if e.is_chain_root() {
              return N::ChainExpression(e);
            }
            if let Some(func) = arrow_of_body(e) {
              return N::ExpressionStatement(ExpressionStatement::ArrowBody(func));
            }
          }
          AstNodes::around(self.raw().file(), id)
        }
        None => N::Program(Program(self.raw().file())),
      },
    }
  }

  pub(crate) fn ancestors(self) -> impl Iterator<Item = AstNodes<'a>> {
    std::iter::successors(Some(self), |node| match node {
      AstNodes::Program(_) => None,
      node => Some(node.parent()),
    })
  }

  /// Returns the parent of a `ChainExpression`, and any other node unchanged.
  pub(crate) fn without_chain_expression(self) -> AstNodes<'a> {
    match self {
      AstNodes::ChainExpression(_) => self.parent().without_chain_expression(),
      _ => self,
    }
  }

  /// True if this is a call or `new` expression whose callee is `e`.
  pub(crate) fn is_call_like_callee(self, e: Expr<'a>) -> bool {
    match self {
      AstNodes::CallExpression(call) | AstNodes::NewExpression(call) => call.callee() == Some(e),
      _ => false,
    }
  }

  /// [`AstNodes::span`], but starting at Prettier's `locStart`. A class's decorators can come before its `export`,
  /// and then belong to both the class and the export: `@decorator export class A {}`.
  pub(crate) fn span_with_decorators(self) -> Span {
    use AstNodes as N;
    let span = self.span();
    let class = match self {
      N::Class(class) => Some(class),
      N::ExportNamedDeclaration(statement) | N::ExportDefaultDeclaration(statement) => {
        match statement.kind() {
          StmtKind::Class(class) => Some(class),
          StmtKind::ExportDefault(e) => e.as_class(),
          _ => None,
        }
      }
      _ => None,
    };
    match class.and_then(|class| class.decorators().next()) {
      Some(first) => Span::new(N::Decorator(first).span().start.min(span.start), span.end),
      None => span,
    }
  }

  /// Returns the node's range in oxc.
  pub(crate) fn span(self) -> Span {
    use AstNodes as N;
    match self {
      N::ChainExpression(e) => e.span(),
      N::ExpressionStatement(statement @ ExpressionStatement::ArrowBody(_)) => statement.span(),
      N::FunctionBody(func) if self.id().is_none() => match func.body() {
        FnBody::Expr(e) => e.span(),
        _ => func.span(),
      },
      _ => match self.id() {
        Some(id) => self.raw().file().span_of(id),
        None => self.raw().span(),
      },
    }
  }
}

impl Spanned for AstNodes<'_> {
  fn span(&self) -> Span {
    AstNodes::span(*self)
  }
}

/// The namespace declaration that `namespace A.B {}` starts with, for the `B` whose id is `id`.
fn module_root(file: &File, mut id: u32) -> u32 {
  while let Some(K::TSNamespaceDeclaration(_)) = file.kind(file.parent_id(id)) {
    id = file.parent_id(id);
  }
  id
}

/// The block of `namespace A.B.C {}`, from the declaration of `A`.
fn module_block(file: &File, id: u32) -> Option<u32> {
  let mut at = match file.kind(id)? {
    K::TSNamespaceDeclaration(it) => it,
    _ => return None,
  };
  loop {
    match &at.body {
      TSNamespaceDeclarationBody::TSNamespaceDeclaration(inner) => at = inner,
      TSNamespaceDeclarationBody::TSModuleBlock(block) => {
        return Some(block.node_id.get().index() as u32);
      }
    }
  }
}

/// The node that has type arguments or parameters, for the `<..>` whose parent is `parent`.
fn owner<'a>(file: &'a File<'a>, parent: u32) -> Node<'a> {
  match file.kind(parent) {
    Some(K::Class(_)) => Node::Class(Class::new(file, parent)),
    Some(K::Function(_) | K::ArrowFunctionExpression(_)) => Node::Func(Func::new(file, parent)),
    _ => Node::of(file, parent).unwrap_or_else(|| Node::around(file, parent)),
  }
}

/// The `<..>` of `owner`. An element's is in its opening tag.
fn type_arguments_id(owner: Node) -> Option<u32> {
  let file = owner.file();
  let id = match file.kind(owner.id()) {
    Some(K::JSXElement(it)) => it.opening_element.node_id.get().index() as u32,
    _ => owner.id(),
  };
  (file.children(id).iter().copied())
    .find(|&child| matches!(file.kind(child), Some(K::TSTypeParameterInstantiation(_))))
}

fn type_parameters_id(owner: Node) -> Option<u32> {
  let file = owner.file();
  file
    .children(owner.id())
    .iter()
    .copied()
    .find(|&child| matches!(file.kind(child), Some(K::TSTypeParameterDeclaration(_))))
}
