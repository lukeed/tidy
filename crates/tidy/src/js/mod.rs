//! JavaScript and TypeScript, with a port of bun's formatter onto oxc's parser. See `readme.md`.

pub(crate) mod ast;
pub(crate) mod ast_nodes;
pub(crate) mod attach;
pub(crate) mod builders;
pub(crate) mod comments;
pub(crate) mod context;
#[path = "detached/embed.rs"]
pub(crate) mod embed;
pub(crate) mod fields;
pub(crate) mod format;
pub(crate) mod ir;
#[path = "detached/jsdoc.rs"]
pub(crate) mod jsdoc;
pub(crate) mod options;
pub(crate) mod owners;
pub(crate) mod parentheses;
pub(crate) mod print;
pub(crate) mod sort;
#[path = "detached/sort_imports.rs"]
pub(crate) mod sort_imports;
pub(crate) mod source_text;
pub(crate) mod stack;
pub(crate) mod strings;
pub(crate) mod text;
pub(crate) mod trivia;
pub(crate) mod utils;

pub(crate) use ir::run::Scratch;
pub(crate) use options::FormatOptions;

// Not in the prelude: a glob import can't shadow the `std` macros with the same names.
pub(crate) use ir::macros::{best_fitting, format_args, write};

/// Imports that nearly every file of the printer needs.
pub(crate) mod base {
  pub(crate) use super::ast::*;
  pub(crate) use super::ast::{Span, Spanned};
  pub(crate) use super::ir::prelude::*;
  pub(crate) use super::options::*;
  pub(crate) use super::prelude::*;
}

pub(crate) mod prelude {
  pub(crate) use super::ast_nodes::{AsAstNodes, AstNodes, ChainElement};
  pub(crate) use super::builders::FormatSeparatedIter;
  pub(crate) use super::comments::{Comment, CommentExt, CommentList, Comments};
  pub(crate) use super::fields::{ExprFields, StmtFields};
  pub(crate) use super::source_text::SourceText;
  pub(crate) use super::trivia::{
    DanglingIndentMode, FormatDanglingComments, FormatLeadingComments, FormatTrailingComments, dangling_comments,
    leading_comments, trailing_comments,
  };
  pub(crate) use super::utils::operators::{BinOpExt, Precedence, UnOpExt};
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum FormatError {
  /// The file has syntax errors and is left as is.
  SyntaxError,
  /// The same, from one of tidy's parsers.
  SyntaxErrorAt(crate::syntax::SyntaxError),
  /// The syntax is nested so deeply that formatting it would overflow the stack.
  NestedTooDeeply,
  /// The formatted text is [longer than a string can be](is_too_long).
  TooLong,
  /// A bug in the formatter: the document it wrote is malformed.
  InvalidDocument,
}

/// The longest string that V8 has, which is where Prettier fails with `RangeError: Invalid string length`.
const MAX_STRING_LENGTH: u64 = (1 << 29) - 24;

/// True if a text of `len` bytes is longer than a JavaScript string can be. Something nested `n` deep is indented by
/// `n * n` spaces.
pub(crate) fn is_too_long(len: usize) -> bool {
  len as u64 > MAX_STRING_LENGTH
}

pub(crate) fn format_file<'a>(file: &'a ast::File<'a>, f: &mut ir::formatter::Formatter<'a>) {
  print::program::write_program(file, f);
}
