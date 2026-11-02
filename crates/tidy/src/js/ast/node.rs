//! Handles, and [`Node`], which is any of them: which handle each of oxc's nodes is, and its parent as bun's tree has
//! it.

use super::{File, NONE, Span};
use oxc_ast::AstKind;
use oxc_span::GetSpan;

/// Declares handles: a file and the id of a node in it.
macro_rules! handles {
  ($($(#[$doc:meta])* $name:ident,)*) => {
    $(
      $(#[$doc])*
      #[derive(Copy, Clone)]
      pub(crate) struct $name<'a> {
        pub(crate) file: &'a File<'a>,
        pub(crate) id: u32,
      }

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
          let span = self.raw().span();
          Span::new(span.start, span.end)
        }

        /// The node's source text.
        #[inline]
        pub(crate) fn text(self) -> &'a str {
          self.file.slice(self.span())
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

      impl<'a> From<$name<'a>> for Node<'a> {
        #[inline]
        fn from(it: $name<'a>) -> Node<'a> {
          Node::$name(it)
        }
      }

      impl super::Spanned for $name<'_> {
        #[inline]
        fn span(&self) -> Span {
          $name::span(*self)
        }
      }
    )*

    /// Any node of a file.
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    pub(crate) enum Node<'a> {
      /// The root.
      File(&'a File<'a>),
      $($name($name<'a>),)*
    }

    impl<'a> Node<'a> {
      pub(crate) fn file(self) -> &'a File<'a> {
        match self {
          Node::File(file) => file,
          $(Node::$name(it) => it.file,)*
        }
      }

      /// The id of its node; the program's for the file.
      pub(crate) fn id(self) -> u32 {
        match self {
          Node::File(_) => 0,
          $(Node::$name(it) => it.id,)*
        }
      }
    }
  };
}

handles! {
  Expr,
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
    let node = match kind {
      K::Program(_) => Node::File(file),
      K::Function(function) => match parent() {
        Some(K::MethodDefinition(_)) => Node::Func(Func::new(file, id)),
        _ if function.is_declaration() => Node::Stmt(Stmt::new(file, id)),
        _ => Node::Expr(Expr::new(file, id)),
      },
      K::Class(class) if class.is_declaration() => Node::Stmt(Stmt::new(file, id)),
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
      | K::AssignmentTargetRest(_)
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
      K::SpreadElement(_) => match parent() {
        Some(K::ObjectExpression(_)) => Node::Prop(Prop::new(file, id)),
        _ => Node::Expr(Expr::new(file, id)),
      },
      K::ObjectProperty(_)
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
        _ => Node::Pat(Pat::new(file, id)),
      },
      K::AssignmentPattern(_)
      | K::ObjectPattern(_)
      | K::ArrayPattern(_)
      | K::BindingRestElement(_)
      | K::FormalParameterRest(_) => Node::Pat(Pat::new(file, id)),
      K::BindingProperty(_) => Node::PatProp(PatProp::new(file, id)),
      K::FormalParameter(_) | K::TSThisParameter(_) => Node::Param(Param::new(file, id)),
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
      K::TSNamedTupleMember(_) | K::TSOptionalType(_) | K::TSRestType(_)
        if matches!(parent(), Some(K::TSTupleType(_))) =>
      {
        Node::TupleElem(TupleElem::new(file, id))
      }
      K::TSLiteralType(_)
      | K::TSConditionalType(_)
      | K::TSUnionType(_)
      | K::TSIntersectionType(_)
      | K::TSParenthesizedType(_)
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
      | K::JSDocNullableType(_)
      | K::JSDocNonNullableType(_)
      | K::JSDocUnknownType(_) => Node::TypeNode(TypeNode::new(file, id)),
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

  /// Its parent, grandparent and so on, ending with the file.
  pub(crate) fn ancestors(self) -> Ancestors<'a> {
    Ancestors { at: Some(self) }
  }
}

#[derive(Copy, Clone)]
pub(crate) struct Ancestors<'a> {
  at: Option<Node<'a>>,
}

impl<'a> Iterator for Ancestors<'a> {
  type Item = Node<'a>;

  fn next(&mut self) -> Option<Node<'a>> {
    match self.at? {
      Node::File(_) => {
        self.at = None;
        None
      }
      at => {
        let parent = at.parent();
        self.at = Some(parent);
        Some(parent)
      }
    }
  }
}
