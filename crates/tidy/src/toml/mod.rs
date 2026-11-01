//! TOML as taplo formats it, by default, with ports of its lexer, parser and formatter. `taplo fmt` won't format a
//! document with a syntax error, so neither does this.
//!
//! ```text
//! text ─ parse::Parser (lexer::next) ─→ Tree ─ format::Formatter ─→ String
//! ```

mod format;
mod lexer;
mod parse;
mod tree;

pub use lexer::Token;
pub use tree::{Id, Kind, Tree};

use crate::Error;
use crate::doc;
use crate::stack::Stack;
use crate::syntax::{Message, Rejected, Rejection};

#[derive(Copy, Clone, Debug)]
enum Abort {
  Syntax(Rejected),
  TooDeep,
}

/// True if tidy formats the file at `path` as TOML: its name ends with `.toml`.
pub fn is_toml_path(path: &str) -> bool {
  let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
  name.len() > ".toml".len() && name.to_ascii_lowercase().ends_with(".toml")
}

/// Options of the TOML formatter, taplo's by default, with the same names.
#[derive(Clone, Debug)]
pub struct Options {
  /// `column_width`: arrays that go past it are expanded.
  pub column_width: usize,
  /// The width of an indent, in spaces.
  pub tab_width: usize,
  /// Indent with tabs.
  pub tabs: bool,
  pub eol: doc::Eol,
  /// Align the `=` of entries next to each other.
  pub align_entries: bool,
  /// Align the comments after entries and array items next to each other.
  pub align_comments: bool,
  /// Align a comment that is the only one, too.
  pub align_single_comments: bool,
  /// A trailing comma in an array on more than one line.
  pub array_trailing_comma: bool,
  /// Expand an array that goes past `column_width`.
  pub array_auto_expand: bool,
  /// Collapse an array without comments that fits on one line.
  pub array_auto_collapse: bool,
  /// Expand the arrays in an inline table that goes past `column_width`.
  pub inline_table_expand: bool,
  /// No spaces inside the brackets of an array on one line.
  pub compact_arrays: bool,
  /// No spaces inside the braces of an inline table.
  pub compact_inline_tables: bool,
  /// No spaces around `=`.
  pub compact_entries: bool,
  /// Indent a table under the table its key extends.
  pub indent_tables: bool,
  /// Indent the entries of a table.
  pub indent_entries: bool,
  pub trailing_newline: bool,
  /// Sort keys that no blank line separates.
  pub reorder_keys: bool,
  /// Sort the items of arrays.
  pub reorder_arrays: bool,
  /// Sort the entries of inline tables.
  pub reorder_inline_tables: bool,
  /// How many blank lines in a row are kept.
  pub allowed_blank_lines: usize,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      column_width: 80,
      tab_width: 2,
      tabs: false,
      eol: doc::Eol::Lf,
      align_entries: false,
      align_comments: true,
      align_single_comments: true,
      array_trailing_comma: true,
      array_auto_expand: true,
      array_auto_collapse: true,
      inline_table_expand: true,
      compact_arrays: true,
      compact_inline_tables: false,
      compact_entries: false,
      indent_tables: false,
      indent_entries: false,
      trailing_newline: true,
      reorder_keys: false,
      reorder_arrays: false,
      reorder_inline_tables: false,
      allowed_blank_lines: 2,
    }
  }
}

/// Formats `text`.
pub fn format(text: &str, options: &Options) -> Result<String, Error> {
  Formatter::default().format(text, options).map(str::to_owned)
}

/// Parses `text` into taplo's syntax tree.
pub fn parse(text: &str) -> Result<Tree, Error> {
  let mut tree = Tree::default();
  parse_into(text, &mut tree, &mut Vec::new(), &mut Vec::new())?;
  Ok(tree)
}

fn parse_into(
  text: &str,
  tree: &mut Tree,
  pending: &mut Vec<Id>,
  open: &mut Vec<(Kind, usize)>,
) -> Result<(), Error> {
  let rejection = Rejection::default();
  if text.len() > u32::MAX as usize {
    rejection.note(Message::TooLarge, 0);
    return Err(syntax(&rejection));
  }
  tree.clear();
  pending.clear();
  open.clear();
  let stack = Stack::new();
  let mut parser = parse::Parser {
    text: text.as_bytes(),
    tree,
    pending,
    open,
    rejection: &rejection,
    stack: &stack,
    span: (0, 0),
    current: None,
    skip_whitespace: true,
  };
  parser.document().map_err(|abort| match abort {
    Abort::Syntax(_) => syntax(&rejection),
    Abort::TooDeep => Error::TooDeep,
  })
}

fn syntax(rejection: &Rejection) -> Error {
  let reason = rejection.reason();
  Error::Syntax { message: reason.0.text().to_owned(), at: reason.1 as usize }
}

/// Formats one document after another, reusing its buffers.
#[derive(Default)]
pub struct Formatter {
  tree: Tree,
  pending: Vec<Id>,
  open: Vec<(Kind, usize)>,
  output: String,
}

impl Formatter {
  /// Formats `text`.
  pub fn format(&mut self, text: &str, options: &Options) -> Result<&str, Error> {
    let (bom, text) = crate::without_bom(text);
    parse_into(text, &mut self.tree, &mut self.pending, &mut self.open)?;
    let indent = match options.tabs {
      true => "\t".to_owned(),
      false => " ".repeat(options.tab_width),
    };
    let formatter =
      format::Formatter { tree: &self.tree, text, options, indent, newline: options.eol.text() };
    self.output = formatter.format();
    Ok(crate::with_bom(bom, &mut self.output))
  }
}
