//! Types: bun's `src/lint/ast/ty.rs` (MIT), over oxc's tree.
//!
//! Where bun's tree differs from oxc's: `(T)` isn't a node, as for expressions, and [`TypeNode::outer_span`] has the
//! parentheses. Only a tuple element with a name, a `?` or a `...` has a node of its own; the [`TupleElem`] of a plain
//! `T` is `T`'s node, so `T`'s parent is the tuple. `...name: T` is one [`TupleElem`]. The `K` of `[K in C]` has no
//! type parameter node: its [`TypeParam`] is the name's node, and `C` is directly in the mapped type
//! ([`Mapped::constraint`]). oxc parses only a name in an interface's `extends`, so it's a `Ref`, and `Heritage`
//! doesn't occur. The attributes of an import declaration are its `with` clause, which has no object literal.

use super::{
  Expr, File, Func, Ident, Keyword, List, MappedModifier, Member, Name, Node, Prop, Span,
  TupleElem, TypeNode, TypeParam, expression_id, skip_trivia, type_id,
};
use oxc_ast::AstKind;
use oxc_ast::ast::{
  Expression, ObjectPropertyKind, TSImportType, TSImportTypeQualifier, TSLiteral,
  TSMappedTypeModifierOperator, TSNamedTupleMember, TSTemplateLiteralType, TSTupleElement, TSType,
  TSTypeName, TSTypeOperatorOperator, TSTypeParameterInstantiation, TSTypePredicateName,
  TSTypeQueryExprName, UnaryOperator, WithClause,
};
use oxc_span::GetSpan;
use smallvec::SmallVec;

