//! Functions, classes and other declarations, and their parts: bun's `src/lint/ast/decl.rs` (MIT), over oxc's tree.
//!
//! Where bun's tree differs from oxc's:
//! - Keywords such as `export`, `async`, `static` and `private` aren't nodes in oxc's tree but fields of the node they
//!   modify. A [`Modifier`] is a value, not a handle: a decorator, or a keyword found in the text from the node's
//!   start and the flags oxc has for it.
//! - `span()` is oxc's span of the node. bun's `Func::span` and `Class::span`, which start at the first decorator or
//!   modifier and include an `export`, are [`Func::full_span`] and [`Class::full_span`]. The `Func` of a method is
//!   oxc's `Function`, whose span starts at the type parameters or the `(`, as [`Func::span_from_params`] does.
//! - The `this` parameter and the parameter of an index signature have no pattern node: their [`Param::pat`] is the
//!   parameter itself, a `TSThisParameter` or a `TSIndexSignatureName`.
//! - The `a = 1` of the destructuring assignment `{ a = 1 } = o` isn't an `Assign` expression: [`Prop::value`] is
//!   the `a` and [`Prop::default`] the `1`.
//! - `static constructor() {}` is a `Constructor`, as in bun, though oxc parses a method.
//! - `AMBIENT` is the declaration's own `declare`, not one around it. `MISSING_BODY` never occurs: oxc doesn't recover
//!   from a missing `{`. Nor does `a?() {}` in an object literal, so [`Prop::is_optional`] is false.
//! - In brackets, an enum member's name is a string or a template without substitutions, so
//!   [`EnumMember::computed_key`] is `None`.
//! - Nothing of bun's binder: no `Func::enclosing`, `returns`, `yields` or `has_implicit_return`.
//! - The `A.B` of `import a = A.B` is a `TSModuleReference` in oxc, not a `TSTypeName`, so
//!   [`ImportEquals::target`] copies it into the arena as one for its [`EntityName`]. It has no node id of its own.

use super::{
  Alias, Class, EntityName, Enum, EnumMember, Export, ExportSpec, Expr, File, Flags, FnKind, Func,
  Handle, Ident, Import, ImportAttributes, ImportEquals, ImportSpec, Interface, Key, KeyKind, List,
  Member, MemberKind, Module, Name, Node, Param, Pat, Prop, PropKind, Span, Stmt, TypeNode,
  TypeParam, expression_id, skip_trivia, target_id, type_id,
};
use oxc_allocator::CloneIn;
use oxc_ast::AstKind;
use oxc_ast::ast::{
  ArrowFunctionBody, AssignmentTargetMaybeDefault, BindingIdentifier, Decorator, Directive,
  FormalParameters, ImportAttributeKey, ImportDeclarationSpecifier, JSXAttributeValue,
  MethodDefinitionKind, ModuleExportName, PropertyKind, Statement, StringLiteral, TSAccessibility,
  TSEnumMemberName, TSMethodSignatureKind, TSModuleReference, TSNamespaceDeclarationBody,
  TSNamespaceDeclarationKind, TSThisParameter, TSTypeAnnotation, TSTypeName,
  TSTypeParameterDeclaration,
};
use oxc_span::GetSpan;
use smallvec::SmallVec;

use AstKind as K;

#[derive(Copy, Clone, Debug)]
pub(crate) enum FnBody<'a> {
  /// An overload, an ambient or abstract declaration, a signature.
  None,
  Block(List<'a, Stmt<'a>>),
  /// An arrow function's expression body.
  Expr(Expr<'a>),
}

/// The parts of a function's head, from whichever oxc node it is.
#[derive(Default)]
struct Head<'a> {
  types: Option<&'a TSTypeParameterDeclaration<'a>>,
  this: Option<&'a TSThisParameter<'a>>,
  params: Option<&'a FormalParameters<'a>>,
  ret: Option<&'a TSTypeAnnotation<'a>>,
}

