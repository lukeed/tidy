//! The kinds and flags of bun's tree that the handles share: bun's `src/sema/hir.rs` (MIT).

bitflags::bitflags! {
  /// Modifiers, for every kind of node that can have them.
  #[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
  pub(crate) struct Flags: u32 {
    const EXPORT = 1 << 0;
    const DEFAULT = 1 << 1;
    /// `declare`, or inside a `declare` declaration, or in a declaration file.
    const AMBIENT = 1 << 2;
    const ABSTRACT = 1 << 3;
    const ASYNC = 1 << 4;
    const GENERATOR = 1 << 5;
    const STATIC = 1 << 6;
    const READONLY = 1 << 7;
    const OPTIONAL = 1 << 8;
    const PRIVATE = 1 << 9;
    const PROTECTED = 1 << 10;
    const PUBLIC = 1 << 11;
    const OVERRIDE = 1 << 12;
    const ACCESSOR = 1 << 13;
    /// `const enum`, `<const T>`.
    const CONST = 1 << 14;
    const REST = 1 << 15;
    /// `x!: T`
    const DEFINITE = 1 << 16;
    const TYPE_ONLY = 1 << 17;
    const IN = 1 << 18;
    const OUT = 1 << 19;
    /// A parameter that also declares a property of the class.
    const PARAMETER_PROPERTY = 1 << 20;
    /// A member whose name is a string or numeric literal: `"a": T`, `0: T`.
    const LITERAL_NAME = 1 << 21;
    /// A member whose name is a string literal: `"0": T`, `["0"]: T`.
    const STRING_NAME = 1 << 23;
    /// A function whose `{` is missing.
    const MISSING_BODY = 1 << 24;
    /// A member whose name is in brackets.
    const COMPUTED_NAME = 1 << 26;
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum FnKind {
  Decl,
  Expr,
  Arrow,
  Method,
  Getter,
  Setter,
  Constructor,
  StaticBlock,
  /// The remaining kinds never have a body: members of interfaces and type literals, and function types.
  CallSignature,
  ConstructSignature,
  FunctionType,
  ConstructorType,
  IndexSignature,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Keyword {
  Any,
  Unknown,
  Never,
  Void,
  Undefined,
  Null,
  String,
  Number,
  Boolean,
  BigInt,
  Symbol,
  Object,
  This,
  Intrinsic,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum MappedModifier {
  None,
  Add,
  Remove,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum MemberKind {
  Property,
  Method,
  Getter,
  Setter,
  Constructor,
  CallSignature,
  ConstructSignature,
  IndexSignature,
  StaticBlock,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum PropKind {
  Init,
  Shorthand,
  Spread,
  Method,
  Getter,
  Setter,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum VarKind {
  Var,
  Let,
  Const,
  Using,
  AwaitUsing,
}
