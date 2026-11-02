//! Handles, and [`Node`], which is any of them: which handle each of oxc's nodes is, and its parent as bun's tree has
//! it.

use super::{File, NONE, Span, StmtTag};
use oxc_ast::AstKind;
use oxc_ast::ast::{ImportAttributeKey, PropertyKey, TSTypeOperatorOperator};
use oxc_span::GetSpan;

/// Declares handles that aren't nodes of their own: a file and the id of a node in it.
macro_rules! handle_structs {
  ($($(#[$doc:meta])* $name:ident,)*) => {
    $(
      $(#[$doc])*
      #[derive(Copy, Clone)]
      pub(crate) struct $name<'a> {
        pub(crate) file: &'a File<'a>,
        pub(crate) id: u32,
      }

      #[allow(dead_code, reason = "every handle has the same accessors")]
      impl<'a> $name<'a> {
        #[inline]
        pub(crate) fn new(file: &'a File<'a>, id: u32) -> Self {
          $name { file, id }
        }

        #[inline]
        pub(crate) fn id(self) -> u32 {
          self.id
        }

        #[inline]
        pub(crate) fn file(self) -> &'a File<'a> {
          self.file
        }

        /// The oxc node.
        #[inline]
        pub(crate) fn raw(self) -> AstKind<'a> {
          self.file.kind(self.id).unwrap_or(AstKind::Program(self.file.program))
        }

        /// From its first token to the end of its last, without parentheses around it.
        #[inline]
        pub(crate) fn span(self) -> Span {
          let raw = self.raw();
          let span = raw.span();
          match raw {
            AstKind::TSIndexSignatureName(it) if stringify!($name) == "Pat" => {
              Span::new(span.start, span.start + it.name.len() as u32)
            }
            AstKind::TSThisParameter(_) if stringify!($name) == "Pat" => Span::new(span.start, span.start + 4),
            _ => Span::new(span.start, span.end),
          }
        }

        /// The node's source text.
        #[inline]
        pub(crate) fn text(self) -> &'a [u8] {
          self.file.slice(self.span())
        }

        /// The node's source text as a string.
        #[inline]
        pub(crate) fn source(self) -> &'a str {
          self.file.str(self.span())
        }
      }

      impl PartialEq for $name<'_> {
        #[inline]
        fn eq(&self, other: &Self) -> bool {
          self.id == other.id
        }
      }

      impl Eq for $name<'_> {}

      impl std::hash::Hash for $name<'_> {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
          self.id.hash(state);
        }
      }

      impl std::fmt::Debug for $name<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          write!(f, "{}({})", stringify!($name), self.id)
        }
      }

      impl super::Spanned for $name<'_> {
        #[inline]
        fn span(&self) -> Span {
          $name::span(*self)
        }
      }
    )*
  };
}