impl<'a> Func<'a> {
  fn head(self) -> Head<'a> {
    match self.raw() {
      K::Function(it) => Head {
        types: it.type_parameters.as_deref(),
        this: it.this_param.as_deref(),
        params: Some(&it.params),
        ret: it.return_type.as_deref(),
      },
      K::ArrowFunctionExpression(it) => Head {
        types: it.type_parameters.as_deref(),
        this: None,
        params: Some(&it.params),
        ret: it.return_type.as_deref(),
      },
      K::TSMethodSignature(it) => Head {
        types: it.type_parameters.as_deref(),
        this: it.this_param.as_deref(),
        params: Some(&it.params),
        ret: it.return_type.as_deref(),
      },
      K::TSCallSignatureDeclaration(it) => Head {
        types: it.type_parameters.as_deref(),
        this: it.this_param.as_deref(),
        params: Some(&it.params),
        ret: it.return_type.as_deref(),
      },
      K::TSConstructSignatureDeclaration(it) => Head {
        types: it.type_parameters.as_deref(),
        this: None,
        params: Some(&it.params),
        ret: it.return_type.as_deref(),
      },
      K::TSFunctionType(it) => Head {
        types: it.type_parameters.as_deref(),
        this: it.this_param.as_deref(),
        params: Some(&it.params),
        ret: Some(&it.return_type),
      },
      K::TSConstructorType(it) => Head {
        types: it.type_parameters.as_deref(),
        this: None,
        params: Some(&it.params),
        ret: Some(&it.return_type),
      },
      K::TSIndexSignature(it) => Head { ret: Some(&it.type_annotation), ..Head::default() },
      _ => Head::default(),
    }
  }

  pub(crate) fn kind(self) -> FnKind {
    match self.raw() {
      K::ArrowFunctionExpression(_) => FnKind::Arrow,
      K::StaticBlock(_) => FnKind::StaticBlock,
      K::TSCallSignatureDeclaration(_) => FnKind::CallSignature,
      K::TSConstructSignatureDeclaration(_) => FnKind::ConstructSignature,
      K::TSFunctionType(_) => FnKind::FunctionType,
      K::TSConstructorType(_) => FnKind::ConstructorType,
      K::TSIndexSignature(_) => FnKind::IndexSignature,
      K::TSMethodSignature(it) => match it.kind {
        TSMethodSignatureKind::Method => FnKind::Method,
        TSMethodSignatureKind::Get => FnKind::Getter,
        TSMethodSignatureKind::Set => FnKind::Setter,
      },
      K::Function(it) => {
        let parent = self.file.parent_id(self.id);
        match self.file.kind(parent) {
          Some(K::MethodDefinition(_)) => match Member::new(self.file, parent).kind() {
            MemberKind::Constructor => FnKind::Constructor,
            MemberKind::Getter => FnKind::Getter,
            MemberKind::Setter => FnKind::Setter,
            _ => FnKind::Method,
          },
          Some(K::ObjectProperty(property)) if property.method => FnKind::Method,
          Some(K::ObjectProperty(property)) if property.kind == PropertyKind::Get => FnKind::Getter,
          Some(K::ObjectProperty(property)) if property.kind == PropertyKind::Set => FnKind::Setter,
          _ if it.is_declaration() => FnKind::Decl,
          _ => FnKind::Expr,
        }
      }
      _ => FnKind::Expr,
    }
  }

  /// `ASYNC`, `GENERATOR`, and the declaration's modifiers.
  pub(crate) fn flags(self) -> Flags {
    let mut flags = match self.owner() {
      Node::Member(member) => member.keywords(),
      _ => Flags::empty(),
    };
    match self.raw() {
      K::Function(it) => {
        flags.set(Flags::ASYNC, it.r#async);
        flags.set(Flags::GENERATOR, it.generator);
        if it.declare {
          flags |= Flags::AMBIENT;
        }
        if it.is_declaration() {
          flags |= exports(self.file, self.id);
        }
      }
      K::ArrowFunctionExpression(it) => flags.set(Flags::ASYNC, it.r#async),
      K::TSConstructorType(it) => flags.set(Flags::ABSTRACT, it.r#abstract),
      _ => {}
    }
    flags
  }

  pub(crate) fn is_async(self) -> bool {
    self.flags().contains(Flags::ASYNC)
  }

  pub(crate) fn is_generator(self) -> bool {
    self.flags().contains(Flags::GENERATOR)
  }

  pub(crate) fn is_arrow(self) -> bool {
    self.kind() == FnKind::Arrow
  }

  /// The name of a function declaration or expression. A method's name is its owner's [`Member::key`] or
  /// [`Prop::key`].
  pub(crate) fn name(self) -> Option<Ident<'a>> {
    match self.raw() {
      K::Function(it) => it.id.as_ref().map(binding),
      _ => None,
    }
  }

  pub(crate) fn type_params(self) -> List<'a, TypeParam<'a>> {
    type_params(self.file, self.head().types)
  }

  /// Excludes the `this` parameter.
  pub(crate) fn params(self) -> List<'a, Param<'a>> {
    if let K::TSIndexSignature(it) = self.raw() {
      return List::run(self.file, Some(it.parameter.node_id().index() as u32), 1);
    }
    let Some(params) = self.head().params else { return List::empty(self.file) };
    let rest = params.rest.as_ref().map(|it| it.node_id().index() as u32);
    let first = params.items.first().map(|it| it.node_id().index() as u32).or(rest);
    List::run(self.file, first, params.items.len() + usize::from(rest.is_some()))
  }

  /// `this: T`, which always comes first.
  pub(crate) fn this_param(self) -> Option<Param<'a>> {
    let this = self.head().this?;
    Some(Param::new(self.file, this.node_id().index() as u32))
  }

  pub(crate) fn return_type(self) -> Option<TypeNode<'a>> {
    annotation(self.file, self.head().ret)
  }

  pub(crate) fn body(self) -> FnBody<'a> {
    let file = self.file;
    match self.raw() {
      K::Function(it) => match &it.body {
        Some(body) => FnBody::Block(block(file, &body.directives, &body.statements)),
        None => FnBody::None,
      },
      K::ArrowFunctionExpression(it) => match &it.body {
        ArrowFunctionBody::FunctionBody(body) => {
          FnBody::Block(block(file, &body.directives, &body.statements))
        }
        body => match body.as_expression() {
          Some(expression) => FnBody::Expr(Expr::new(file, expression_id(expression))),
          None => FnBody::None,
        },
      },
      K::StaticBlock(it) => FnBody::Block(block(file, &[], &it.body)),
      _ => FnBody::None,
    }
  }

  pub(crate) fn has_body(self) -> bool {
    !matches!(self.body(), FnBody::None)
  }

  /// The statements of a block body.
  pub(crate) fn body_statements(self) -> Option<List<'a, Stmt<'a>>> {
    match self.body() {
      FnBody::Block(statements) => Some(statements),
      _ => None,
    }
  }

  /// The `{ .. }` of a block body.
  pub(crate) fn body_span(self) -> Option<Span> {
    match self.raw() {
      K::Function(it) => it.body.as_ref().map(|body| span(body.span)),
      K::ArrowFunctionExpression(it) => match &it.body {
        ArrowFunctionBody::FunctionBody(body) => Some(span(body.span)),
        _ => None,
      },
      K::StaticBlock(it) => {
        let open = skip_trivia(self.file.text.as_bytes(), it.span.start + "static".len() as u32);
        Some(Span::new(open, it.span.end))
      }
      _ => None,
    }
  }

  /// The position of the parameter list's `(`, or an index signature's `[`. `None` for an arrow function, which may
  /// not have one.
  pub(crate) fn open_paren(self) -> Option<u32> {
    match self.raw() {
      K::ArrowFunctionExpression(_) | K::StaticBlock(_) => None,
      K::TSIndexSignature(it) => {
        let head = self
          .file
          .text
          .as_bytes()
          .get(it.span.start as usize..it.parameter.span.start as usize)?;
        let at = head.iter().rposition(|&byte| byte == b'[')?;
        Some(it.span.start + at as u32)
      }
      _ => self.head().params.map(|it| it.span.start),
    }
  }

  /// The `=>` of an arrow function.
  pub(crate) fn arrow_span(self) -> Option<Span> {
    let K::ArrowFunctionExpression(it) = self.raw() else { return None };
    let before = it.return_type.as_ref().map_or(it.params.span.end, |ty| ty.span.end);
    let at = skip_trivia(self.file.text.as_bytes(), before);
    Some(Span::new(at, at + 2))
  }

  /// The node it belongs to: the `Stmt` of a declaration, the `Expr` of a function expression or an arrow function or
  /// an object literal's method, the `Member` of a class method, a static block or a signature, the `TypeNode` of a
  /// function type.
  pub(crate) fn owner(self) -> Node<'a> {
    let (file, id) = (self.file, self.id);
    match self.raw() {
      K::Function(it) => {
        let parent = file.parent_id(id);
        match file.kind(parent) {
          Some(K::MethodDefinition(_)) => Node::Member(Member::new(file, parent)),
          _ if it.is_declaration() => Node::Stmt(Stmt::new(file, statement(file, id))),
          _ => Node::Expr(Expr::new(file, id)),
        }
      }
      K::ArrowFunctionExpression(_) => Node::Expr(Expr::new(file, id)),
      K::TSFunctionType(_) | K::TSConstructorType(_) => Node::Type(TypeNode::new(file, id)),
      K::StaticBlock(_)
      | K::TSMethodSignature(_)
      | K::TSCallSignatureDeclaration(_)
      | K::TSConstructSignatureDeclaration(_)
      | K::TSIndexSignature(_) => Node::Member(Member::new(file, id)),
      _ => Node::File(file),
    }
  }

  /// bun's `Func::span`: from its first token, which can be a modifier or a decorator, to its end. A method's or an
  /// accessor's starts where the member starts.
  pub(crate) fn full_span(self) -> Span {
    match self.owner() {
      Node::Member(member) => member.span(),
      Node::Stmt(statement) => statement.span(),
      _ => self.span(),
    }
  }

  /// Starts at the type parameters or the `(`. For a method or an accessor, this is the range of ESLint's
  /// `FunctionExpression`, the `value` of the `MethodDefinition` or `Property`.
  pub(crate) fn span_from_params(self) -> Span {
    let head = self.head();
    let start = (head.types.map(|it| it.span.start))
      .or_else(|| self.open_paren())
      .or_else(|| head.params.map(|it| it.span.start))
      .unwrap_or(self.span().start);
    Span::new(start, self.full_span().end)
  }

  /// The range of ESLint's node for it, which is oxc's: a declaration without `export`, the expression, the `value` of
  /// a method, the member or the type.
  pub(crate) fn estree_span(self) -> Span {
    self.span()
  }

  /// Everything between the parentheses: the `this` parameter, if any, then the other parameters.
  pub(crate) fn params_with_this(self) -> impl Iterator<Item = Param<'a>> + 'a {
    self.this_param().into_iter().chain(self.params())
  }

  /// The position of the parameter list's `)`, or an index signature's `]`. `None` for an arrow function without
  /// parentheses and for a static block.
  pub(crate) fn close_paren(self) -> Option<u32> {
    let text = self.file.text.as_bytes();
    if let K::TSIndexSignature(it) = self.raw() {
      let mut at = skip_trivia(text, it.parameter.span.end);
      if text.get(at as usize) == Some(&b',') {
        at = skip_trivia(text, at + 1);
      }
      return (text.get(at as usize) == Some(&b']')).then_some(at);
    }
    let params = self.head().params?;
    let close = params.span.end.checked_sub(1)?;
    (params.span.start < close && text.get(close as usize) == Some(&b')')).then_some(close)
  }

  /// The parameter list from `(` to `)`. For an arrow function without parentheses, the parameter. For an index
  /// signature, the brackets.
  pub(crate) fn params_span(self) -> Option<Span> {
    match self.raw() {
      K::TSIndexSignature(_) => Some(Span::new(self.open_paren()?, self.close_paren()? + 1)),
      _ => self.head().params.map(|it| span(it.span)),
    }
  }
}