#[derive(Copy, Clone, Debug)]
pub(crate) enum TypeKind<'a> {
  /// Syntax the parser failed on.
  Error,
  /// An element of `extends` of an interface that isn't a name, which is an error: `extends f()<Args>`.
  Heritage {
    expr: Expr<'a>,
    args: List<'a, TypeNode<'a>>,
  },
  Keyword(Keyword),
  /// `A.B.C<Args>`
  Ref {
    name: EntityName<'a>,
    args: List<'a, TypeNode<'a>>,
  },
  StringLit(Name<'a>),
  NumberLit(f64),
  BigIntLit {
    text: Name<'a>,
    negative: bool,
  },
  BoolLit(bool),
  /// `` `a${T}b` ``
  Template(List<'a, TypeNode<'a>>),
  /// `T[]`
  Array(TypeNode<'a>),
  Tuple(List<'a, TupleElem<'a>>),
  Union(List<'a, TypeNode<'a>>),
  Intersection(List<'a, TypeNode<'a>>),
  /// `(a: A) => R`, `new (a: A) => R`
  Fn(Func<'a>),
  /// `{ a: A }`
  Object(List<'a, Member<'a>>),
  Cond {
    check: TypeNode<'a>,
    extends: TypeNode<'a>,
    yes: TypeNode<'a>,
    no: TypeNode<'a>,
  },
  Infer(TypeParam<'a>),
  Mapped(Mapped<'a>),
  /// `obj[index]`
  IndexedAccess {
    obj: TypeNode<'a>,
    index: TypeNode<'a>,
  },
  Keyof(TypeNode<'a>),
  Readonly(TypeNode<'a>),
  UniqueSymbol,
  /// `unique T` where `T` isn't the keyword `symbol`, which is an error.
  Unique(TypeNode<'a>),
  /// `typeof a.b.c<Args>`. `expr` is the name as an expression.
  Typeof {
    expr: Expr<'a>,
    args: List<'a, TypeNode<'a>>,
  },
  /// `import("spec").A.B<Args>`, `typeof import("spec")`
  Import {
    spec: Option<Name<'a>>,
    name: EntityName<'a>,
    args: List<'a, TypeNode<'a>>,
    is_typeof: bool,
  },
  /// `x is T`, `asserts x`, `asserts x is T`, `this is T`
  Predicate {
    param: Name<'a>,
    ty: Option<TypeNode<'a>>,
    asserts: bool,
  },
}

/// The kind of a type, without its payload.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum TypeTag {
  Error,
  Heritage,
  Keyword,
  Ref,
  StringLit,
  NumberLit,
  BigIntLit,
  BoolLit,
  Template,
  Array,
  Tuple,
  Union,
  Intersection,
  Fn,
  Object,
  Cond,
  Infer,
  Mapped,
  IndexedAccess,
  Keyof,
  Readonly,
  UniqueSymbol,
  Unique,
  /// `?T`, `T!` and the like, whose kind is `Error`.
  JSDoc,
  Typeof,
  Import,
  Predicate,
}

impl<'a> TypeNode<'a> {
  /// The type `ty`, without parentheses.
  pub(crate) fn of(file: &'a File<'a>, ty: &TSType) -> TypeNode<'a> {
    TypeNode::new(file, type_id(ty))
  }

  fn at(self, ty: &TSType) -> TypeNode<'a> {
    TypeNode::of(self.file, ty)
  }

  fn list(self, types: &[TSType]) -> List<'a, TypeNode<'a>> {
    List::run(self.file, types.first().map(type_id), types.len())
  }

  fn args(self, args: Option<&TSTypeParameterInstantiation>) -> List<'a, TypeNode<'a>> {
    args.map_or_else(|| List::empty(self.file), |it| self.list(&it.params))
  }

  pub(crate) fn kind(self) -> TypeKind<'a> {
    use AstKind as K;
    let file = self.file;
    let Some(raw) = file.kind(self.id) else { return TypeKind::Error };
    match raw {
      K::TSAnyKeyword(_) => TypeKind::Keyword(Keyword::Any),
      K::TSUnknownKeyword(_) => TypeKind::Keyword(Keyword::Unknown),
      K::TSNeverKeyword(_) => TypeKind::Keyword(Keyword::Never),
      K::TSVoidKeyword(_) => TypeKind::Keyword(Keyword::Void),
      K::TSUndefinedKeyword(_) => TypeKind::Keyword(Keyword::Undefined),
      K::TSNullKeyword(_) => TypeKind::Keyword(Keyword::Null),
      K::TSStringKeyword(_) => TypeKind::Keyword(Keyword::String),
      K::TSNumberKeyword(_) => TypeKind::Keyword(Keyword::Number),
      K::TSBooleanKeyword(_) => TypeKind::Keyword(Keyword::Boolean),
      K::TSBigIntKeyword(_) => TypeKind::Keyword(Keyword::BigInt),
      K::TSSymbolKeyword(_) => TypeKind::Keyword(Keyword::Symbol),
      K::TSObjectKeyword(_) => TypeKind::Keyword(Keyword::Object),
      K::TSThisType(_) => TypeKind::Keyword(Keyword::This),
      K::TSIntrinsicKeyword(_) => TypeKind::Keyword(Keyword::Intrinsic),
      K::TSTypeReference(it) => TypeKind::Ref {
        name: EntityName::of(file, &it.type_name),
        args: self.args(it.type_arguments.as_deref()),
      },
      K::TSClassImplements(it) => TypeKind::Ref {
        name: EntityName::of(file, &it.expression),
        args: self.args(it.type_arguments.as_deref()),
      },
      K::TSInterfaceHeritage(it) => TypeKind::Ref {
        name: EntityName::of(file, &it.type_name),
        args: self.args(it.type_arguments.as_deref()),
      },
      K::TSLiteralType(it) => self.literal(&it.literal),
      K::TSTemplateLiteralType(it) => TypeKind::Template(self.list(&it.types)),
      K::TSArrayType(it) => TypeKind::Array(self.at(&it.element_type)),
      K::TSTupleType(it) => {
        let elements = &it.element_types;
        TypeKind::Tuple(List::run(file, elements.first().map(element_id), elements.len()))
      }
      K::TSUnionType(it) => TypeKind::Union(self.list(&it.types)),
      K::TSIntersectionType(it) => TypeKind::Intersection(self.list(&it.types)),
      K::TSFunctionType(_) | K::TSConstructorType(_) => TypeKind::Fn(Func::new(file, self.id)),
      K::TSTypeLiteral(it) => {
        let first = it.members.first().map(|it| it.node_id().index() as u32);
        TypeKind::Object(List::run(file, first, it.members.len()))
      }
      K::TSConditionalType(it) => TypeKind::Cond {
        check: self.at(&it.check_type),
        extends: self.at(&it.extends_type),
        yes: self.at(&it.true_type),
        no: self.at(&it.false_type),
      },
      K::TSInferType(it) => {
        TypeKind::Infer(TypeParam::new(file, it.type_parameter.node_id.get().index() as u32))
      }
      K::TSMappedType(_) => TypeKind::Mapped(Mapped { file, id: self.id }),
      K::TSIndexedAccessType(it) => {
        TypeKind::IndexedAccess { obj: self.at(&it.object_type), index: self.at(&it.index_type) }
      }
      K::TSTypeOperator(it) => match (it.operator, &it.type_annotation) {
        (TSTypeOperatorOperator::Keyof, ty) => TypeKind::Keyof(self.at(ty)),
        (TSTypeOperatorOperator::Readonly, ty) => TypeKind::Readonly(self.at(ty)),
        (TSTypeOperatorOperator::Unique, TSType::TSSymbolKeyword(_)) => TypeKind::UniqueSymbol,
        (TSTypeOperatorOperator::Unique, ty) => TypeKind::Unique(self.at(ty)),
      },
      K::TSTypeQuery(it) => match &it.expr_name {
        TSTypeQueryExprName::TSImportType(import) => self.import(import, true),
        name => TypeKind::Typeof {
          expr: Expr::new(file, name.node_id().index() as u32),
          args: self.args(it.type_arguments.as_deref()),
        },
      },
      K::TSImportType(it) => self.import(it, false),
      K::TSTypePredicate(it) => TypeKind::Predicate {
        param: match &it.parameter_name {
          TSTypePredicateName::Identifier(name) => Name(name.name.as_str()),
          TSTypePredicateName::This(_) => Name("this"),
        },
        ty: it.type_annotation.as_ref().map(|it| self.at(&it.type_annotation)),
        asserts: it.asserts,
      },
      _ => TypeKind::Error,
    }
  }

  fn literal(self, literal: &'a TSLiteral<'a>) -> TypeKind<'a> {
    match literal {
      TSLiteral::BooleanLiteral(it) => TypeKind::BoolLit(it.value),
      TSLiteral::NumericLiteral(it) => TypeKind::NumberLit(it.value),
      TSLiteral::BigIntLiteral(it) => {
        TypeKind::BigIntLit { text: Name(it.raw.map_or("", |raw| raw.as_str())), negative: false }
      }
      TSLiteral::StringLiteral(it) => {
        let text = self.file.str(span(it.span));
        let quoted = text.get(1..text.len().saturating_sub(1)).unwrap_or_default();
        TypeKind::StringLit(Name(it.value.as_str().unwrap_or(quoted)))
      }
      TSLiteral::TemplateLiteral(it) => {
        let cooked = it.quasis.first().and_then(|it| it.value.cooked).and_then(|it| it.as_str());
        TypeKind::StringLit(Name(cooked.unwrap_or_default()))
      }
      TSLiteral::UnaryExpression(it) => match (&it.argument, it.operator) {
        (Expression::NumericLiteral(number), UnaryOperator::UnaryNegation) => {
          TypeKind::NumberLit(-number.value)
        }
        (Expression::BigIntLiteral(big), UnaryOperator::UnaryNegation) => {
          TypeKind::BigIntLit { text: Name(big.raw.map_or("", |raw| raw.as_str())), negative: true }
        }
        _ => TypeKind::Error,
      },
    }
  }

  fn import(self, import: &'a TSImportType<'a>, is_typeof: bool) -> TypeKind<'a> {
    let names = import.qualifier.as_ref().map(Names::Qualifier);
    TypeKind::Import {
      spec: import.source.value.as_str().map(Name),
      name: EntityName { file: self.file, names },
      args: self.args(import.type_arguments.as_deref()),
      is_typeof,
    }
  }

  pub(crate) fn tag(self) -> TypeTag {
    if let Some(
      AstKind::JSDocNullableType(_)
      | AstKind::JSDocNonNullableType(_)
      | AstKind::JSDocUnknownType(_),
    ) = self.file.kind(self.id)
    {
      return TypeTag::JSDoc;
    }
    match self.kind() {
      TypeKind::Error => TypeTag::Error,
      TypeKind::Heritage { .. } => TypeTag::Heritage,
      TypeKind::Keyword(_) => TypeTag::Keyword,
      TypeKind::Ref { .. } => TypeTag::Ref,
      TypeKind::StringLit(_) => TypeTag::StringLit,
      TypeKind::NumberLit(_) => TypeTag::NumberLit,
      TypeKind::BigIntLit { .. } => TypeTag::BigIntLit,
      TypeKind::BoolLit(_) => TypeTag::BoolLit,
      TypeKind::Template(_) => TypeTag::Template,
      TypeKind::Array(_) => TypeTag::Array,
      TypeKind::Tuple(_) => TypeTag::Tuple,
      TypeKind::Union(_) => TypeTag::Union,
      TypeKind::Intersection(_) => TypeTag::Intersection,
      TypeKind::Fn(_) => TypeTag::Fn,
      TypeKind::Object(_) => TypeTag::Object,
      TypeKind::Cond { .. } => TypeTag::Cond,
      TypeKind::Infer(_) => TypeTag::Infer,
      TypeKind::Mapped(_) => TypeTag::Mapped,
      TypeKind::IndexedAccess { .. } => TypeTag::IndexedAccess,
      TypeKind::Keyof(_) => TypeTag::Keyof,
      TypeKind::Readonly(_) => TypeTag::Readonly,
      TypeKind::UniqueSymbol => TypeTag::UniqueSymbol,
      TypeKind::Unique(_) => TypeTag::Unique,
      TypeKind::Typeof { .. } => TypeTag::Typeof,
      TypeKind::Import { .. } => TypeTag::Import,
      TypeKind::Predicate { .. } => TypeTag::Predicate,
    }
  }

  /// Includes all the parentheses around it.
  pub(crate) fn outer_span(self) -> Span {
    self.file.parens(self.id).last().map_or_else(|| self.span(), |it| it.1)
  }

  pub(crate) fn is_parenthesized(self) -> bool {
    !self.file.parens(self.id).is_empty()
  }

  /// The range of ESLint's `TSTypeAnnotation` around it, if it's a return type or the type of a variable, parameter
  /// or property. It runs from the `:`, or the `=>` of a function type, to the end of the type and its parentheses.
  pub(crate) fn annotation_span(self) -> Span {
    match self.file.kind(self.file.parent_id(self.id)) {
      Some(AstKind::TSTypeAnnotation(it)) => span(it.span),
      _ => self.outer_span(),
    }
  }

  pub(crate) fn parent(self) -> Node<'a> {
    Node::Type(self).parent()
  }

  pub(crate) fn is_keyword(self, keyword: Keyword) -> bool {
    self.as_keyword() == Some(keyword)
  }

  pub(crate) fn as_keyword(self) -> Option<Keyword> {
    match self.kind() {
      TypeKind::Keyword(keyword) => Some(keyword),
      _ => None,
    }
  }

  /// The `name` and `args` of a `Ref`.
  pub(crate) fn as_ref(self) -> Option<(EntityName<'a>, List<'a, TypeNode<'a>>)> {
    match self.kind() {
      TypeKind::Ref { name, args } => Some((name, args)),
      _ => None,
    }
  }

  /// The pieces of a `Template`.
  pub(crate) fn as_template(self) -> Option<TypeTemplate<'a>> {
    let template = TypeTemplate { ty: self };
    template.template().map(|_| template)
  }

  /// The `x` of the `Predicate` `x is T` or `asserts x`, with its position. It can be `this`.
  pub(crate) fn predicate_param(self) -> Option<Ident<'a>> {
    let AstKind::TSTypePredicate(it) = self.file.kind(self.id)? else { return None };
    Some(match &it.parameter_name {
      TSTypePredicateName::Identifier(name) => {
        Ident { name: Name(name.name.as_str()), span: span(name.span), string: false }
      }
      TSTypePredicateName::This(this) => {
        Ident { name: Name("this"), span: span(this.span), string: false }
      }
    })
  }

  /// The `symbol` of `unique symbol`.
  pub(crate) fn unique_symbol_keyword_span(self) -> Option<Span> {
    match self.file.kind(self.id)? {
      AstKind::TSTypeOperator(it) if it.operator == TSTypeOperatorOperator::Unique => {
        match &it.type_annotation {
          TSType::TSSymbolKeyword(symbol) => Some(span(symbol.span)),
          _ => None,
        }
      }
      _ => None,
    }
  }

  fn import_type(self) -> Option<&'a TSImportType<'a>> {
    match self.file.kind(self.id)? {
      AstKind::TSImportType(it) => Some(it),
      AstKind::TSTypeQuery(it) => match &it.expr_name {
        TSTypeQueryExprName::TSImportType(it) => Some(it),
        _ => None,
      },
      _ => None,
    }
  }

  /// For an `Import`: starts at the `import`, skipping any `typeof` before it. In ESLint, `typeof import("m")` is a
  /// `TSTypeQuery` of a `TSImportType` with this range.
  pub(crate) fn import_span(self) -> Option<Span> {
    self.import_type().map(|it| span(it.span))
  }

  /// For an `Import`: the module specifier with its quotes.
  pub(crate) fn import_source_span(self) -> Option<Span> {
    self.import_type().map(|it| span(it.source.span))
  }

  /// For an `Import`: the `with: { .. }` of `import("m", { with: { .. } })`.
  pub(crate) fn import_attributes(self) -> Option<ImportAttributes<'a>> {
    let options = self.import_type()?.options.as_deref()?;
    let ObjectPropertyKind::ObjectProperty(property) = options.properties.first()? else {
      return None;
    };
    Some(ImportAttributes {
      file: self.file,
      keyword: property.key.span().start,
      id: expression_id(&property.value),
    })
  }
}

