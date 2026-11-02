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

/// Formats `text`, the JavaScript or TypeScript file at `path`, which tells which it is.
pub fn format(text: &str, path: &str) -> Result<String, crate::Error> {
  format_with(text, path, &FormatOptions::default())
}

pub(crate) fn format_with(text: &str, path: &str, options: &FormatOptions) -> Result<String, crate::Error> {
  let allocator = oxc_allocator::Allocator::default();
  let source_type = source_type(path);
  let module = parse(&allocator, text, source_type);
  let parsed = match module.diagnostics.is_empty() {
    true => module,
    false => match parse(&allocator, text, source_type.with_script(true)) {
      script if script.diagnostics.is_empty() => script,
      _ => module,
    },
  };
  if let Some(error) = parsed.diagnostics.first() {
    let at = error.labels.as_ref().first().map_or(0, |label| label.offset() as usize);
    return Err(crate::Error::Syntax { message: error.message.to_string(), at });
  }
  let source_type = parsed.program.source_type;
  let program = allocator.alloc(parsed.program);
  let file = ast::File::new(text, path.as_bytes(), source_type, program, &allocator);
  let mut scratch = Scratch::default();
  let mut out = Vec::new();
  match ir::run::format(&file, options, &mut scratch, &mut out, format_file) {
    Ok(()) => Ok(String::from_utf8_lossy(&out).into_owned()),
    Err(FormatError::NestedTooDeeply) => Err(crate::Error::TooDeep),
    Err(error) => Err(crate::Error::Syntax { message: format!("{error:?}"), at: 0 }),
  }
}

/// How Prettier reads the file at `path`: JavaScript with JSX, through Babel, and TypeScript with JSX for `.tsx` only.
fn source_type(path: &str) -> oxc_span::SourceType {
  let lower = path.to_ascii_lowercase();
  match lower.rsplit_once('.').map(|(_, extension)| extension) {
    Some("ts" | "mts" | "cts") => oxc_span::SourceType::ts(),
    Some("tsx") => oxc_span::SourceType::tsx(),
    _ => oxc_span::SourceType::jsx(),
  }
}

fn parse<'a>(
  allocator: &'a oxc_allocator::Allocator,
  text: &'a str,
  source_type: oxc_span::SourceType,
) -> oxc_parser::ParserReturn<'a> {
  let options = oxc_parser::ParseOptions { preserve_parens: true, ..oxc_parser::ParseOptions::default() };
  oxc_parser::Parser::new(allocator, text, source_type).with_options(options).parse()
}

pub(crate) fn format_file<'a>(file: &'a ast::File<'a>, f: &mut ir::formatter::Formatter<'a>) {
  print::program::write_program(file, f);
}