/// Declares handles, and [`Node`], which is any of them.
macro_rules! handles {
  ($($(#[$doc:meta])* $variant:ident $name:ident,)*) => {
    handle_structs! { $($(#[$doc])* $name,)* }

    $(impl<'a> From<$name<'a>> for Node<'a> {
      #[inline]
      fn from(it: $name<'a>) -> Node<'a> {
        Node::$variant(it)
      }
    })*

    /// Any node of a file.
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    pub(crate) enum Node<'a> {
      /// The root.
      File(&'a File<'a>),
      $($variant($name<'a>),)*
    }

    impl<'a> Node<'a> {
      pub(crate) fn file(self) -> &'a File<'a> {
        match self {
          Node::File(file) => file,
          $(Node::$variant(it) => it.file,)*
        }
      }

      /// The id of its node; the program's for the file.
      pub(crate) fn id(self) -> u32 {
        match self {
          Node::File(_) => 0,
          $(Node::$variant(it) => it.id,)*
        }
      }
    }
  };
}

handles! {
  Expr Expr,
  Stmt Stmt,
  Type TypeNode,
  Pat Pat,
  PatProp PatProp,
  PatElem PatElem,
  Func Func,
  Param Param,
  TypeParam TypeParam,
  Class Class,
  Member Member,
  Prop Prop,
  VarDecl VarDecl,
  Case Case,
  EnumMember EnumMember,
  ImportSpec ImportSpec,
  ExportSpec ExportSpec,
  TupleElem TupleElem,
}

macro_rules! plain_handles {
  ($($name:ident,)*) => {
    $(impl<'a> super::Handle<'a> for $name<'a> {
      #[inline]
      fn from_raw(file: &'a File<'a>, id: u32) -> Self {
        $name::new(file, id)
      }
    })*
  };
}

plain_handles! {
  Stmt,
  TypeNode,
  Pat,
  PatProp,
  PatElem,
  Func,
  Param,
  TypeParam,
  Class,
  Member,
  Prop,
  VarDecl,
  Case,
  EnumMember,
  ImportSpec,
  ExportSpec,
  TupleElem,
}

handle_structs! {
  /// `interface I {}`, by its statement.
  Interface,
  /// `type A = T`, by its statement.
  Alias,
  /// `enum E {}`, by its statement.
  Enum,
  /// `namespace N {}`, `declare module "m" {}`, `declare global {}`, by its statement.
  Module,
  /// `import .. from "m"`, by its statement.
  Import,
  /// `import a = require("m")`, `import a = b.c`, by its statement.
  ImportEquals,
  /// `export { .. }`, with or without `from`, by its statement.
  Export,
}

impl super::Spanned for Node<'_> {
  fn span(&self) -> Span {
    Node::span(*self)
  }
}

impl super::Spanned for File<'_> {
  fn span(&self) -> Span {
    Span::new(self.program.span.start, self.program.span.end)
  }
}

impl PartialEq for File<'_> {
  fn eq(&self, other: &Self) -> bool {
    std::ptr::eq(self, other)
  }
}

impl Eq for File<'_> {}

impl std::fmt::Debug for File<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("File")
  }
}