impl<'a> Param<'a> {
  /// The pattern after the modifiers and the `...`. See the module's notes for `this` and an index signature.
  pub(crate) fn pat(self) -> Pat<'a> {
    let id = match self.raw() {
      K::FormalParameter(it) => it.pattern.node_id().index() as u32,
      K::FormalParameterRest(it) => it.rest.argument.node_id().index() as u32,
      _ => self.id,
    };
    Pat::new(self.file, id)
  }

  pub(crate) fn ty(self) -> Option<TypeNode<'a>> {
    let ty = match self.raw() {
      K::FormalParameter(it) => it.type_annotation.as_deref(),
      K::FormalParameterRest(it) => it.type_annotation.as_deref(),
      K::TSThisParameter(it) => it.type_annotation.as_deref(),
      K::TSIndexSignatureName(it) => Some(&*it.type_annotation),
      _ => None,
    };
    annotation(self.file, ty)
  }

  pub(crate) fn default(self) -> Option<Expr<'a>> {
    match self.raw() {
      K::FormalParameter(it) => {
        it.initializer.as_ref().map(|init| Expr::new(self.file, expression_id(init)))
      }
      _ => None,
    }
  }

  /// `REST`, `OPTIONAL`, `PARAMETER_PROPERTY`, and the modifiers.
  pub(crate) fn flags(self) -> Flags {
    match self.raw() {
      K::FormalParameter(it) => {
        let mut flags = accessibility(it.accessibility);
        flags.set(Flags::READONLY, it.readonly);
        flags.set(Flags::OVERRIDE, it.r#override);
        if !flags.is_empty() {
          flags |= Flags::PARAMETER_PROPERTY;
        }
        flags.set(Flags::OPTIONAL, it.optional);
        flags
      }
      K::FormalParameterRest(_) => Flags::REST,
      _ => Flags::empty(),
    }
  }

  pub(crate) fn is_rest(self) -> bool {
    self.flags().contains(Flags::REST)
  }

  pub(crate) fn is_optional(self) -> bool {
    self.flags().contains(Flags::OPTIONAL)
  }

  /// `constructor(private x)`, which also declares a class property.
  pub(crate) fn is_parameter_property(self) -> bool {
    self.flags().contains(Flags::PARAMETER_PROPERTY)
  }

  /// Keywords and decorators, in source order.
  pub(crate) fn modifiers(self) -> Modifiers<'a> {
    match self.raw() {
      K::FormalParameter(it) => modifiers(self.file, it.span.start, self.flags(), &it.decorators),
      K::FormalParameterRest(it) => {
        modifiers(self.file, it.span.start, Flags::empty(), &it.decorators)
      }
      _ => Modifiers::default(),
    }
  }

  pub(crate) fn decorators(self) -> impl Iterator<Item = Expr<'a>> + 'a {
    self.modifiers().into_iter().filter_map(Modifier::decorator)
  }

  /// The pattern, the `?` and the type annotation. This is the range of ESLint's `Identifier`, `ObjectPattern` or
  /// `ArrayPattern` for the parameter, because typescript-eslint includes the `?` and the annotation in that node.
  pub(crate) fn binding_span(self) -> Span {
    let pat = self.pat().span();
    let end = match self.ty() {
      Some(ty) => ty.outer_span().end,
      None if self.is_optional() => skip_trivia(self.file.text.as_bytes(), pat.end) + 1,
      None => pat.end,
    };
    Span::new(pat.start, end)
  }

  /// Excludes decorators and modifiers. This is the range of ESLint's node for the parameter, or of the `parameter`
  /// of a `TSParameterProperty`: the `AssignmentPattern` of `a = 1`, the `RestElement` of `...a`, otherwise
  /// [`Param::binding_span`].
  pub(crate) fn span_without_modifiers(self) -> Span {
    let whole = self.span();
    let start = match self.raw() {
      K::FormalParameter(it) => it.pattern.span().start,
      K::FormalParameterRest(it) => it.rest.span.start,
      _ => whole.start,
    };
    Span::new(start, whole.end)
  }

  pub(crate) fn func(self) -> Option<Func<'a>> {
    let mut id = self.file.parent_id(self.id);
    if let Some(K::FormalParameters(_)) = self.file.kind(id) {
      id = self.file.parent_id(id);
    }
    self.file.kind(id).filter(|kind| is_func(*kind)).map(|_| Func::new(self.file, id))
  }
}

impl<'a> TypeParam<'a> {
  fn raw_param(self) -> Option<&'a oxc_ast::ast::TSTypeParameter<'a>> {
    match self.raw() {
      K::TSTypeParameter(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn name(self) -> Ident<'a> {
    match self.raw() {
      K::TSTypeParameter(it) => binding(&it.name),
      K::BindingIdentifier(it) => binding(it),
      _ => missing(self.span()),
    }
  }

  /// The `C` of `T extends C`, or of the mapped type `[K in C]`.
  pub(crate) fn constraint(self) -> Option<TypeNode<'a>> {
    if let Some(mapped) = self.mapped() {
      return mapped.constraint();
    }
    let ty = self.raw_param()?.constraint.as_ref()?;
    Some(TypeNode::new(self.file, type_id(ty)))
  }

  pub(crate) fn default(self) -> Option<TypeNode<'a>> {
    let ty = self.raw_param()?.default.as_ref()?;
    Some(TypeNode::new(self.file, type_id(ty)))
  }

  /// `CONST`, `IN`, `OUT`
  pub(crate) fn flags(self) -> Flags {
    let mut flags = Flags::empty();
    if let Some(it) = self.raw_param() {
      flags.set(Flags::CONST, it.r#const);
      flags.set(Flags::IN, it.r#in);
      flags.set(Flags::OUT, it.out);
    }
    flags
  }

  /// `const`, `in` and `out`, in source order.
  pub(crate) fn modifiers(self) -> Modifiers<'a> {
    modifiers(self.file, self.span().start, self.flags(), &[])
  }

  /// The node that declares it: a `Func`, a `Class`, the `Stmt` of an interface or a type alias, or the `TypeNode` of
  /// an `infer` or a mapped type.
  pub(crate) fn parent(self) -> Node<'a> {
    let file = self.file;
    let list = file.parent_id(self.id);
    if let Some(K::TSTypeParameterDeclaration(_)) = file.kind(list) {
      let owner = file.parent_id(list);
      match file.kind(owner) {
        Some(K::Class(_)) => return Node::Class(Class::new(file, owner)),
        Some(K::TSInterfaceDeclaration(_) | K::TSTypeAliasDeclaration(_)) => {
          return Node::Stmt(Stmt::new(file, statement(file, owner)));
        }
        Some(kind) if is_func(kind) => return Node::Func(Func::new(file, owner)),
        _ => {}
      }
    }
    Node::TypeParam(self).parent()
  }
}

impl<'a> Class<'a> {
  fn raw_class(self) -> Option<&'a oxc_ast::ast::Class<'a>> {
    match self.raw() {
      K::Class(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn name(self) -> Option<Ident<'a>> {
    self.raw_class()?.id.as_ref().map(binding)
  }

  /// `ABSTRACT`, `AMBIENT`, `EXPORT`, `DEFAULT`
  pub(crate) fn flags(self) -> Flags {
    let Some(it) = self.raw_class() else { return Flags::empty() };
    let mut flags = Flags::empty();
    flags.set(Flags::ABSTRACT, it.r#abstract);
    flags.set(Flags::AMBIENT, it.declare);
    if it.is_declaration() {
      flags |= exports(self.file, self.id);
    }
    flags
  }

  pub(crate) fn type_params(self) -> List<'a, TypeParam<'a>> {
    type_params(self.file, self.raw_class().and_then(|it| it.type_parameters.as_deref()))
  }

  /// The `B` of `extends B<Args>`.
  pub(crate) fn extends(self) -> Option<Expr<'a>> {
    let heritage = self.raw_class()?.heritage.as_ref()?;
    Some(Expr::new(self.file, expression_id(&heritage.expression)))
  }

  /// The `Args` of `extends B<Args>`.
  pub(crate) fn extends_args(self) -> List<'a, TypeNode<'a>> {
    let args = self.raw_class().and_then(|it| it.heritage.as_ref()?.type_arguments.as_deref());
    match args {
      Some(args) => List::run(self.file, args.params.first().map(type_id), args.params.len()),
      None => List::empty(self.file),
    }
  }

  pub(crate) fn implements(self) -> List<'a, TypeNode<'a>> {
    match self.raw_class() {
      Some(it) => {
        let first = it.implements.first().map(|it| it.node_id().index() as u32);
        List::run(self.file, first, it.implements.len())
      }
      None => List::empty(self.file),
    }
  }

  pub(crate) fn members(self) -> List<'a, Member<'a>> {
    match self.raw_class() {
      Some(it) => {
        let members = &it.body.body;
        List::run(self.file, members.first().map(|it| it.node_id().index() as u32), members.len())
      }
      None => List::empty(self.file),
    }
  }

  /// Keywords and decorators, in source order. A declaration's are its statement's, `export` included.
  pub(crate) fn modifiers(self) -> Modifiers<'a> {
    match self.raw_class() {
      Some(it) => modifiers(
        self.file,
        self.full_span_without_decorators().start,
        self.flags(),
        &it.decorators,
      ),
      None => Modifiers::default(),
    }
  }

  pub(crate) fn decorators(self) -> impl Iterator<Item = Expr<'a>> + 'a {
    self.modifiers().into_iter().filter_map(Modifier::decorator)
  }

  /// Its first token, which can be a decorator before the `export`.
  pub(crate) fn start(self) -> u32 {
    let first = self.modifiers().first().map_or(u32::MAX, |it| it.span().start);
    first.min(self.full_span_without_decorators().start)
  }

  fn full_span_without_decorators(self) -> Span {
    match self.owner() {
      Node::Stmt(statement) => statement.span(),
      _ => self.span(),
    }
  }

  /// Its `Stmt` or `Expr`.
  pub(crate) fn owner(self) -> Node<'a> {
    match self.raw_class() {
      Some(it) if it.is_declaration() => {
        Node::Stmt(Stmt::new(self.file, statement(self.file, self.id)))
      }
      _ => Node::Expr(Expr::new(self.file, self.id)),
    }
  }

  /// bun's `Class::span`: from its first token, which can be a decorator or a modifier, to its end.
  pub(crate) fn full_span(self) -> Span {
    Span::new(self.start(), self.full_span_without_decorators().end)
  }

  /// The range of ESLint's `ClassDeclaration` or `ClassExpression`, which excludes `export` and `export default`.
  pub(crate) fn estree_span(self) -> Span {
    self.span()
  }