/// `` `text${T}text` `` in a type.
#[derive(Copy, Clone, Debug)]
pub(crate) struct TypeTemplate<'a> {
  ty: TypeNode<'a>,
}

impl<'a> TypeTemplate<'a> {
  fn template(self) -> Option<&'a TSTemplateLiteralType<'a>> {
    match self.ty.file.kind(self.ty.id)? {
      AstKind::TSTemplateLiteralType(it) => Some(it),
      _ => None,
    }
  }

  /// The substitutions.
  pub(crate) fn types(self) -> List<'a, TypeNode<'a>> {
    self.template().map_or_else(|| List::empty(self.ty.file), |it| self.ty.list(&it.types))
  }

  /// The number of text pieces, which is one more than the number of substitutions.
  pub(crate) fn quasi_count(self) -> usize {
    self.template().map_or(1, |it| it.quasis.len())
  }

  /// The cooked value of text piece `i`.
  pub(crate) fn cooked(self, i: usize) -> Option<Name<'a>> {
    let quasi = self.template()?.quasis.get(i)?;
    quasi.value.cooked.and_then(|cooked| cooked.as_str()).map(Name)
  }

  /// Text piece `i` with its delimiters: `` `a${ ``, `}b${`, `` }c` ``.
  pub(crate) fn quasi_span(self, i: usize) -> Span {
    let Some(quasi) = self.template().and_then(|it| it.quasis.get(i)) else {
      return Span::default();
    };
    let end = if quasi.tail { quasi.span.end + 1 } else { quasi.span.end + 2 };
    Span::new(quasi.span.start - 1, end)
  }

  /// The source text of piece `i`, without its delimiters.
  pub(crate) fn raw(self, i: usize) -> &'a [u8] {
    match self.template().and_then(|it| it.quasis.get(i)) {
      Some(quasi) => quasi.value.raw.as_str().as_bytes(),
      None => b"",
    }
  }
}