impl<'a> Node<'a> {
  /// The handle of the node with `id`. A function or a class that is an expression or a statement is that expression
  /// or statement, whose `Func` or `Class` has the same id. `None` for a node that isn't one of bun's: a name, a body,
  /// a type annotation, a part of JSX.
  pub(crate) fn of(file: &'a File<'a>, id: u32) -> Option<Node<'a>> {
    let kind = file.kind(id)?;
    let parent = || file.kind(file.parent_id(id));
    use AstKind as K;
    let exported =
      || matches!(parent(), Some(K::ExportDeclaration(_) | K::ExportDefaultDeclaration(_)));
    let node = match kind {
      K::Program(_) => Node::File(file),
      K::Function(_) | K::Class(_) if exported() => match kind {
        K::Function(_) => Node::Func(Func::new(file, id)),
        _ => Node::Class(Class::new(file, id)),
      },
      K::VariableDeclaration(_)
      | K::TSEnumDeclaration(_)
      | K::TSTypeAliasDeclaration(_)
      | K::TSInterfaceDeclaration(_)
      | K::TSExternalModuleDeclaration(_)
      | K::TSNamespaceDeclaration(_)
      | K::TSGlobalDeclaration(_)
      | K::TSImportEqualsDeclaration(_)
        if exported() =>
      {
        return None;
      }
      K::TSNamespaceDeclaration(_) if matches!(parent(), Some(K::TSNamespaceDeclaration(_))) => {
        return None;
      }
      K::Function(function) => match parent() {
        Some(K::MethodDefinition(_)) => Node::Func(Func::new(file, id)),
        _ if function.is_declaration() => Node::Stmt(Stmt::new(file, id)),
        _ => Node::Expr(Expr::new(file, id)),
      },
      K::Class(class) if class.is_declaration() => Node::Stmt(Stmt::new(file, id)),
      K::IdentifierReference(_) | K::ThisExpression(_) | K::TSQualifiedName(_)
        if matches!(
          name_owner(file, id),
          Some(K::TSTypeReference(_) | K::TSClassImplements(_))
        ) =>
      {
        return None;
      }
      K::TSQualifiedName(_) if matches!(name_owner(file, id), Some(K::TSTypeQuery(_))) => {
        Node::Expr(Expr::new(file, id))
      }
      K::NumericLiteral(_) | K::StringLiteral(_) | K::BigIntLiteral(_) | K::TemplateLiteral(_)
        if is_field(file, file.parent_id(id), id) =>
      {
        return None;
      }
      K::BooleanLiteral(_)
      | K::NumericLiteral(_)
      | K::StringLiteral(_)
      | K::BigIntLiteral(_)
      | K::TemplateLiteral(_)
      | K::UnaryExpression(_)
        if in_type(file, id) =>
      {
        return None;
      }
      K::IdentifierReference(_)
      | K::ThisExpression(_)
      | K::ArrayExpression(_)
      | K::Elision(_)
      | K::ObjectExpression(_)
      | K::TemplateLiteral(_)
      | K::TaggedTemplateExpression(_)
      | K::ComputedMemberExpression(_)
      | K::StaticMemberExpression(_)
      | K::PrivateFieldExpression(_)
      | K::CallExpression(_)
      | K::NewExpression(_)
      | K::ImportMeta(_)
      | K::NewTarget(_)
      | K::UpdateExpression(_)
      | K::UnaryExpression(_)
      | K::BinaryExpression(_)
      | K::PrivateInExpression(_)
      | K::LogicalExpression(_)
      | K::ConditionalExpression(_)
      | K::AssignmentExpression(_)
      | K::ArrayAssignmentTarget(_)
      | K::ObjectAssignmentTarget(_)
      | K::AssignmentTargetWithDefault(_)
      | K::SequenceExpression(_)
      | K::Super(_)
      | K::AwaitExpression(_)
      | K::ArrowFunctionExpression(_)
      | K::YieldExpression(_)
      | K::Class(_)
      | K::ImportExpression(_)
      | K::BooleanLiteral(_)
      | K::NullLiteral(_)
      | K::NumericLiteral(_)
      | K::StringLiteral(_)
      | K::BigIntLiteral(_)
      | K::RegExpLiteral(_)
      | K::JSXElement(_)
      | K::JSXFragment(_)
      | K::JSXEmptyExpression(_)
      | K::JSXText(_)
      | K::TSAsExpression(_)
      | K::TSSatisfiesExpression(_)
      | K::TSTypeAssertion(_)
      | K::TSNonNullExpression(_)
      | K::TSInstantiationExpression(_)
      | K::PrivateIdentifier(_) => Node::Expr(Expr::new(file, id)),
      K::JSXIdentifier(_) | K::JSXMemberExpression(_) | K::JSXNamespacedName(_) => match parent() {
        Some(K::JSXOpeningElement(_) | K::JSXClosingElement(_) | K::JSXMemberExpression(_)) => {
          Node::Expr(Expr::new(file, id))
        }
        _ => return None,
      },
      K::JSXSpreadChild(_) => Node::Expr(Expr::new(file, id)),
      K::JSXAttribute(_) | K::JSXSpreadAttribute(_) => Node::Prop(Prop::new(file, id)),
      K::AssignmentTargetRest(_) if matches!(parent(), Some(K::ObjectAssignmentTarget(_))) => {
        Node::Prop(Prop::new(file, id))
      }
      K::AssignmentTargetRest(_) => Node::Expr(Expr::new(file, id)),
      K::SpreadElement(_) => match parent() {
        Some(K::ObjectExpression(_)) => Node::Prop(Prop::new(file, id)),
        _ => Node::Expr(Expr::new(file, id)),
      },
      K::ObjectProperty(_)
      | K::ImportAttribute(_)
      | K::AssignmentTargetPropertyIdentifier(_)
      | K::AssignmentTargetPropertyProperty(_) => Node::Prop(Prop::new(file, id)),
      K::Directive(_)
      | K::BlockStatement(_)
      | K::VariableDeclaration(_)
      | K::EmptyStatement(_)
      | K::ExpressionStatement(_)
      | K::IfStatement(_)
      | K::DoWhileStatement(_)
      | K::WhileStatement(_)
      | K::ForStatement(_)
      | K::ForInStatement(_)
      | K::ForOfStatement(_)
      | K::ContinueStatement(_)
      | K::BreakStatement(_)
      | K::ReturnStatement(_)
      | K::WithStatement(_)
      | K::SwitchStatement(_)
      | K::LabeledStatement(_)
      | K::ThrowStatement(_)
      | K::TryStatement(_)
      | K::DebuggerStatement(_)
      | K::ImportDeclaration(_)
      | K::ExportNamedDeclaration(_)
      | K::ExportDeclaration(_)
      | K::ExportFromDeclaration(_)
      | K::ExportDefaultDeclaration(_)
      | K::ExportAllDeclaration(_)
      | K::TSEnumDeclaration(_)
      | K::TSTypeAliasDeclaration(_)
      | K::TSInterfaceDeclaration(_)
      | K::TSExternalModuleDeclaration(_)
      | K::TSNamespaceDeclaration(_)
      | K::TSGlobalDeclaration(_)
      | K::TSImportEqualsDeclaration(_)
      | K::TSExportAssignment(_)
      | K::TSNamespaceExportDeclaration(_) => Node::Stmt(Stmt::new(file, id)),
      K::VariableDeclarator(_) => Node::VarDecl(VarDecl::new(file, id)),
      K::SwitchCase(_) => Node::Case(Case::new(file, id)),
      K::BindingIdentifier(_) => match parent() {
        Some(
          K::Function(_)
          | K::Class(_)
          | K::TSTypeAliasDeclaration(_)
          | K::TSInterfaceDeclaration(_)
          | K::TSEnumDeclaration(_)
          | K::TSNamespaceDeclaration(_)
          | K::TSImportEqualsDeclaration(_)
          | K::TSTypeParameter(_)
          | K::ImportSpecifier(_)
          | K::ImportDefaultSpecifier(_)
          | K::ImportNamespaceSpecifier(_),
        ) => return None,
        Some(K::TSMappedType(_)) => Node::TypeParam(TypeParam::new(file, id)),
        _ => Node::Pat(Pat::new(file, id)),
      },
      K::AssignmentPattern(_) | K::BindingRestElement(_)
        if matches!(parent(), Some(K::ArrayPattern(_))) =>
      {
        Node::PatElem(PatElem::new(file, id))
      }
      K::BindingRestElement(_) if matches!(parent(), Some(K::ObjectPattern(_))) => {
        Node::PatProp(PatProp::new(file, id))
      }
      K::AssignmentPattern(_) if matches!(parent(), Some(K::BindingProperty(_))) => return None,
      K::BindingRestElement(_) if matches!(parent(), Some(K::FormalParameterRest(_))) => {
        return None;
      }
      K::AssignmentPattern(_)
      | K::ObjectPattern(_)
      | K::ArrayPattern(_)
      | K::BindingRestElement(_) => Node::Pat(Pat::new(file, id)),
      K::BindingProperty(_) => Node::PatProp(PatProp::new(file, id)),
      K::FormalParameter(_)
      | K::FormalParameterRest(_)
      | K::TSThisParameter(_)
      | K::TSIndexSignatureName(_) => Node::Param(Param::new(file, id)),
      K::MethodDefinition(_)
      | K::PropertyDefinition(_)
      | K::StaticBlock(_)
      | K::AccessorProperty(_)
      | K::TSIndexSignature(_)
      | K::TSPropertySignature(_)
      | K::TSCallSignatureDeclaration(_)
      | K::TSMethodSignature(_)
      | K::TSConstructSignatureDeclaration(_) => Node::Member(Member::new(file, id)),
      K::ImportSpecifier(_) | K::ImportDefaultSpecifier(_) | K::ImportNamespaceSpecifier(_) => {
        Node::ImportSpec(ImportSpec::new(file, id))
      }
      K::ExportSpecifier(_) => Node::ExportSpec(ExportSpec::new(file, id)),
      K::TSEnumMember(_) => Node::EnumMember(EnumMember::new(file, id)),
      K::TSTypeParameter(_) => Node::TypeParam(TypeParam::new(file, id)),
      K::TSNamedTupleMember(_) if matches!(parent(), Some(K::TSRestType(_))) => return None,
      K::TSImportType(_) if matches!(parent(), Some(K::TSTypeQuery(_))) => return None,
      K::TSThisType(_) if matches!(parent(), Some(K::TSTypePredicate(_))) => return None,
      K::TSSymbolKeyword(_) if parent().is_some_and(is_unique) => return None,
      K::TSNamedTupleMember(_) | K::TSOptionalType(_) | K::TSRestType(_)
        if matches!(parent(), Some(K::TSTupleType(_))) =>
      {
        Node::TupleElem(TupleElem::new(file, id))
      }
      K::TSLiteralType(_)
      | K::TSConditionalType(_)
      | K::TSUnionType(_)
      | K::TSIntersectionType(_)
      | K::TSTypeOperator(_)
      | K::TSArrayType(_)
      | K::TSIndexedAccessType(_)
      | K::TSTupleType(_)
      | K::TSNamedTupleMember(_)
      | K::TSOptionalType(_)
      | K::TSRestType(_)
      | K::TSAnyKeyword(_)
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
      | K::TSThisType(_)
      | K::TSObjectKeyword(_)
      | K::TSBigIntKeyword(_)
      | K::TSTypeReference(_)
      | K::TSTypePredicate(_)
      | K::TSTypeLiteral(_)
      | K::TSInferType(_)
      | K::TSTypeQuery(_)
      | K::TSImportType(_)
      | K::TSFunctionType(_)
      | K::TSConstructorType(_)
      | K::TSMappedType(_)
      | K::TSTemplateLiteralType(_)
      | K::TSInterfaceHeritage(_)
      | K::TSClassImplements(_)
      | K::JSDocNullableType(_)
      | K::JSDocNonNullableType(_)
      | K::JSDocUnknownType(_) => Node::Type(TypeNode::new(file, id)),
      _ => return None,
    };
    Some(node)
  }

  /// The innermost node of bun's that contains the node with `id`, or the file.
  pub(crate) fn around(file: &'a File<'a>, mut id: u32) -> Node<'a> {
    loop {
      id = file.parent_id(id);
      if id == NONE {
        return Node::File(file);
      }
      if let Some(AstKind::Class(_)) = file.kind(id) {
        return Node::Class(Class::new(file, id));
      }
      if let Some(node) = Node::of(file, id) {
        return node;
      }
    }
  }

  /// Its direct parent. The file is its own parent. A `Func` or a `Class`'s parent is the expression or statement with
  /// the same id, if there's one.
  pub(crate) fn parent(self) -> Node<'a> {
    match self {
      Node::File(_) => self,
      Node::Func(Func { file, id }) | Node::Class(Class { file, id }) => match Node::of(file, id) {
        Some(owner) if owner != self => owner,
        _ => Node::around(file, id),
      },
      _ => Node::around(self.file(), self.id()),
    }
  }

  pub(crate) fn as_expr(self) -> Option<Expr<'a>> {
    match self {
      Node::Expr(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn as_stmt(self) -> Option<Stmt<'a>> {
    match self {
      Node::Stmt(it) => Some(it),
      _ => None,
    }
  }

  /// A number below [`File::node_count`] that's unique to oxc's node, to index a per-node table or bit set. A `Func` or
  /// a `Class` shares it with the `Expr` or `Stmt` that owns it.
  pub(crate) fn index(self) -> u32 {
    self.id()
  }

  /// Calls `visit` with the children that can contain `offset`. In a run of more than 8 siblings of one kind, that's
  /// only the last one that starts at or before `offset`.
  pub(crate) fn for_each_child_near(self, offset: u32, mut visit: impl FnMut(Node<'a>)) {
    self.children_into(Some(offset), &mut visit);
  }

  fn children_into(self, near: Option<u32>, visit: &mut dyn FnMut(Node<'a>)) {
    let (file, id) = (self.file(), self.id());
    if let Node::Expr(_) | Node::Stmt(_) = self {
      match file.kind(id) {
        Some(AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) => {
          return visit(Node::Func(Func::new(file, id)));
        }
        Some(AstKind::Class(_)) => return visit(Node::Class(Class::new(file, id))),
        _ => {}
      }
    }
    match self {
      Node::Stmt(statement) if statement.is_wrapper() => visit(Node::Expr(Expr::new(file, id))),
      Node::Stmt(statement)
        if matches!(statement.tag(), StmtTag::ImportEquals | StmtTag::ExportAsNamespace) => {}
      Node::ImportSpec(_) | Node::ExportSpec(_) => {}
      _ => descend(file, id, near, visit),
    }
  }

  /// The program's span for the file.
  pub(crate) fn span(self) -> Span {
    match self {
      Node::File(file) => Span::new(file.program.span.start, file.program.span.end),
      _ => self.file().span_of(self.id()),
    }
  }
}

/// Visits the handles among the children of the node with `id`, and those inside the children that aren't handles.
fn descend<'a>(file: &'a File<'a>, id: u32, near: Option<u32>, visit: &mut dyn FnMut(Node<'a>)) {
  let children = file.children(id);
  let mut at = 0;
  while let Some(&child) = children.get(at) {
    at += 1;
    if is_field(file, id, child) {
      continue;
    }
    let Some(node) = Node::of(file, child) else {
      descend(file, child, near, visit);
      continue;
    };
    let node = match near {
      Some(offset) if same_kind(file, node, &children[at..], 8) == 8 => {
        let run = &children[at - 1..at + same_kind(file, node, &children[at..], usize::MAX)];
        at += run.len() - 1;
        let before = run.partition_point(|&it| file.span_of(it).start <= offset);
        match before.checked_sub(1).and_then(|last| Node::of(file, run[last])) {
          Some(last) => last,
          None => continue,
        }
      }
      _ => node,
    };
    if !node.as_expr().is_some_and(Expr::is_missing) {
      visit(node);
    }
  }
}