  /// The `class` keyword.
  pub(crate) fn keyword_span(self) -> Span {
    let start = match self.modifiers().last() {
      Some(last) => skip_trivia(self.file.text.as_bytes(), last.span().end),
      None => self.span().start,
    };
    Span::new(start, start + "class".len() as u32)
  }

  /// The `{ .. }` around the members.
  pub(crate) fn body_span(self) -> Span {
    self.raw_class().map_or_else(|| self.span(), |it| span(it.body.span))
  }
}

impl<'a> Member<'a> {
  /// `static constructor() {}` is also a `Constructor`, but ESLint treats it as a method. See
  /// [`Member::is_constructor`].
  pub(crate) fn kind(self) -> MemberKind {
    match self.raw() {
      K::MethodDefinition(it) => match it.kind {
        MethodDefinitionKind::Constructor => MemberKind::Constructor,
        MethodDefinitionKind::Get => MemberKind::Getter,
        MethodDefinitionKind::Set => MemberKind::Setter,
        MethodDefinitionKind::Method
          if it.r#static
            && matches!(self.raw_key().map(Key::kind),
              Some(KeyKind::Ident(name) | KeyKind::String(name)) if name.is("constructor")) =>
        {
          MemberKind::Constructor
        }
        MethodDefinitionKind::Method => MemberKind::Method,
      },
      K::StaticBlock(_) => MemberKind::StaticBlock,
      K::TSIndexSignature(_) => MemberKind::IndexSignature,
      K::TSCallSignatureDeclaration(_) => MemberKind::CallSignature,
      K::TSConstructSignatureDeclaration(_) => MemberKind::ConstructSignature,
      K::TSMethodSignature(it) => match it.kind {
        TSMethodSignatureKind::Method => MemberKind::Method,
        TSMethodSignatureKind::Get => MemberKind::Getter,
        TSMethodSignatureKind::Set => MemberKind::Setter,
      },
      _ => MemberKind::Property,
    }
  }

  /// ESLint's `kind === "constructor"`: a `Constructor` that isn't `static`, or a class method named with the string
  /// `"constructor"` that isn't parsed as the constructor, like `"constructor"<T>() {}`.
  pub(crate) fn is_constructor(self) -> bool {
    if self.is_static() {
      return false;
    }
    match self.kind() {
      MemberKind::Constructor => true,
      MemberKind::Method => {
        !self.is_signature()
          && matches!(self.raw_key().map(Key::kind), Some(KeyKind::String(name)) if name.is("constructor"))
      }
      _ => false,
    }
  }

  /// The key of any member that has one, a constructor's too.
  fn raw_key(self) -> Option<Key<'a>> {
    let file = self.file;
    match self.raw() {
      K::MethodDefinition(it) => Some(Key::of(file, &it.key, it.computed)),
      K::PropertyDefinition(it) => Some(Key::of(file, &it.key, it.computed)),
      K::AccessorProperty(it) => Some(Key::of(file, &it.key, it.computed)),
      K::TSPropertySignature(it) => Some(Key::of(file, &it.key, it.computed)),
      K::TSMethodSignature(it) => Some(Key::of(file, &it.key, it.computed)),
      _ => None,
    }
  }

  /// `None` for a constructor, a static block and a signature without a name.
  pub(crate) fn key(self) -> Option<Key<'a>> {
    match self.kind() {
      MemberKind::Property | MemberKind::Method | MemberKind::Getter | MemberKind::Setter => {
        self.raw_key()
      }
      _ => None,
    }
  }

  /// The `e` of a [`KeyKind::Computed`] key.
  pub(crate) fn computed_key(self) -> Option<Expr<'a>> {
    match self.key()?.kind() {
      KeyKind::Computed(e) => Some(e),
      _ => None,
    }
  }

  /// The keywords as flags.
  fn keywords(self) -> Flags {
    let mut flags = Flags::empty();
    match self.raw() {
      K::MethodDefinition(it) => {
        flags = accessibility(it.accessibility);
        flags.set(Flags::STATIC, it.r#static);
        flags.set(Flags::OVERRIDE, it.r#override);
        flags.set(Flags::OPTIONAL, it.optional);
        flags.set(Flags::ABSTRACT, it.r#type.is_abstract());
      }
      K::PropertyDefinition(it) => {
        flags = accessibility(it.accessibility);
        flags.set(Flags::STATIC, it.r#static);
        flags.set(Flags::AMBIENT, it.declare);
        flags.set(Flags::OVERRIDE, it.r#override);
        flags.set(Flags::OPTIONAL, it.optional);
        flags.set(Flags::DEFINITE, it.definite);
        flags.set(Flags::READONLY, it.readonly);
        flags.set(Flags::ABSTRACT, it.r#type.is_abstract());
      }
      K::AccessorProperty(it) => {
        flags = accessibility(it.accessibility) | Flags::ACCESSOR;
        flags.set(Flags::STATIC, it.r#static);
        flags.set(Flags::OVERRIDE, it.r#override);
        flags.set(Flags::DEFINITE, it.definite);
        flags.set(Flags::ABSTRACT, it.r#type.is_abstract());
      }
      K::StaticBlock(_) => flags = Flags::STATIC,
      K::TSIndexSignature(it) => {
        flags.set(Flags::STATIC, it.r#static);
        flags.set(Flags::READONLY, it.readonly);
      }
      K::TSPropertySignature(it) => {
        flags.set(Flags::OPTIONAL, it.optional);
        flags.set(Flags::READONLY, it.readonly);
      }
      K::TSMethodSignature(it) => flags.set(Flags::OPTIONAL, it.optional),
      _ => {}
    }
    flags
  }

  /// `STATIC`, `READONLY`, `OPTIONAL`, `DEFINITE`, `ABSTRACT`, `OVERRIDE`, `ACCESSOR`, `AMBIENT`, `PUBLIC`, `PROTECTED`,
  /// `PRIVATE`, and how its name is written: `LITERAL_NAME`, `STRING_NAME`, `COMPUTED_NAME`.
  pub(crate) fn flags(self) -> Flags {
    let name = match self.raw_key().map(Key::kind) {
      Some(KeyKind::String(_)) => Flags::LITERAL_NAME | Flags::STRING_NAME,
      Some(KeyKind::Number(_)) => Flags::LITERAL_NAME,
      Some(KeyKind::ComputedString(_)) => Flags::COMPUTED_NAME | Flags::STRING_NAME,
      Some(KeyKind::ComputedNumber(_) | KeyKind::Computed(_)) => Flags::COMPUTED_NAME,
      _ => Flags::empty(),
    };
    self.keywords() | name
  }

  pub(crate) fn is_static(self) -> bool {
    self.flags().contains(Flags::STATIC)
  }

  /// True if it's in an interface or a type literal rather than a class.
  pub(crate) fn is_signature(self) -> bool {
    matches!(
      self.file.kind(self.file.parent_id(self.id)),
      Some(K::TSInterfaceBody(_) | K::TSTypeLiteral(_))
    )
  }

  /// A constructor's `constructor` token, which is its `key` in ESLint. It can be written as a string.
  pub(crate) fn constructor_keyword(self) -> Option<Ident<'a>> {
    if self.kind() != MemberKind::Constructor {
      return None;
    }
    let key = self.raw_key()?;
    let string = matches!(key.kind(), KeyKind::String(_));
    Some(Ident { name: key.name()?, span: key.span(self.file), string })
  }

  /// A property's type annotation.
  pub(crate) fn ty(self) -> Option<TypeNode<'a>> {
    let ty = match self.raw() {
      K::PropertyDefinition(it) => it.type_annotation.as_deref(),
      K::AccessorProperty(it) => it.type_annotation.as_deref(),
      K::TSPropertySignature(it) => it.type_annotation.as_deref(),
      _ => None,
    };
    annotation(self.file, ty)
  }

  /// A property's initializer.
  pub(crate) fn init(self) -> Option<Expr<'a>> {
    let init = match self.raw() {
      K::PropertyDefinition(it) => it.value.as_ref(),
      K::AccessorProperty(it) => it.value.as_ref(),
      _ => None,
    };
    init.map(|it| Expr::new(self.file, expression_id(it)))
  }

  /// The function, for anything but a property.
  pub(crate) fn func(self) -> Option<Func<'a>> {
    match self.raw() {
      K::MethodDefinition(it) => Some(Func::new(self.file, it.value.node_id().index() as u32)),
      K::StaticBlock(_)
      | K::TSIndexSignature(_)
      | K::TSCallSignatureDeclaration(_)
      | K::TSConstructSignatureDeclaration(_)
      | K::TSMethodSignature(_) => Some(Func::new(self.file, self.id)),
      _ => None,
    }
  }

  /// Keywords and decorators, in source order. A static block's `static` isn't one.
  pub(crate) fn modifiers(self) -> Modifiers<'a> {
    let (flags, decorators) = match self.raw() {
      K::MethodDefinition(it) if it.value.r#async => {
        (self.keywords() | Flags::ASYNC, &it.decorators[..])
      }
      K::MethodDefinition(it) => (self.keywords(), &it.decorators[..]),
      K::PropertyDefinition(it) => (self.keywords(), &it.decorators[..]),
      K::AccessorProperty(it) => (self.keywords(), &it.decorators[..]),
      K::StaticBlock(_) => return Modifiers::default(),
      _ => (self.keywords(), &[][..]),
    };
    modifiers(self.file, self.span().start, flags, decorators)
  }

  pub(crate) fn decorators(self) -> impl Iterator<Item = Expr<'a>> + 'a {
    self.modifiers().into_iter().filter_map(Modifier::decorator)
  }

  /// The `Class`, the `Stmt` of the interface, or the `TypeNode` of the type literal.
  pub(crate) fn parent(self) -> Node<'a> {
    let body = self.file.parent_id(self.id);
    match self.file.kind(body) {
      Some(K::ClassBody(_)) => Node::Class(Class::new(self.file, self.file.parent_id(body))),
      Some(K::TSInterfaceBody(_)) => {
        Node::Stmt(Stmt::new(self.file, statement(self.file, self.file.parent_id(body))))
      }
      _ => Node::Member(self).parent(),
    }
  }
}

/// A keyword before a declaration, a member or a parameter, or a decorator.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Modifier<'a> {
  flag: Flags,
  span: Span,
  decorator: Option<Expr<'a>>,
}

impl<'a> Modifier<'a> {
  /// A keyword's flag. Empty for a decorator.
  pub(crate) fn flag(self) -> Flags {
    self.flag
  }

  /// The `e` of `@e`.
  pub(crate) fn decorator(self) -> Option<Expr<'a>> {
    self.decorator
  }

  /// The keyword, or the decorator from its `@`.
  pub(crate) fn span(self) -> Span {
    self.span
  }
}