impl<'a> List<'a, TypeNode<'a>> {
  /// The `<..>` around type arguments, which is the range of ESLint's `TSTypeParameterInstantiation`. `None` if the
  /// list is empty.
  pub(crate) fn angle_brackets_span(self) -> Option<Span> {
    let first = self.first()?;
    match first.file.kind(first.file.parent_id(first.id))? {
      AstKind::TSTypeParameterInstantiation(it) => Some(span(it.span)),
      _ => None,
    }
  }
}

impl<'a> List<'a, TypeParam<'a>> {
  /// The `<..>` around type parameters, which is the range of ESLint's `TSTypeParameterDeclaration`. `None` if the
  /// list is empty.
  pub(crate) fn angle_brackets_span(self) -> Option<Span> {
    let first = self.first()?;
    match first.file.kind(first.file.parent_id(first.id))? {
      AstKind::TSTypeParameterDeclaration(it) => Some(span(it.span)),
      _ => None,
    }
  }
}

/// `with { type: "json" }` after the module specifier of an import or export, or the `with: { .. }` in the second
/// argument of an import type.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ImportAttributes<'a> {
  file: &'a File<'a>,
  keyword: u32,
  /// The `{ .. }` of an import type, or the `with` clause of a declaration.
  id: u32,
}

impl<'a> ImportAttributes<'a> {
  /// The `with` clause of an import or export declaration.
  pub(crate) fn clause(file: &'a File<'a>, clause: &WithClause) -> Self {
    ImportAttributes { file, keyword: clause.span.start, id: clause.node_id.get().index() as u32 }
  }