/// How many of the first `max` ids of `rest` are handles of the same kind as `node`, before one that isn't.
fn same_kind(file: &File, node: Node, rest: &[u32], max: usize) -> usize {
  let kind = std::mem::discriminant(&node);
  let same = |it: &&u32| Node::of(file, **it).is_some_and(|it| std::mem::discriminant(&it) == kind);
  rest.iter().take(max).take_while(same).count()
}

/// True for a child that bun's tree has as a field rather than a node: a key that isn't computed or is a literal in
/// brackets, a module specifier, the `#b` of `a.#b`.
fn is_field(file: &File, parent: u32, child: u32) -> bool {
  use AstKind as K;
  let key = |key: &PropertyKey, computed: bool| {
    key.node_id().index() as u32 == child
      && (!computed || key.as_expression().is_some_and(super::key::is_literal))
  };
  match file.kind(parent) {
    Some(K::ObjectProperty(it)) => key(&it.key, it.computed),
    Some(K::MethodDefinition(it)) => key(&it.key, it.computed),
    Some(K::PropertyDefinition(it)) => key(&it.key, it.computed),
    Some(K::AccessorProperty(it)) => key(&it.key, it.computed),
    Some(K::TSPropertySignature(it)) => key(&it.key, it.computed),
    Some(K::TSMethodSignature(it)) => key(&it.key, it.computed),
    Some(K::BindingProperty(it)) => key(&it.key, it.computed),
    Some(K::AssignmentTargetPropertyProperty(it)) => key(&it.name, it.computed),
    Some(K::TSEnumMember(it)) => file.span_of(child).start == it.id.span().start,
    Some(K::ImportAttribute(it)) => {
      matches!(&it.key, ImportAttributeKey::StringLiteral(key) if key.node_id.get().index() as u32 == child)
    }
    Some(K::PrivateFieldExpression(_)) => matches!(file.kind(child), Some(K::PrivateIdentifier(_))),
    Some(
      K::ImportDeclaration(_)
      | K::ExportFromDeclaration(_)
      | K::ExportAllDeclaration(_)
      | K::TSExternalModuleDeclaration(_)
      | K::TSExternalModuleReference(_),
    ) => matches!(file.kind(child), Some(K::StringLiteral(_))),
    _ => false,
  }
}

/// The node that a type name such as `A.B.C` is part of.
fn name_owner<'a>(file: &File<'a>, mut id: u32) -> Option<AstKind<'a>> {
  loop {
    id = file.parent_id(id);
    match file.kind(id)? {
      AstKind::TSQualifiedName(_) => {}
      owner => return Some(owner),
    }
  }
}

/// `unique T`
fn is_unique(kind: AstKind) -> bool {
  matches!(kind, AstKind::TSTypeOperator(it) if it.operator == TSTypeOperatorOperator::Unique)
}

/// True for a literal in a literal type, such as the `1` of `-1`, and for the specifier of an import type.
fn in_type(file: &File, id: u32) -> bool {
  let parent = file.parent_id(id);
  match file.kind(parent) {
    Some(AstKind::TSLiteralType(_) | AstKind::TSImportType(_)) => true,
    Some(AstKind::UnaryExpression(_)) => {
      matches!(file.kind(file.parent_id(parent)), Some(AstKind::TSLiteralType(_)))
    }
    _ => false,
  }
}