/// The modifiers of a node, in source order.
#[derive(Clone, Default, Debug)]
pub(crate) struct Modifiers<'a>(SmallVec<[Modifier<'a>; 2]>);

impl<'a> Modifiers<'a> {
  pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = Modifier<'a>> + '_ {
    self.0.iter().copied()
  }

  pub(crate) fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub(crate) fn first(&self) -> Option<Modifier<'a>> {
    self.0.first().copied()
  }

  pub(crate) fn last(&self) -> Option<Modifier<'a>> {
    self.0.last().copied()
  }
}

impl<'a> IntoIterator for Modifiers<'a> {
  type Item = Modifier<'a>;
  type IntoIter = smallvec::IntoIter<[Modifier<'a>; 2]>;

  fn into_iter(self) -> Self::IntoIter {
    self.0.into_iter()
  }
}

/// The keywords that are modifiers: bun's `modifier_text`.
const KEYWORDS: [(Flags, &str); 15] = [
  (Flags::ABSTRACT, "abstract"),
  (Flags::ACCESSOR, "accessor"),
  (Flags::ASYNC, "async"),
  (Flags::CONST, "const"),
  (Flags::AMBIENT, "declare"),
  (Flags::DEFAULT, "default"),
  (Flags::EXPORT, "export"),
  (Flags::IN, "in"),
  (Flags::OUT, "out"),
  (Flags::OVERRIDE, "override"),
  (Flags::PRIVATE, "private"),
  (Flags::PROTECTED, "protected"),
  (Flags::PUBLIC, "public"),
  (Flags::READONLY, "readonly"),
  (Flags::STATIC, "static"),
];

/// The modifiers of a node that starts at `start`: its `decorators`, and the keywords of `flags` in the text, each read
/// once, up to the first word that isn't one.
pub(super) fn modifiers<'a>(
  file: &'a File<'a>,
  start: u32,
  flags: Flags,
  decorators: &[Decorator<'a>],
) -> Modifiers<'a> {
  let text = file.text.as_bytes();
  let decorator = |it: &Decorator<'a>| Modifier {
    flag: Flags::empty(),
    span: span(it.span),
    decorator: Some(Expr::new(file, expression_id(&it.expression))),
  };
  let mut decorators = decorators.iter().peekable();
  let first = decorators.peek().map_or(start, |it| it.span.start.min(start));
  let (mut list, mut left, mut at) = (SmallVec::new(), flags, skip_trivia(text, first));
  loop {
    if let Some(it) = decorators.next_if(|it| it.span.start <= at) {
      list.push(decorator(it));
      at = skip_trivia(text, it.span.end.max(at));
      continue;
    }
    let found =
      KEYWORDS.iter().find(|&&(flag, word)| left.contains(flag) && is_word(text, at, word));
    let Some(&(flag, word)) = found else { break };
    let end = at + word.len() as u32;
    list.push(Modifier { flag, span: Span::new(at, end), decorator: None });
    left -= flag;
    at = skip_trivia(text, end);
  }
  list.extend(decorators.map(decorator));
  Modifiers(list)
}

/// True if `word` is at `at`, and not the start of a longer name.
fn is_word(text: &[u8], at: u32, word: &str) -> bool {
  let rest = text.get(at as usize..).unwrap_or_default();
  rest.starts_with(word.as_bytes())
    && !rest.get(word.len()).is_some_and(|&byte| {
      byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'\\') || byte >= 0x80
    })
}

impl<'a> Prop<'a> {
  pub(crate) fn kind(self) -> PropKind {
    match self.raw() {
      K::ObjectProperty(it) if it.method => PropKind::Method,
      K::ObjectProperty(it) if it.shorthand => PropKind::Shorthand,
      K::ObjectProperty(it) => match it.kind {
        PropertyKind::Init => PropKind::Init,
        PropertyKind::Get => PropKind::Getter,
        PropertyKind::Set => PropKind::Setter,
      },
      K::AssignmentTargetPropertyIdentifier(_) => PropKind::Shorthand,
      K::SpreadElement(_) | K::JSXSpreadAttribute(_) | K::AssignmentTargetRest(_) => {
        PropKind::Spread
      }
      _ => PropKind::Init,
    }
  }

  /// `None` for a spread.
  pub(crate) fn key(self) -> Option<Key<'a>> {
    let file = self.file;
    match self.raw() {
      K::ObjectProperty(it) => Some(Key::of(file, &it.key, it.computed)),
      K::AssignmentTargetPropertyProperty(it) => Some(Key::of(file, &it.name, it.computed)),
      K::JSXAttribute(it) => Some(Key::jsx(file, &it.name)),
      K::AssignmentTargetPropertyIdentifier(it) => {
        let at = span(it.binding.span);
        Some(Key::new(KeyKind::Ident(Name(it.binding.name.as_str())), at, at))
      }
      K::ImportAttribute(it) => Some(match &it.key {
        ImportAttributeKey::Identifier(name) => {
          let at = span(name.span);
          Key::new(KeyKind::Ident(Name(name.name.as_str())), at, at)
        }
        ImportAttributeKey::StringLiteral(name) => {
          let at = span(name.span);
          Key::new(KeyKind::String(string(file, name)), at, at)
        }
      }),
      _ => None,
    }
  }

  /// Same as [`Key::is_computed`] on its key.
  pub(crate) fn has_computed_key(self) -> bool {
    self.key().is_some_and(Key::is_computed)
  }

  /// The `e` of a [`KeyKind::Computed`] key.
  pub(crate) fn computed_key(self) -> Option<Expr<'a>> {
    match self.key()?.kind() {
      KeyKind::Computed(e) => Some(e),
      _ => None,
    }
  }

  /// - `Init`: the value. `None` for a JSX attribute without one.
  /// - `Shorthand`: the identifier. In a destructuring assignment `{ a = 1 }` the default is [`Prop::default`].
  /// - `Spread`: the spread expression.
  /// - `Method`, `Getter`, `Setter`: an `ExprKind::Fn`.
  pub(crate) fn value(self) -> Option<Expr<'a>> {
    let file = self.file;
    let id = match self.raw() {
      K::ObjectProperty(it) => expression_id(&it.value),
      K::SpreadElement(it) => expression_id(&it.argument),
      K::JSXSpreadAttribute(it) => expression_id(&it.argument),
      K::AssignmentTargetRest(it) => target_id(&it.target),
      K::AssignmentTargetPropertyIdentifier(it) => it.binding.node_id().index() as u32,
      K::AssignmentTargetPropertyProperty(it) => match &it.binding {
        AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(it) => {
          it.node_id().index() as u32
        }
        binding => target_id(binding.as_assignment_target()?),
      },
      K::ImportAttribute(it) => it.value.node_id().index() as u32,
      K::JSXAttribute(it) => match it.value.as_ref()? {
        JSXAttributeValue::StringLiteral(it) => it.node_id().index() as u32,
        JSXAttributeValue::ExpressionContainer(it) => {
          return Some(Expr::from_raw(file, it.node_id().index() as u32));
        }
        JSXAttributeValue::Element(it) => it.node_id().index() as u32,
        JSXAttributeValue::Fragment(it) => it.node_id().index() as u32,
      },
      _ => return None,
    };
    Some(Expr::new(file, id))
  }

  /// The `1` of the shorthand `{ a = 1 }` in a destructuring assignment.
  pub(crate) fn default(self) -> Option<Expr<'a>> {
    match self.raw() {
      K::AssignmentTargetPropertyIdentifier(it) => {
        it.init.as_ref().map(|init| Expr::new(self.file, expression_id(init)))
      }
      _ => None,
    }
  }

  /// A method's or accessor's function.
  pub(crate) fn func(self) -> Option<Func<'a>> {
    match self.kind() {
      PropKind::Method | PropKind::Getter | PropKind::Setter => self.value()?.as_fn(),
      _ => None,
    }
  }

  /// `a?() {}`, which is an error that oxc doesn't parse.
  pub(crate) fn is_optional(self) -> bool {
    false
  }

  pub(crate) fn is_jsx_attribute(self) -> bool {
    matches!(self.raw(), K::JSXAttribute(_) | K::JSXSpreadAttribute(_))
  }

  /// True for a `key: "value"` entry of the `with { .. }` after a module specifier.
  pub(crate) fn is_import_attribute(self) -> bool {
    matches!(self.raw(), K::ImportAttribute(_))
  }

  /// A method's `async`: oxc has no decorators on a property, and parses no other keyword.
  pub(crate) fn modifiers(self) -> Modifiers<'a> {
    let flags = match self.func() {
      Some(func) if self.kind() == PropKind::Method => func.flags() & Flags::ASYNC,
      _ => Flags::empty(),
    };
    modifiers(self.file, self.span().start, flags, &[])
  }

  /// The object literal or the JSX element. For an [import attribute](Prop::is_import_attribute), the `Stmt` of the
  /// import or the export.
  pub(crate) fn parent(self) -> Node<'a> {
    Node::Prop(self).parent()
  }
}