  /// The `{ type: "json" }`. For a declaration, it's the `with` clause.
  pub(crate) fn object(self) -> Expr<'a> {
    Expr::new(self.file, self.id)
  }

  /// `with`, or the deprecated `assert`.
  pub(crate) fn keyword_span(self) -> Span {
    let rest = self.file.text.as_bytes().get(self.keyword as usize..).unwrap_or_default();
    let len = rest
      .iter()
      .take_while(|it| it.is_ascii_alphanumeric() || **it == b'_' || **it == b'$')
      .count();
    Span::new(self.keyword, self.keyword + len as u32)
  }

  /// The `{ .. }`.
  pub(crate) fn braces_span(self) -> Span {
    match self.file.kind(self.id) {
      Some(AstKind::WithClause(it)) => {
        Span::new(skip_trivia(self.file.text.as_bytes(), self.keyword_span().end), it.span.end)
      }
      _ => self.object().span(),
    }
  }

  /// In an import type: the second argument, `{ with: { .. } }`.
  pub(crate) fn options_span(self) -> Span {
    let property = self.file.parent_id(self.id);
    match self.file.kind(self.file.parent_id(property)) {
      Some(AstKind::ObjectExpression(it)) => span(it.span),
      _ => self.braces_span(),
    }
  }

  /// `key: "value"`. The key is a name or a string.
  pub(crate) fn entries(self) -> List<'a, Prop<'a>> {
    match self.file.kind(self.id) {
      Some(AstKind::WithClause(it)) => {
        let first = it.with_entries.first().map(|it| it.node_id.get().index() as u32);
        List::run(self.file, first, it.with_entries.len())
      }
      _ => self.object().as_object().unwrap_or_else(|| List::empty(self.file)),
    }
  }
}

