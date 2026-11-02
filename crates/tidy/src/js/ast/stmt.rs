//! Statements: bun's `src/lint/ast/stmt.rs` (MIT), over oxc's tree.
//!
//! Where bun's tree differs from oxc's:
//! - `export const a = 1` is one `Var` statement with [`Flags::EXPORT`], and `export default function f() {}` one `Fn`
//!   with [`Flags::EXPORT`] and [`Flags::DEFAULT`]. oxc's `ExportDeclaration` and `ExportDefaultDeclaration` are that
//!   statement, and the declaration inside isn't a statement of its own.
//! - The expression in a `for` head is a statement in bun's tree that isn't a node, a [wrapper](Stmt::is_wrapper).
//!   Here it's a `Stmt` with the expression's id, whose kind is `Expr`.
//! - A directive is an `Expr` statement of a string, before the other statements of its body.

use super::{
  Alias, Case, Class, Enum, Export, Expr, Flags, Func, Ident, Import, ImportEquals, Interface,
  List, Module, Name, Node, Span, Stmt, VarDecl, VarKind, expression_id, skip_trivia,
};
use oxc_ast::AstKind;
use oxc_ast::ast::{
  Declaration, ExportDefaultDeclarationKind, ForStatementInit, ForStatementLeft,
  ImportOrExportKind, ModuleExportName, Statement, VariableDeclarationKind,
};
use oxc_span::GetSpan;