/// Declares the `stmt` of a declaration's handle.
macro_rules! declarations {
  ($($name:ident,)*) => {
    $(impl<'a> $name<'a> {
      /// Its statement: the `export` around it, if there's one.
      pub(crate) fn stmt(self) -> Stmt<'a> {
        Stmt::new(self.file, statement(self.file, self.id))
      }
    })*
  };
}

declarations! {
  Interface,
  Alias,
  Enum,
  Module,
  Import,
  ImportEquals,
  Export,
}

impl<'a> Interface<'a> {
  fn raw_interface(self) -> Option<&'a oxc_ast::ast::TSInterfaceDeclaration<'a>> {
    match self.raw() {
      K::TSInterfaceDeclaration(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn name(self) -> Ident<'a> {
    self.raw_interface().map_or_else(|| missing(self.span()), |it| binding(&it.id))
  }

  /// `AMBIENT`, `EXPORT`, `DEFAULT`
  pub(crate) fn flags(self) -> Flags {
    let declare = self.raw_interface().is_some_and(|it| it.declare);
    exports(self.file, self.id) | if declare { Flags::AMBIENT } else { Flags::empty() }
  }

  pub(crate) fn type_params(self) -> List<'a, TypeParam<'a>> {
    type_params(self.file, self.raw_interface().and_then(|it| it.type_parameters.as_deref()))
  }

  pub(crate) fn extends(self) -> List<'a, TypeNode<'a>> {
    match self.raw_interface() {
      Some(it) => {
        let first = it.extends.first().map(|it| it.node_id().index() as u32);
        List::run(self.file, first, it.extends.len())
      }
      None => List::empty(self.file),
    }
  }

  pub(crate) fn members(self) -> List<'a, Member<'a>> {
    match self.raw_interface() {
      Some(it) => {
        let members = &it.body.body;
        List::run(self.file, members.first().map(|it| it.node_id().index() as u32), members.len())
      }
      None => List::empty(self.file),
    }
  }

  /// The `{ .. }` around the members.
  pub(crate) fn body_span(self) -> Span {
    self.raw_interface().map_or_else(|| self.span(), |it| span(it.body.span))
  }
}

impl<'a> Alias<'a> {
  fn raw_alias(self) -> Option<&'a oxc_ast::ast::TSTypeAliasDeclaration<'a>> {
    match self.raw() {
      K::TSTypeAliasDeclaration(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn name(self) -> Ident<'a> {
    self.raw_alias().map_or_else(|| missing(self.span()), |it| binding(&it.id))
  }

  /// `AMBIENT`, `EXPORT`
  pub(crate) fn flags(self) -> Flags {
    let declare = self.raw_alias().is_some_and(|it| it.declare);
    exports(self.file, self.id) | if declare { Flags::AMBIENT } else { Flags::empty() }
  }

  pub(crate) fn type_params(self) -> List<'a, TypeParam<'a>> {
    type_params(self.file, self.raw_alias().and_then(|it| it.type_parameters.as_deref()))
  }

  pub(crate) fn ty(self) -> TypeNode<'a> {
    let id = self.raw_alias().map_or(super::NONE, |it| type_id(&it.type_annotation));
    TypeNode::new(self.file, id)
  }
}

impl<'a> Enum<'a> {
  fn raw_enum(self) -> Option<&'a oxc_ast::ast::TSEnumDeclaration<'a>> {
    match self.raw() {
      K::TSEnumDeclaration(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn name(self) -> Ident<'a> {
    self.raw_enum().map_or_else(|| missing(self.span()), |it| binding(&it.id))
  }

  /// `CONST`, `AMBIENT`, `EXPORT`
  pub(crate) fn flags(self) -> Flags {
    let mut flags = exports(self.file, self.id);
    if let Some(it) = self.raw_enum() {
      flags.set(Flags::CONST, it.r#const);
      flags.set(Flags::AMBIENT, it.declare);
    }
    flags
  }

  pub(crate) fn members(self) -> List<'a, EnumMember<'a>> {
    match self.raw_enum() {
      Some(it) => {
        let members = &it.body.members;
        List::run(self.file, members.first().map(|it| it.node_id().index() as u32), members.len())
      }
      None => List::empty(self.file),
    }
  }

  /// The `{ .. }` around the members.
  pub(crate) fn body_span(self) -> Span {
    self.raw_enum().map_or_else(|| self.span(), |it| span(it.body.span))
  }
}

impl<'a> EnumMember<'a> {
  pub(crate) fn key(self) -> Option<Key<'a>> {
    let K::TSEnumMember(member) = self.raw() else { return None };
    let file = self.file;
    let (kind, inner) = match &member.id {
      TSEnumMemberName::Identifier(it) => (KeyKind::Ident(Name(it.name.as_str())), span(it.span)),
      TSEnumMemberName::String(it) => (KeyKind::String(string(file, it)), span(it.span)),
      TSEnumMemberName::ComputedString(it) => {
        (KeyKind::ComputedString(string(file, it)), span(it.span))
      }
      TSEnumMemberName::ComputedTemplateString(it) => {
        let cooked = it.quasis.first().and_then(|it| it.value.cooked?.as_str()).unwrap_or_default();
        (KeyKind::ComputedString(Name(cooked)), span(it.span))
      }
    };
    let outer = match kind {
      KeyKind::ComputedString(_) => {
        let close = skip_trivia(file.text.as_bytes(), inner.end);
        Span::new(member.span.start, close + 1)
      }
      _ => inner,
    };
    Some(Key::new(kind, inner, outer))
  }

  /// The `e` of a [`KeyKind::Computed`] key, which oxc doesn't parse.
  pub(crate) fn computed_key(self) -> Option<Expr<'a>> {
    None
  }

  pub(crate) fn init(self) -> Option<Expr<'a>> {
    let K::TSEnumMember(member) = self.raw() else { return None };
    member.initializer.as_ref().map(|it| Expr::new(self.file, expression_id(it)))
  }

  /// The `Stmt` of the enum.
  pub(crate) fn parent(self) -> Node<'a> {
    let body = self.file.parent_id(self.id);
    Node::Stmt(Stmt::new(self.file, statement(self.file, self.file.parent_id(body))))
  }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum ModuleName<'a> {
  /// `namespace N`
  Ident(Ident<'a>),
  /// `declare module "m"`
  String(Ident<'a>),
  /// `declare global`
  Global,
}

impl<'a> Module<'a> {
  pub(crate) fn name(self) -> ModuleName<'a> {
    match self.raw() {
      K::TSNamespaceDeclaration(it) => ModuleName::Ident(binding(&it.id)),
      K::TSExternalModuleDeclaration(it) => ModuleName::String(Ident {
        name: string(self.file, &it.id),
        span: span(it.id.span),
        string: true,
      }),
      _ => ModuleName::Global,
    }
  }

  /// The statements between the braces. It's the same list for the `A`, `B` and `C` of `namespace A.B.C { .. }`.
  pub(crate) fn body(self) -> List<'a, Stmt<'a>> {
    let file = self.file;
    let inner = self.innermost();
    let module = match inner.raw() {
      K::TSNamespaceDeclaration(it) => match &it.body {
        TSNamespaceDeclarationBody::TSModuleBlock(it) => Some(&**it),
        TSNamespaceDeclarationBody::TSNamespaceDeclaration(_) => None,
      },
      K::TSExternalModuleDeclaration(it) => it.body.as_deref(),
      K::TSGlobalDeclaration(it) => Some(&it.body),
      _ => None,
    };
    match module {
      Some(it) => block(file, &it.directives, &it.body),
      None => List::empty(file),
    }
  }

  /// False for `declare module "m";`.
  pub(crate) fn has_body(self) -> bool {
    match self.raw() {
      K::TSExternalModuleDeclaration(it) => it.body.is_some(),
      _ => true,
    }
  }

  /// True if it's written with the `module` keyword instead of `namespace`.
  pub(crate) fn uses_module_keyword(self) -> bool {
    match self.raw() {
      K::TSNamespaceDeclaration(it) => it.kind == TSNamespaceDeclarationKind::Module,
      K::TSExternalModuleDeclaration(_) => true,
      _ => false,
    }
  }

  /// The span of the name. For `declare global`, the `global` keyword.
  pub(crate) fn name_span(self) -> Span {
    match (self.name(), self.raw()) {
      (ModuleName::Ident(name) | ModuleName::String(name), _) => name.span(),
      (ModuleName::Global, K::TSGlobalDeclaration(it)) => span(it.global_span),
      (ModuleName::Global, _) => self.span(),
    }
  }

  /// The `{ .. }`. It's the same span for the `A`, `B` and `C` of `namespace A.B.C { .. }`.
  pub(crate) fn body_span(self) -> Option<Span> {
    match self.innermost().raw() {
      K::TSNamespaceDeclaration(it) => match &it.body {
        TSNamespaceDeclarationBody::TSModuleBlock(it) => Some(span(it.span)),
        TSNamespaceDeclarationBody::TSNamespaceDeclaration(_) => None,
      },
      K::TSExternalModuleDeclaration(it) => it.body.as_ref().map(|it| span(it.span)),
      K::TSGlobalDeclaration(it) => Some(span(it.body.span)),
      _ => None,
    }
  }

  /// The `C` of `namespace A.B.C { .. }`. Returns itself if its name has no dots.
  pub(crate) fn innermost(self) -> Module<'a> {
    let mut at = self;
    while let Some(nested) = at.nested() {
      at = nested;
    }
    at
  }

  /// The `A` of `namespace A.B.C { .. }`, which is the node. Returns itself if it's already the outermost.
  pub(crate) fn outermost(self) -> Module<'a> {
    let mut at = self;
    loop {
      let parent = self.file.parent_id(at.id);
      match self.file.kind(parent) {
        Some(K::TSNamespaceDeclaration(_)) => at = Module::new(self.file, parent),
        _ => return at,
      }
    }
  }

  /// The `B` of `namespace A.B`.
  pub(crate) fn nested(self) -> Option<Module<'a>> {
    match self.raw() {
      K::TSNamespaceDeclaration(it) => match &it.body {
        TSNamespaceDeclarationBody::TSNamespaceDeclaration(nested) => {
          Some(Module::new(self.file, nested.node_id().index() as u32))
        }
        TSNamespaceDeclarationBody::TSModuleBlock(_) => None,
      },
      _ => None,
    }
  }
}