/// `A`, `A.B.C`: a name in a type, which isn't an expression.
#[derive(Copy, Clone)]
pub(crate) struct EntityName<'a> {
  file: &'a File<'a>,
  names: Option<Names<'a>>,
}

/// The oxc node of an [`EntityName`].
#[derive(Copy, Clone)]
enum Names<'a> {
  Type(&'a TSTypeName<'a>),
  Qualifier(&'a TSImportTypeQualifier<'a>),
}

impl std::fmt::Debug for EntityName<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_list().entries(self.parts()).finish()
  }
}

impl<'a> EntityName<'a> {
  pub(crate) fn of(file: &'a File<'a>, name: &'a TSTypeName<'a>) -> Self {
    EntityName { file, names: Some(Names::Type(name)) }
  }

  fn collect(self) -> SmallVec<[Ident<'a>; 4]> {
    let mut out = SmallVec::new();
    match self.names {
      Some(Names::Type(name)) => type_name(name, &mut out),
      Some(Names::Qualifier(name)) => qualifier(name, &mut out),
      None => {}
    }
    out
  }

  /// The names between the dots.
  pub(crate) fn parts(self) -> impl DoubleEndedIterator<Item = Ident<'a>> + ExactSizeIterator + 'a {
    self.collect().into_iter()
  }