#[derive(Copy, Clone, Debug)]
pub(crate) enum StmtKind<'a> {
  Empty,
  Debugger,
  Expr(Expr<'a>),
  /// `var`, `let`, `const`, `using`, `await using`. Also the declaration in a `for` head.
  Var(List<'a, VarDecl<'a>>),
  Fn(Func<'a>),
  Class(Class<'a>),
  Interface(Interface<'a>),
  TypeAlias(Alias<'a>),
  Enum(Enum<'a>),
  /// `namespace N {}`, `declare module "m" {}`, `declare global {}`
  Module(Module<'a>),
  Return(Option<Expr<'a>>),
  If {
    test: Expr<'a>,
    yes: Stmt<'a>,
    no: Option<Stmt<'a>>,
  },
  /// `init` is a `Var`, or an `Expr` that's a [wrapper](Stmt::is_wrapper).
  For {
    init: Option<Stmt<'a>>,
    test: Option<Expr<'a>>,
    update: Option<Expr<'a>>,
    body: Stmt<'a>,
  },
  /// `left` is a `Var` with one declaration, or an `Expr` that's a [wrapper](Stmt::is_wrapper).
  ForIn {
    left: Stmt<'a>,
    expr: Expr<'a>,
    body: Stmt<'a>,
  },
  ForOf {
    left: Stmt<'a>,
    expr: Expr<'a>,
    body: Stmt<'a>,
    is_await: bool,
  },
  While {
    test: Expr<'a>,
    body: Stmt<'a>,
  },
  DoWhile {
    body: Stmt<'a>,
    test: Expr<'a>,
  },
  Block(List<'a, Stmt<'a>>),
  With {
    object: Expr<'a>,
    body: Stmt<'a>,
  },
  Switch {
    expr: Expr<'a>,
    cases: List<'a, Case<'a>>,
  },
  /// `block`, `handler` and `finalizer` are `Block`s.
  Try {
    block: Stmt<'a>,
    param: Option<VarDecl<'a>>,
    handler: Option<Stmt<'a>>,
    finalizer: Option<Stmt<'a>>,
  },
  Throw(Expr<'a>),
  Break(Option<Name<'a>>),
  Continue(Option<Name<'a>>),
  Labeled {
    label: Name<'a>,
    body: Stmt<'a>,
  },
  Import(Import<'a>),
  /// `import a = require("m")`, `import a = b.c`
  ImportEquals(ImportEquals<'a>),
  /// `export { a as b }`, with or without `from`.
  ExportNamed(Export<'a>),
  /// `export * from "m"`, `export * as alias from "m"`
  ExportStar {
    spec: Option<Name<'a>>,
    alias: Option<Ident<'a>>,
    type_only: bool,
  },
  /// `export default e`
  ExportDefault(Expr<'a>),
  /// `export = e`
  ExportAssign(Expr<'a>),
  /// `export as namespace N`
  ExportAsNamespace(Name<'a>),
}

/// The kind of a statement, without its payload.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub(crate) enum StmtTag {
  Empty,
  Debugger,
  Expr,
  Var,
  Fn,
  Class,
  Interface,
  TypeAlias,
  Enum,
  Module,
  Return,
  If,
  For,
  ForIn,
  ForOf,
  While,
  DoWhile,
  Block,
  With,
  Switch,
  Try,
  Throw,
  Break,
  Continue,
  Labeled,
  Import,
  ImportEquals,
  ExportNamed,
  ExportStar,
  ExportDefault,
  ExportAssign,
  ExportAsNamespace,
}

fn span(span: oxc_span::Span) -> Span {
  Span::new(span.start, span.end)
}

impl<'a> Stmt<'a> {
  fn at(self, id: u32) -> Stmt<'a> {
    Stmt::new(self.file, id)
  }

  fn statement(self, statement: &Statement) -> Stmt<'a> {
    self.at(statement.node_id().index() as u32)
  }

  fn expr(self, expression: &oxc_ast::ast::Expression) -> Expr<'a> {
    Expr::new(self.file, expression_id(expression))
  }

  /// The node whose kind this statement has: the declaration of an export, or the statement itself.
  fn declaration(self) -> Option<AstKind<'a>> {
    match self.file.kind(self.id)? {
      AstKind::ExportDeclaration(it) => Some(declaration_kind(&it.declaration)),
      AstKind::ExportDefaultDeclaration(it) => match &it.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(it) => Some(AstKind::Function(it)),
        ExportDefaultDeclarationKind::ClassDeclaration(it) => Some(AstKind::Class(it)),
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(it) => {
          Some(AstKind::TSInterfaceDeclaration(it))
        }
        _ => self.file.kind(self.id),
      },
      kind => Some(kind),
    }
  }

  pub(crate) fn kind(self) -> StmtKind<'a> {
    use AstKind as K;
    let Some(raw) = self.declaration() else { return StmtKind::Empty };
    let file = self.file;
    let own = |kind: AstKind| kind.node_id().index() as u32;
    match raw {
      K::EmptyStatement(_) => StmtKind::Empty,
      K::DebuggerStatement(_) => StmtKind::Debugger,
      K::ExpressionStatement(it) => StmtKind::Expr(self.expr(&it.expression)),
      K::Directive(it) => {
        StmtKind::Expr(Expr::new(file, it.expression.node_id.get().index() as u32))
      }
      K::VariableDeclaration(it) => {
        let first = it.declarations.first().map(|it| it.node_id.get().index() as u32);
        StmtKind::Var(List::run(file, first, it.declarations.len()))
      }
      K::Function(_) => StmtKind::Fn(Func::new(file, own(raw))),
      K::Class(_) => StmtKind::Class(Class::new(file, own(raw))),
      K::TSInterfaceDeclaration(_) => StmtKind::Interface(Interface::new(file, own(raw))),
      K::TSTypeAliasDeclaration(_) => StmtKind::TypeAlias(Alias::new(file, own(raw))),
      K::TSEnumDeclaration(_) => StmtKind::Enum(Enum::new(file, own(raw))),
      K::TSNamespaceDeclaration(_)
      | K::TSExternalModuleDeclaration(_)
      | K::TSGlobalDeclaration(_) => StmtKind::Module(Module::new(file, own(raw))),
      K::TSImportEqualsDeclaration(_) => StmtKind::ImportEquals(ImportEquals::new(file, own(raw))),
      K::ReturnStatement(it) => StmtKind::Return(it.argument.as_ref().map(|it| self.expr(it))),
      K::IfStatement(it) => StmtKind::If {
        test: self.expr(&it.test),
        yes: self.statement(&it.consequent),
        no: it.alternate.as_ref().map(|it| self.statement(it)),
      },
      K::ForStatement(it) => StmtKind::For {
        init: it.init.as_ref().map(|init| match init {
          ForStatementInit::VariableDeclaration(it) => self.at(it.node_id.get().index() as u32),
          init => self.at(init.as_expression().map_or(super::NONE, expression_id)),
        }),
        test: it.test.as_ref().map(|it| self.expr(it)),
        update: it.update.as_ref().map(|it| self.expr(it)),
        body: self.statement(&it.body),
      },
      K::ForInStatement(it) => StmtKind::ForIn {
        left: self.left(&it.left),
        expr: self.expr(&it.right),
        body: self.statement(&it.body),
      },
      K::ForOfStatement(it) => StmtKind::ForOf {
        left: self.left(&it.left),
        expr: self.expr(&it.right),
        body: self.statement(&it.body),
        is_await: it.r#await,
      },
      K::WhileStatement(it) => {
        StmtKind::While { test: self.expr(&it.test), body: self.statement(&it.body) }
      }
      K::DoWhileStatement(it) => {
        StmtKind::DoWhile { body: self.statement(&it.body), test: self.expr(&it.test) }
      }
      K::BlockStatement(it) => StmtKind::Block(statements(file, &it.body)),
      K::WithStatement(it) => {
        StmtKind::With { object: self.expr(&it.object), body: self.statement(&it.body) }
      }
      K::SwitchStatement(it) => {
        let first = it.cases.first().map(|it| it.node_id.get().index() as u32);
        StmtKind::Switch {
          expr: self.expr(&it.discriminant),
          cases: List::run(file, first, it.cases.len()),
        }
      }
      K::TryStatement(it) => StmtKind::Try {
        block: self.at(it.block.node_id.get().index() as u32),
        param: it
          .handler
          .as_ref()
          .and_then(|handler| handler.param.as_ref())
          .map(|param| VarDecl::new(file, param.node_id.get().index() as u32)),
        handler: it
          .handler
          .as_ref()
          .map(|handler| self.at(handler.body.node_id.get().index() as u32)),
        finalizer: it.finalizer.as_ref().map(|it| self.at(it.node_id.get().index() as u32)),
      },
      K::ThrowStatement(it) => StmtKind::Throw(self.expr(&it.argument)),
      K::BreakStatement(it) => StmtKind::Break(it.label.as_ref().map(|it| Name(it.name.as_str()))),
      K::ContinueStatement(it) => {
        StmtKind::Continue(it.label.as_ref().map(|it| Name(it.name.as_str())))
      }
      K::LabeledStatement(it) => {
        StmtKind::Labeled { label: Name(it.label.name.as_str()), body: self.statement(&it.body) }
      }
      K::ImportDeclaration(_) => StmtKind::Import(Import::new(file, own(raw))),
      K::ExportNamedDeclaration(_) | K::ExportFromDeclaration(_) => {
        StmtKind::ExportNamed(Export::new(file, own(raw)))
      }
      K::ExportAllDeclaration(it) => StmtKind::ExportStar {
        spec: it.source.value.as_str().map(Name),
        alias: it.exported.as_ref().map(|exported| export_name(file, exported)),
        type_only: it.export_kind == ImportOrExportKind::Type,
      },
      K::ExportDefaultDeclaration(it) => match it.declaration.as_expression() {
        Some(expression) => StmtKind::ExportDefault(self.expr(expression)),
        None => StmtKind::Empty,
      },
      K::TSExportAssignment(it) => StmtKind::ExportAssign(self.expr(&it.expression)),
      K::TSNamespaceExportDeclaration(it) => StmtKind::ExportAsNamespace(Name(it.id.name.as_str())),
      _ => match Node::of(file, self.id) {
        Some(Node::Expr(_)) => StmtKind::Expr(Expr::new(file, self.id)),
        _ => StmtKind::Empty,
      },
    }
  }

  /// The left side of a `for .. in` or `for .. of`.
  fn left(self, left: &ForStatementLeft) -> Stmt<'a> {
    match left {
      ForStatementLeft::VariableDeclaration(it) => self.at(it.node_id.get().index() as u32),
      left => match left.as_assignment_target().and_then(|it| it.as_simple_assignment_target()) {
        Some(simple) => self.at(simple.node_id().index() as u32),
        None => self.at(left.node_id().index() as u32),
      },
    }
  }

  pub(crate) fn tag(self) -> StmtTag {
    match self.kind() {
      StmtKind::Empty => StmtTag::Empty,
      StmtKind::Debugger => StmtTag::Debugger,
      StmtKind::Expr(_) => StmtTag::Expr,
      StmtKind::Var(_) => StmtTag::Var,
      StmtKind::Fn(_) => StmtTag::Fn,
      StmtKind::Class(_) => StmtTag::Class,
      StmtKind::Interface(_) => StmtTag::Interface,
      StmtKind::TypeAlias(_) => StmtTag::TypeAlias,
      StmtKind::Enum(_) => StmtTag::Enum,
      StmtKind::Module(_) => StmtTag::Module,
      StmtKind::Return(_) => StmtTag::Return,
      StmtKind::If { .. } => StmtTag::If,
      StmtKind::For { .. } => StmtTag::For,
      StmtKind::ForIn { .. } => StmtTag::ForIn,
      StmtKind::ForOf { .. } => StmtTag::ForOf,
      StmtKind::While { .. } => StmtTag::While,
      StmtKind::DoWhile { .. } => StmtTag::DoWhile,
      StmtKind::Block(_) => StmtTag::Block,
      StmtKind::With { .. } => StmtTag::With,
      StmtKind::Switch { .. } => StmtTag::Switch,
      StmtKind::Try { .. } => StmtTag::Try,
      StmtKind::Throw(_) => StmtTag::Throw,
      StmtKind::Break(_) => StmtTag::Break,
      StmtKind::Continue(_) => StmtTag::Continue,
      StmtKind::Labeled { .. } => StmtTag::Labeled,
      StmtKind::Import(_) => StmtTag::Import,
      StmtKind::ImportEquals(_) => StmtTag::ImportEquals,
      StmtKind::ExportNamed(_) => StmtTag::ExportNamed,
      StmtKind::ExportStar { .. } => StmtTag::ExportStar,
      StmtKind::ExportDefault(_) => StmtTag::ExportDefault,
      StmtKind::ExportAssign(_) => StmtTag::ExportAssign,
      StmtKind::ExportAsNamespace(_) => StmtTag::ExportAsNamespace,
    }
  }

  pub(crate) fn parent(self) -> Node<'a> {
    Node::Stmt(self).parent()
  }

  /// The modifiers as flags: `export`, `default`, `declare`, `abstract`, `async` and `const` of the declaration.
  pub(crate) fn flags(self) -> Flags {
    let mut flags = Flags::empty();
    match self.file.kind(self.id) {
      Some(AstKind::ExportDeclaration(_)) => flags |= Flags::EXPORT,
      Some(AstKind::ExportDefaultDeclaration(_)) => flags |= Flags::EXPORT | Flags::DEFAULT,
      _ => {}
    }
    match self.declaration() {
      Some(AstKind::VariableDeclaration(it)) if it.declare => flags |= Flags::AMBIENT,
      Some(AstKind::Function(it)) => {
        flags.set(Flags::AMBIENT, it.declare);
        flags.set(Flags::ASYNC, it.r#async);
        flags.set(Flags::GENERATOR, it.generator);
      }
      Some(AstKind::Class(it)) => {
        flags.set(Flags::AMBIENT, it.declare);
        flags.set(Flags::ABSTRACT, it.r#abstract);
      }
      Some(AstKind::TSInterfaceDeclaration(it)) if it.declare => flags |= Flags::AMBIENT,
      Some(AstKind::TSTypeAliasDeclaration(it)) if it.declare => flags |= Flags::AMBIENT,
      Some(AstKind::TSEnumDeclaration(it)) => {
        flags.set(Flags::AMBIENT, it.declare);
        flags.set(Flags::CONST, it.r#const);
      }
      Some(AstKind::TSNamespaceDeclaration(it)) if it.declare => flags |= Flags::AMBIENT,
      Some(AstKind::TSExternalModuleDeclaration(it)) if it.declare => flags |= Flags::AMBIENT,
      Some(AstKind::TSGlobalDeclaration(it)) if it.declare => flags |= Flags::AMBIENT,
      Some(AstKind::ImportDeclaration(it)) if it.import_kind == ImportOrExportKind::Type => {
        flags |= Flags::TYPE_ONLY
      }
      Some(AstKind::ExportNamedDeclaration(it)) if it.export_kind == ImportOrExportKind::Type => {
        flags |= Flags::TYPE_ONLY
      }
      Some(AstKind::ExportFromDeclaration(it)) if it.export_kind == ImportOrExportKind::Type => {
        flags |= Flags::TYPE_ONLY
      }
      _ => {}
    }
    flags
  }

  /// True if it has the `export` modifier.
  pub(crate) fn is_exported(self) -> bool {
    self.flags().contains(Flags::EXPORT)
  }

  /// `export default` before a function, a class or an interface.
  pub(crate) fn is_default_export(self) -> bool {
    self.flags().contains(Flags::EXPORT | Flags::DEFAULT)
  }

  /// The declaration without `export` or `export default`.
  pub(crate) fn span_without_export(self) -> Span {
    match self.file.kind(self.id) {
      Some(AstKind::ExportDeclaration(it)) => span(it.declaration.span()),
      Some(AstKind::ExportDefaultDeclaration(it)) => span(it.declaration.span()),
      _ => self.span(),
    }
  }

  /// From the `export` to the end, for an exported declaration.
  pub(crate) fn export_span(self) -> Option<Span> {
    self.is_exported().then(|| self.span())
  }

  /// The `;` that ends it, if it has one.
  pub(crate) fn semicolon(self) -> Option<Span> {
    let end = self.span().end;
    let terminated = matches!(
      self.tag(),
      StmtTag::Empty
        | StmtTag::Debugger
        | StmtTag::Expr
        | StmtTag::Var
        | StmtTag::Return
        | StmtTag::DoWhile
        | StmtTag::Throw
        | StmtTag::Break
        | StmtTag::Continue
        | StmtTag::TypeAlias
        | StmtTag::Import
        | StmtTag::ImportEquals
        | StmtTag::ExportNamed
        | StmtTag::ExportStar
        | StmtTag::ExportDefault
        | StmtTag::ExportAssign
        | StmtTag::ExportAsNamespace
        | StmtTag::Fn
        | StmtTag::Module
    );
    (terminated && self.file.text.as_bytes().get(end.wrapping_sub(1) as usize) == Some(&b';'))
      .then(|| Span::new(end - 1, end))
  }

  /// For a `Try` with a `catch`: from the `catch` to the end of its block.
  pub(crate) fn catch_clause_span(self) -> Option<Span> {
    match self.file.kind(self.id)? {
      AstKind::TryStatement(it) => it.handler.as_ref().map(|it| span(it.span)),
      _ => None,
    }
  }

  /// The module specifier with its quotes.
  pub(crate) fn module_specifier_span(self) -> Option<Span> {
    let source = match self.file.kind(self.id)? {
      AstKind::ImportDeclaration(it) => &it.source,
      AstKind::ExportFromDeclaration(it) => &it.source,
      AstKind::ExportAllDeclaration(it) => &it.source,
      AstKind::TSImportEqualsDeclaration(it) => match &it.module_reference {
        oxc_ast::ast::TSModuleReference::ExternalModuleReference(it) => &it.expression,
        _ => return None,
      },
      _ => return None,
    };
    Some(span(source.span))
  }

  /// The offset of the `*` of an `ExportStar`.
  pub(crate) fn export_star_offset(self) -> Option<u32> {
    match self.file.kind(self.id)? {
      AstKind::ExportAllDeclaration(it) => {
        let text = self.file.text.as_bytes();
        let mut at = skip_trivia(text, it.span.start + "export".len() as u32);
        if text.get(at as usize..).is_some_and(|rest| rest.starts_with(b"type")) {
          at = skip_trivia(text, at + "type".len() as u32);
        }
        Some(at)
      }
      _ => None,
    }
  }

  /// The `N` of `export as namespace N`.
  pub(crate) fn namespace_export_name(self) -> Option<Ident<'a>> {
    match self.file.kind(self.id)? {
      AstKind::TSNamespaceExportDeclaration(it) => {
        Some(Ident { name: Name(it.id.name.as_str()), span: span(it.id.span), string: false })
      }
      _ => None,
    }
  }

  /// True if it stands for the expression in a `for` head, which isn't a statement.
  pub(crate) fn is_wrapper(self) -> bool {
    !matches!(Node::of(self.file, self.id), Some(Node::Stmt(_)) | None)
  }

  /// The `for` statement it's a wrapper in.
  pub(crate) fn wrapped_in(self) -> Option<Stmt<'a>> {
    match self.is_wrapper() {
      true => match Node::Expr(Expr::new(self.file, self.id)).parent() {
        Node::Stmt(parent) => Some(parent),
        _ => None,
      },
      false => None,
    }
  }

  /// The label of a `Labeled`, `Break` or `Continue`, with its position.
  pub(crate) fn label(self) -> Option<Ident<'a>> {
    let label = match self.file.kind(self.id)? {
      AstKind::LabeledStatement(it) => &it.label,
      AstKind::BreakStatement(it) => it.label.as_ref()?,
      AstKind::ContinueStatement(it) => it.label.as_ref()?,
      _ => return None,
    };
    Some(Ident { name: Name(label.name.as_str()), span: span(label.span), string: false })
  }

  pub(crate) fn as_block(self) -> Option<List<'a, Stmt<'a>>> {
    match self.kind() {
      StmtKind::Block(body) => Some(body),
      _ => None,
    }
  }

  pub(crate) fn is_with(self) -> bool {
    self.tag() == StmtTag::With
  }

  pub(crate) fn as_expr(self) -> Option<Expr<'a>> {
    match self.kind() {
      StmtKind::Expr(e) => Some(e),
      _ => None,
    }
  }

  pub(crate) fn as_var(self) -> Option<List<'a, VarDecl<'a>>> {
    match self.kind() {
      StmtKind::Var(decls) => Some(decls),
      _ => None,
    }
  }

  pub(crate) fn as_fn(self) -> Option<Func<'a>> {
    match self.kind() {
      StmtKind::Fn(func) => Some(func),
      _ => None,
    }
  }

  pub(crate) fn as_class(self) -> Option<Class<'a>> {
    match self.kind() {
      StmtKind::Class(class) => Some(class),
      _ => None,
    }
  }

  pub(crate) fn as_interface(self) -> Option<Interface<'a>> {
    match self.kind() {
      StmtKind::Interface(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn as_module(self) -> Option<Module<'a>> {
    match self.kind() {
      StmtKind::Module(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn as_import(self) -> Option<Import<'a>> {
    match self.kind() {
      StmtKind::Import(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn as_export_named(self) -> Option<Export<'a>> {
    match self.kind() {
      StmtKind::ExportNamed(it) => Some(it),
      _ => None,
    }
  }

  pub(crate) fn as_return(self) -> Option<Option<Expr<'a>>> {
    match self.kind() {
      StmtKind::Return(value) => Some(value),
      _ => None,
    }
  }

  pub(crate) fn as_if(self) -> Option<(Expr<'a>, Stmt<'a>, Option<Stmt<'a>>)> {
    match self.kind() {
      StmtKind::If { test, yes, no } => Some((test, yes, no)),
      _ => None,
    }
  }

  pub(crate) fn as_switch(self) -> Option<(Expr<'a>, List<'a, Case<'a>>)> {
    match self.kind() {
      StmtKind::Switch { expr, cases } => Some((expr, cases)),
      _ => None,
    }
  }

  pub(crate) fn as_labeled(self) -> Option<(Name<'a>, Stmt<'a>)> {
    match self.kind() {
      StmtKind::Labeled { label, body } => Some((label, body)),
      _ => None,
    }
  }

  pub(crate) fn as_for_of(self) -> Option<(Stmt<'a>, Expr<'a>, Stmt<'a>, bool)> {
    match self.kind() {
      StmtKind::ForOf { left, expr, body, is_await } => Some((left, expr, body, is_await)),
      _ => None,
    }
  }

  #[allow(clippy::type_complexity)]
  pub(crate) fn as_for(
    self,
  ) -> Option<(Option<Stmt<'a>>, Option<Expr<'a>>, Option<Expr<'a>>, Stmt<'a>)> {
    match self.kind() {
      StmtKind::For { init, test, update, body } => Some((init, test, update, body)),
      _ => None,
    }
  }

  #[allow(clippy::type_complexity)]
  pub(crate) fn as_try(
    self,
  ) -> Option<(Stmt<'a>, Option<VarDecl<'a>>, Option<Stmt<'a>>, Option<Stmt<'a>>)> {
    match self.kind() {
      StmtKind::Try { block, param, handler, finalizer } => {
        Some((block, param, handler, finalizer))
      }
      _ => None,
    }
  }

  pub(crate) fn as_with(self) -> Option<(Expr<'a>, Stmt<'a>)> {
    match self.kind() {
      StmtKind::With { object, body } => Some((object, body)),
      _ => None,
    }
  }

  pub(crate) fn is_loop(self) -> bool {
    matches!(
      self.tag(),
      StmtTag::For | StmtTag::ForIn | StmtTag::ForOf | StmtTag::While | StmtTag::DoWhile
    )
  }

  /// `"use strict"` and the like: the text between the quotes of a directive.
  pub(crate) fn directive(self) -> Option<&'a [u8]> {
    match self.file.kind(self.id)? {
      AstKind::Directive(it) => {
        let text = self.file.slice(span(it.expression.span));
        Some(text.get(1..text.len().saturating_sub(1)).unwrap_or_default().as_bytes())
      }
      _ => None,
    }
  }
}

/// The declaration of an `export` as a node.
fn declaration_kind<'a>(declaration: &'a Declaration<'a>) -> AstKind<'a> {
  match declaration {
    Declaration::VariableDeclaration(it) => AstKind::VariableDeclaration(it),
    Declaration::FunctionDeclaration(it) => AstKind::Function(it),
    Declaration::ClassDeclaration(it) => AstKind::Class(it),
    Declaration::TSTypeAliasDeclaration(it) => AstKind::TSTypeAliasDeclaration(it),
    Declaration::TSInterfaceDeclaration(it) => AstKind::TSInterfaceDeclaration(it),
    Declaration::TSEnumDeclaration(it) => AstKind::TSEnumDeclaration(it),
    Declaration::TSNamespaceDeclaration(it) => AstKind::TSNamespaceDeclaration(it),
    Declaration::TSExternalModuleDeclaration(it) => AstKind::TSExternalModuleDeclaration(it),
    Declaration::TSGlobalDeclaration(it) => AstKind::TSGlobalDeclaration(it),
    Declaration::TSImportEqualsDeclaration(it) => AstKind::TSImportEqualsDeclaration(it),
  }
}

/// A list of statements, with the directives before them.
pub(crate) fn statements<'a>(
  file: &'a super::File<'a>,
  body: &'a [Statement<'a>],
) -> List<'a, Stmt<'a>> {
  List::run(file, body.first().map(|it| it.node_id().index() as u32), body.len())
}

/// `export * as name`, with its position.
fn export_name<'a>(file: &'a super::File<'a>, name: &'a ModuleExportName<'a>) -> Ident<'a> {
  let span = span(name.span());
  match name {
    ModuleExportName::StringLiteral(it) => {
      let quoted = file.slice(span);
      let inner = quoted.get(1..quoted.len().saturating_sub(1)).unwrap_or_default();
      Ident { name: Name(it.value.as_str().unwrap_or(inner)), span, string: true }
    }
    _ => Ident { name: Name(file.slice(span)), span, string: false },
  }
}

impl<'a> VarDecl<'a> {
  /// The binding: a declarator's `id`, or a `catch` parameter's pattern.
  pub(crate) fn pat(self) -> super::Pat<'a> {
    let id = match self.file.kind(self.id) {
      Some(AstKind::VariableDeclarator(it)) => it.id.node_id().index() as u32,
      Some(AstKind::CatchParameter(it)) => it.pattern.node_id().index() as u32,
      _ => super::NONE,
    };
    super::Pat::new(self.file, id)
  }

  pub(crate) fn ty(self) -> Option<super::TypeNode<'a>> {
    let annotation = match self.file.kind(self.id)? {
      AstKind::VariableDeclarator(it) => it.type_annotation.as_ref()?,
      AstKind::CatchParameter(it) => it.type_annotation.as_ref()?,
      _ => return None,
    };
    Some(super::TypeNode::new(self.file, annotation.type_annotation.node_id().index() as u32))
  }

  pub(crate) fn init(self) -> Option<Expr<'a>> {
    match self.file.kind(self.id)? {
      AstKind::VariableDeclarator(it) => {
        it.init.as_ref().map(|it| Expr::new(self.file, expression_id(it)))
      }
      _ => None,
    }
  }

  pub(crate) fn var_kind(self) -> VarKind {
    match self.file.kind(self.file.parent_id(self.id)) {
      Some(AstKind::VariableDeclaration(it)) => match it.kind {
        VariableDeclarationKind::Var => VarKind::Var,
        VariableDeclarationKind::Let => VarKind::Let,
        VariableDeclarationKind::Const => VarKind::Const,
        VariableDeclarationKind::Using => VarKind::Using,
        VariableDeclarationKind::AwaitUsing => VarKind::AwaitUsing,
      },
      _ => VarKind::Let,
    }
  }

  /// [`Flags::DEFINITE`] for `x!: T`.
  pub(crate) fn flags(self) -> Flags {
    match self.file.kind(self.id) {
      Some(AstKind::VariableDeclarator(it)) if it.definite => Flags::DEFINITE,
      _ => Flags::empty(),
    }
  }

  pub(crate) fn is_definite(self) -> bool {
    self.flags().contains(Flags::DEFINITE)
  }

  /// The pattern and its type annotation.
  pub(crate) fn binding_span(self) -> Span {
    let pat = self.pat().span();
    match self.ty() {
      Some(ty) => Span::new(pat.start, ty.span().end),
      None => pat,
    }
  }

  /// The `Var` statement, or the `Try` statement of a `catch` parameter.
  pub(crate) fn parent(self) -> Node<'a> {
    Node::VarDecl(self).parent()
  }
}

impl<'a> Case<'a> {
  fn raw_case(self) -> Option<&'a oxc_ast::ast::SwitchCase<'a>> {
    match self.file.kind(self.id)? {
      AstKind::SwitchCase(it) => Some(it),
      _ => None,
    }
  }

  /// `None` for `default`.
  pub(crate) fn test(self) -> Option<Expr<'a>> {
    let test = self.raw_case()?.test.as_ref()?;
    Some(Expr::new(self.file, expression_id(test)))
  }

  pub(crate) fn is_default(self) -> bool {
    self.raw_case().is_some_and(|it| it.test.is_none())
  }

  pub(crate) fn body(self) -> List<'a, Stmt<'a>> {
    match self.raw_case() {
      Some(it) => statements(self.file, &it.consequent),
      None => List::empty(self.file),
    }
  }

  /// The `Switch` statement.
  pub(crate) fn parent(self) -> Node<'a> {
    Node::Case(self).parent()
  }
}