impl<'a> Import<'a> {
  fn raw_import(self) -> Option<&'a oxc_ast::ast::ImportDeclaration<'a>> {
    match self.raw() {
      K::ImportDeclaration(it) => Some(it),
      _ => None,
    }
  }

  fn specifiers(self) -> &'a [ImportDeclarationSpecifier<'a>] {
    self.raw_import().and_then(|it| it.specifiers.as_ref()).map_or(&[], |it| it.as_slice())
  }

  /// The module specifier.
  pub(crate) fn spec(self) -> Name<'a> {
    self.raw_import().map_or(Name(""), |it| string(self.file, &it.source))
  }

  pub(crate) fn default(self) -> Option<Ident<'a>> {
    self.specifiers().iter().find_map(|it| match it {
      ImportDeclarationSpecifier::ImportDefaultSpecifier(it) => Some(binding(&it.local)),
      _ => None,
    })
  }

  pub(crate) fn namespace(self) -> Option<Ident<'a>> {
    self.specifiers().iter().find_map(|it| match it {
      ImportDeclarationSpecifier::ImportNamespaceSpecifier(it) => Some(binding(&it.local)),
      _ => None,
    })
  }

  /// `* as namespace`
  pub(crate) fn namespace_span(self) -> Option<Span> {
    self.specifiers().iter().find_map(|it| match it {
      ImportDeclarationSpecifier::ImportNamespaceSpecifier(it) => Some(span(it.span)),
      _ => None,
    })
  }

  pub(crate) fn named(self) -> List<'a, ImportSpec<'a>> {
    let named = || {
      self.specifiers().iter().filter_map(|it| match it {
        ImportDeclarationSpecifier::ImportSpecifier(it) => Some(it.node_id().index() as u32),
        _ => None,
      })
    };
    List::run(self.file, named().next(), named().count())
  }

  /// True if it has `{ }`, even if they're empty.
  pub(crate) fn has_named_imports(self) -> bool {
    self.open_brace().is_some()
  }

  /// The `{` of the named imports.
  fn open_brace(self) -> Option<u32> {
    let it = self.raw_import()?;
    let text = self.file.text.as_bytes();
    let mut at = self.clause_start();
    if it.import_kind.is_type() {
      at = skip_trivia(text, at + "type".len() as u32);
    }
    if let Some(phase) = it.phase {
      at = skip_trivia(text, at + phase.as_str().len() as u32);
    }
    if let Some(ImportDeclarationSpecifier::ImportDefaultSpecifier(default)) =
      self.specifiers().first()
    {
      at = skip_trivia(text, default.span.end);
      if text.get(at as usize) == Some(&b',') {
        at = skip_trivia(text, at + 1);
      }
    }
    (at < it.source.span.start && text.get(at as usize) == Some(&b'{')).then_some(at)
  }

  /// `import type ..`
  pub(crate) fn is_type_only(self) -> bool {
    self.raw_import().is_some_and(|it| it.import_kind.is_type())
  }

  /// `import defer ..`
  pub(crate) fn is_deferred(self) -> bool {
    self.phase() == Some("defer")
  }

  /// `"defer"` for `import defer ..`, `"source"` for `import source ..`.
  pub(crate) fn phase(self) -> Option<&'static str> {
    self.raw_import()?.phase.map(|phase| phase.as_str())
  }

  /// `import "spec"`
  pub(crate) fn is_side_effect(self) -> bool {
    self.specifiers().is_empty() && !self.has_named_imports()
  }

  /// The module specifier with its quotes.
  pub(crate) fn spec_span(self) -> Option<Span> {
    self.raw_import().map(|it| span(it.source.span))
  }

  fn clause_start(self) -> u32 {
    skip_trivia(self.file.text.as_bytes(), self.span().start + "import".len() as u32)
  }

  /// `with { type: "json" }`
  pub(crate) fn attributes(self) -> Option<ImportAttributes<'a>> {
    let import = self.raw_import()?;
    let clause = import.with_clause.as_deref()?;
    Some(ImportAttributes::clause(self.file, import.source.span.end, clause))
  }

  /// Everything between `import` and `from`. Empty for `import "spec"`.
  pub(crate) fn clause_span(self) -> Span {
    let start = self.clause_start();
    let text = self.file.text.as_bytes();
    let end = match (self.open_brace(), self.specifiers().last()) {
      (Some(open), last) => {
        let mut at = skip_trivia(text, last.map_or(open + 1, |it| it.span().end).max(open + 1));
        if text.get(at as usize) == Some(&b',') {
          at = skip_trivia(text, at + 1);
        }
        at + 1
      }
      (None, Some(last)) => last.span().end,
      (None, None) => start,
    };
    Span::new(start, end)
  }
}