  /// The name at index `i`, counting from the left.
  pub(crate) fn get(self, i: usize) -> Option<Ident<'a>> {
    self.collect().get(i).copied()
  }

  pub(crate) fn len(self) -> usize {
    self.collect().len()
  }

  pub(crate) fn is_empty(self) -> bool {
    self.names.is_none()
  }

  /// The `A` of `A.B.C`, which is the part that's looked up in the scope.
  pub(crate) fn first(self) -> Option<Ident<'a>> {
    self.parts().next()
  }

  pub(crate) fn last(self) -> Option<Ident<'a>> {
    self.parts().next_back()
  }

  /// The name, if there are no dots.
  pub(crate) fn as_ident(self) -> Option<Ident<'a>> {
    let parts = self.collect();
    (parts.len() == 1).then(|| parts[0])
  }

  /// True if it's the single name `name`.
  pub(crate) fn is(self, name: &str) -> bool {
    self.as_ident().is_some_and(|it| it.name().is(name))
  }

  pub(crate) fn span(self) -> Span {
    match (self.first(), self.last()) {
      (Some(first), Some(last)) => first.span().to(last.span()),
      _ => Span::default(),
    }
  }
}

fn ident<'a>(name: &'a str, at: oxc_span::Span) -> Ident<'a> {
  Ident { name: Name(name), span: span(at), string: false }
}

fn type_name<'a>(name: &'a TSTypeName<'a>, out: &mut SmallVec<[Ident<'a>; 4]>) {
  match name {
    TSTypeName::IdentifierReference(it) => out.push(ident(it.name.as_str(), it.span)),
    TSTypeName::QualifiedName(it) => {
      type_name(&it.left, out);
      out.push(ident(it.right.name.as_str(), it.right.span));
    }
    TSTypeName::ThisExpression(it) => out.push(ident("this", it.span)),
  }
}

fn qualifier<'a>(name: &'a TSImportTypeQualifier<'a>, out: &mut SmallVec<[Ident<'a>; 4]>) {
  match name {
    TSImportTypeQualifier::Identifier(it) => out.push(ident(it.name.as_str(), it.span)),
    TSImportTypeQualifier::QualifiedName(it) => {
      qualifier(&it.left, out);
      out.push(ident(it.right.name.as_str(), it.right.span));
    }
  }
}

/// The id of a tuple element: its type's for a plain type.
fn element_id(element: &TSTupleElement) -> u32 {
  match element.as_ts_type() {
    Some(ty) => type_id(ty),
    None => element.node_id().index() as u32,
  }
}

impl<'a> TupleElem<'a> {
  /// `name: T`, also in `...name: T`.
  fn named(self) -> Option<&'a TSNamedTupleMember<'a>> {
    match self.file.kind(self.id)? {
      AstKind::TSNamedTupleMember(it) => Some(it),
      AstKind::TSRestType(it) => match &it.type_annotation {
        TSType::TSNamedTupleMember(it) => Some(it),
        _ => None,
      },
      _ => None,
    }
  }

  /// The `T`.
  pub(crate) fn ty(self) -> TypeNode<'a> {
    let id = match (self.named(), self.file.kind(self.id)) {
      (Some(named), _) => element_id(&named.element_type),
      (None, Some(AstKind::TSOptionalType(it))) => type_id(&it.type_annotation),
      (None, Some(AstKind::TSRestType(it))) => type_id(&it.type_annotation),
      _ => self.id,
    };
    TypeNode::new(self.file, id)
  }

  pub(crate) fn name(self) -> Option<Ident<'a>> {
    self.named().map(|it| ident(it.label.name.as_str(), it.label.span))
  }

  pub(crate) fn is_optional(self) -> bool {
    matches!(self.file.kind(self.id), Some(AstKind::TSOptionalType(_)))
      || self.named().is_some_and(|it| it.optional)
  }

  /// It starts with `...`.
  pub(crate) fn is_rest(self) -> bool {
    matches!(self.file.kind(self.id), Some(AstKind::TSRestType(_)))
  }

  /// The tuple type.
  pub(crate) fn parent(self) -> Node<'a> {
    Node::TupleElem(self).parent()
  }
}

/// `{ readonly [K in C as N]?: T }`
#[derive(Copy, Clone)]
pub(crate) struct Mapped<'a> {
  file: &'a File<'a>,
  /// The mapped type's.
  id: u32,
}

impl std::fmt::Debug for Mapped<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Mapped({})", self.id)
  }
}

impl<'a> Mapped<'a> {
  fn mapped(self) -> Option<&'a oxc_ast::ast::TSMappedType<'a>> {
    match self.file.kind(self.id)? {
      AstKind::TSMappedType(it) => Some(it),
      _ => None,
    }
  }

  /// The `K in C`, by the `K`.
  pub(crate) fn param(self) -> TypeParam<'a> {
    let id = self.mapped().map_or(super::NONE, |it| it.key.node_id.get().index() as u32);
    TypeParam::new(self.file, id)
  }

  /// The `C`.
  pub(crate) fn constraint(self) -> Option<TypeNode<'a>> {
    self.mapped().map(|it| TypeNode::of(self.file, &it.constraint))
  }

  /// The `N`.
  pub(crate) fn name_type(self) -> Option<TypeNode<'a>> {
    self.mapped()?.name_type.as_ref().map(|it| TypeNode::of(self.file, it))
  }

  /// The `T`.
  pub(crate) fn ty(self) -> Option<TypeNode<'a>> {
    self.mapped()?.type_annotation.as_ref().map(|it| TypeNode::of(self.file, it))
  }

  pub(crate) fn readonly(self) -> MappedModifier {
    modifier(self.mapped().and_then(|it| it.readonly))
  }

  pub(crate) fn optional(self) -> MappedModifier {
    modifier(self.mapped().and_then(|it| it.optional))
  }

  /// `+readonly`, as opposed to `readonly`.
  pub(crate) fn is_readonly_with_plus(self) -> bool {
    self.mapped().and_then(|it| it.readonly) == Some(TSMappedTypeModifierOperator::Plus)
  }

  /// `+?`, as opposed to `?`.
  pub(crate) fn is_optional_with_plus(self) -> bool {
    self.mapped().and_then(|it| it.optional) == Some(TSMappedTypeModifierOperator::Plus)
  }
}

fn modifier(operator: Option<TSMappedTypeModifierOperator>) -> MappedModifier {
  match operator {
    None => MappedModifier::None,
    Some(TSMappedTypeModifierOperator::True | TSMappedTypeModifierOperator::Plus) => {
      MappedModifier::Add
    }
    Some(TSMappedTypeModifierOperator::Minus) => MappedModifier::Remove,
  }
}

impl<'a> TypeParam<'a> {
  /// The mapped type whose `K` this is.
  pub(crate) fn mapped(self) -> Option<Mapped<'a>> {
    let id = self.file.parent_id(self.id);
    match (self.file.kind(self.id)?, self.file.kind(id)?) {
      (AstKind::BindingIdentifier(_), AstKind::TSMappedType(_)) => {
        Some(Mapped { file: self.file, id })
      }
      _ => None,
    }
  }
}

fn span(span: oxc_span::Span) -> Span {
  Span::new(span.start, span.end)
}