impl<'a> ImportSpec<'a> {
  /// The name in the other module. It can be written as a string. `default` for a default import, `*` for a
  /// namespace import.
  pub(crate) fn imported(self) -> Ident<'a> {
    match self.raw() {
      K::ImportSpecifier(it) => export_name(self.file, &it.imported),
      K::ImportDefaultSpecifier(it) => Ident { name: Name("default"), ..binding(&it.local) },
      K::ImportNamespaceSpecifier(it) => {
        let star = Span::new(it.span.start, it.span.start + 1);
        Ident { name: Name("*"), span: star, string: false }
      }
      _ => missing(self.span()),
    }
  }

  pub(crate) fn local(self) -> Ident<'a> {
    match self.raw() {
      K::ImportSpecifier(it) => binding(&it.local),
      K::ImportDefaultSpecifier(it) => binding(&it.local),
      K::ImportNamespaceSpecifier(it) => binding(&it.local),
      _ => missing(self.span()),
    }
  }

  /// True if it has an `as`.
  pub(crate) fn is_renamed(self) -> bool {
    self.imported().span != self.local().span
  }

  /// `{ type a }`
  pub(crate) fn is_type_only(self) -> bool {
    matches!(self.raw(), K::ImportSpecifier(it) if it.import_kind.is_type())
  }

  pub(crate) fn import(self) -> Import<'a> {
    Import::new(self.file, self.file.parent_id(self.id))
  }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum ImportEqualsTarget<'a> {
  Require(Option<Name<'a>>),
  Entity(EntityName<'a>),
}

impl<'a> ImportEquals<'a> {
  fn raw_import(self) -> Option<&'a oxc_ast::ast::TSImportEqualsDeclaration<'a>> {
    match self.raw() {
      K::TSImportEqualsDeclaration(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn name(self) -> Ident<'a> {
    self.raw_import().map_or_else(|| missing(self.span()), |it| binding(&it.id))
  }

  pub(crate) fn target(self) -> ImportEqualsTarget<'a> {
    let Some(it) = self.raw_import() else { return ImportEqualsTarget::Require(None) };
    match &it.module_reference {
      TSModuleReference::ExternalModuleReference(it) => {
        ImportEqualsTarget::Require(Some(string(self.file, &it.expression)))
      }
      TSModuleReference::IdentifierReference(it) => {
        self.entity(TSTypeName::IdentifierReference(it.clone_in(self.file.allocator)))
      }
      TSModuleReference::QualifiedName(it) => {
        self.entity(TSTypeName::QualifiedName(it.clone_in(self.file.allocator)))
      }
    }
  }

  fn entity(self, name: TSTypeName<'a>) -> ImportEqualsTarget<'a> {
    ImportEqualsTarget::Entity(EntityName::of(self.file, self.file.allocator.alloc(name)))
  }

  /// `EXPORT`, `TYPE_ONLY`
  pub(crate) fn flags(self) -> Flags {
    let mut flags = exports(self.file, self.id);
    flags.set(Flags::TYPE_ONLY, self.raw_import().is_some_and(|it| it.import_kind.is_type()));
    flags
  }

  /// `require("spec")`, the range of ESLint's `TSExternalModuleReference`.
  pub(crate) fn require_span(self) -> Option<Span> {
    match &self.raw_import()?.module_reference {
      TSModuleReference::ExternalModuleReference(it) => Some(span(it.span)),
      _ => None,
    }
  }
}

impl<'a> Export<'a> {
  /// The specifiers, the module specifier after `from`, and `export type`.
  fn parts(self) -> (&'a [oxc_ast::ast::ExportSpecifier<'a>], Option<&'a StringLiteral<'a>>, bool) {
    match self.raw() {
      K::ExportNamedDeclaration(it) => (&it.specifiers, None, it.export_kind.is_type()),
      K::ExportFromDeclaration(it) => (&it.specifiers, Some(&it.source), it.export_kind.is_type()),
      _ => (&[], None, false),
    }
  }

  /// The module specifier after `from`.
  pub(crate) fn spec(self) -> Option<Name<'a>> {
    self.parts().1.map(|it| string(self.file, it))
  }

  pub(crate) fn has_from(self) -> bool {
    self.parts().1.is_some()
  }

  pub(crate) fn items(self) -> List<'a, ExportSpec<'a>> {
    let items = self.parts().0;
    List::run(self.file, items.first().map(|it| it.node_id().index() as u32), items.len())
  }

  /// `export type { .. }`
  pub(crate) fn is_type_only(self) -> bool {
    self.parts().2
  }

  /// The module specifier with its quotes.
  pub(crate) fn spec_span(self) -> Option<Span> {
    self.parts().1.map(|it| span(it.span))
  }

  /// `with { type: "json" }`
  pub(crate) fn attributes(self) -> Option<ImportAttributes<'a>> {
    match self.raw() {
      K::ExportFromDeclaration(it) => {
        Some(ImportAttributes::clause(self.file, it.source.span.end, it.with_clause.as_deref()?))
      }
      _ => None,
    }
  }
}

impl<'a> ExportSpec<'a> {
  fn raw_spec(self) -> Option<&'a oxc_ast::ast::ExportSpecifier<'a>> {
    match self.raw() {
      K::ExportSpecifier(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn local(self) -> Ident<'a> {
    self.raw_spec().map_or_else(|| missing(self.span()), |it| export_name(self.file, &it.local))
  }

  pub(crate) fn exported(self) -> Ident<'a> {
    self.raw_spec().map_or_else(|| missing(self.span()), |it| export_name(self.file, &it.exported))
  }

  /// True if it has an `as`.
  pub(crate) fn is_renamed(self) -> bool {
    self.local().span != self.exported().span
  }

  /// `{ type a }`
  pub(crate) fn is_type_only(self) -> bool {
    self.raw_spec().is_some_and(|it| it.export_kind.is_type())
  }

  pub(crate) fn export(self) -> Export<'a> {
    Export::new(self.file, self.file.parent_id(self.id))
  }
}

/// True for the oxc nodes that are a [`Func`].
fn is_func(kind: AstKind) -> bool {
  matches!(
    kind,
    K::Function(_)
      | K::ArrowFunctionExpression(_)
      | K::StaticBlock(_)
      | K::TSMethodSignature(_)
      | K::TSCallSignatureDeclaration(_)
      | K::TSConstructSignatureDeclaration(_)
      | K::TSFunctionType(_)
      | K::TSConstructorType(_)
      | K::TSIndexSignature(_)
  )
}

/// The id of a declaration's statement: the `export` around it, or the declaration.
fn statement(file: &File, id: u32) -> u32 {
  let parent = file.parent_id(id);
  match file.kind(parent) {
    Some(K::ExportDeclaration(_) | K::ExportDefaultDeclaration(_)) => parent,
    _ => id,
  }
}

/// `EXPORT` and `DEFAULT` from the `export` around a declaration.
fn exports(file: &File, id: u32) -> Flags {
  match file.kind(file.parent_id(id)) {
    Some(K::ExportDeclaration(_)) => Flags::EXPORT,
    Some(K::ExportDefaultDeclaration(_)) => Flags::EXPORT | Flags::DEFAULT,
    _ => Flags::empty(),
  }
}

fn accessibility(accessibility: Option<TSAccessibility>) -> Flags {
  match accessibility {
    Some(TSAccessibility::Private) => Flags::PRIVATE,
    Some(TSAccessibility::Protected) => Flags::PROTECTED,
    Some(TSAccessibility::Public) => Flags::PUBLIC,
    None => Flags::empty(),
  }
}

fn type_params<'a>(
  file: &'a File<'a>,
  types: Option<&'a TSTypeParameterDeclaration<'a>>,
) -> List<'a, TypeParam<'a>> {
  match types {
    Some(types) => {
      let first = types.params.first().map(|it| it.node_id().index() as u32);
      List::run(file, first, types.params.len())
    }
    None => List::empty(file),
  }
}

fn annotation<'a>(file: &'a File<'a>, ty: Option<&TSTypeAnnotation>) -> Option<TypeNode<'a>> {
  ty.map(|ty| TypeNode::new(file, type_id(&ty.type_annotation)))
}

/// The statements of a body, with its directives first.
pub(super) fn block<'a>(
  file: &'a File<'a>,
  directives: &[Directive],
  statements: &[Statement],
) -> List<'a, Stmt<'a>> {
  let first = (directives.first().map(|it| it.node_id().index() as u32))
    .or_else(|| statements.first().map(|it| it.node_id().index() as u32));
  List::run(file, first, directives.len() + statements.len())
}

fn binding<'a>(it: &BindingIdentifier<'a>) -> Ident<'a> {
  Ident { name: Name(it.name.as_str()), span: span(it.span), string: false }
}

/// A name that isn't there: the handle isn't of the node its method expects.
fn missing<'a>(at: Span) -> Ident<'a> {
  Ident { name: Name(""), span: Span::empty(at.start), string: false }
}

/// The value of a string literal.
fn string<'a>(file: &'a File<'a>, it: &StringLiteral<'a>) -> Name<'a> {
  let text = file.str(span(it.span));
  let quoted = text.get(1..text.len().saturating_sub(1)).unwrap_or_default();
  Name(it.value.as_str().unwrap_or(quoted))
}

/// The name of an import or an export, which can be a string.
fn export_name<'a>(file: &'a File<'a>, name: &ModuleExportName<'a>) -> Ident<'a> {
  match name {
    ModuleExportName::IdentifierName(it) => {
      Ident { name: Name(it.name.as_str()), span: span(it.span), string: false }
    }
    ModuleExportName::IdentifierReference(it) => {
      Ident { name: Name(it.name.as_str()), span: span(it.span), string: false }
    }
    ModuleExportName::StringLiteral(it) => {
      Ident { name: string(file, it), span: span(it.span), string: true }
    }
  }
}

fn span(span: oxc_span::Span) -> Span {
  Span::new(span.start, span.end)
}
