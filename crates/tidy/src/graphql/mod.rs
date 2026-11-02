//! GraphQL as Prettier formats it, with its `src/language-graphql`. Ported from bun's `src/format/graphql` (MIT).
//!
//! ```text
//! text ─ parse::parse ─→ Tree ─ comments::attach ─→ [Attached] ─ print::Printer ─→ Docs ─ doc::print ─→ String
//! ```

mod comments;
mod parse;
mod print;

use crate::Error;
use crate::doc::Eol;
use crate::stack::Stack;

/// True if tidy formats the file at `path` as GraphQL: `.graphql`, `.gql` and `.graphqls`, as Prettier does.
pub fn is_graphql_path(path: &str) -> bool {
  let lower = path.to_ascii_lowercase();
  [".graphql", ".gql", ".graphqls"].iter().any(|extension| lower.ends_with(extension))
}

/// Options of the GraphQL formatter, Prettier's by default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: Eol,
  /// `bracketSpacing`: `{ a: 1 }` rather than `{a: 1}`.
  pub bracket_spacing: bool,
  /// The comment that leaves the next node as it is written.
  pub ignore: String,
}

impl Default for Options {
  fn default() -> Self {
    Options {
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: Eol::Lf,
      bracket_spacing: true,
      ignore: "prettier-ignore".into(),
    }
  }
}

/// Formats `text`.
pub fn format(text: &str, options: &Options) -> Result<String, Error> {
  Formatter::default().format(text, options).map(str::to_owned)
}

/// Formats one GraphQL text after another, reusing its buffers.
#[derive(Default)]
pub struct Formatter {
  tree: parse::Tree,
  attached: Vec<comments::Attached>,
  bump: bumpalo::Bump,
  docs: crate::doc::Scratch,
  output: String,
}

impl Formatter {
  /// Formats `text`.
  pub fn format(&mut self, text: &str, options: &Options) -> Result<&str, Error> {
    let (bom, text) = crate::without_bom(text);
    let text = String::from_utf8_lossy(&crate::strings::lf(text.as_bytes())).into_owned();
    self.output.clear();
    // Prettier's core prints nothing for a text of white space only.
    if !crate::strings::trim_js_whitespace(text.as_bytes()).is_empty() {
      self.print(&text, options)?;
    }
    Ok(crate::with_bom(bom, &mut self.output))
  }

  /// Formats `text` as a code block of Markdown, without the line break at its end.
  pub(crate) fn embedded(&mut self, text: &str, options: &Options) -> Option<&str> {
    let text = String::from_utf8_lossy(&crate::strings::lf(text.as_bytes())).into_owned();
    self.print(&text, options).ok()?;
    self.output.truncate(self.output.trim_end().len());
    Some(&self.output)
  }

  fn print(&mut self, text: &str, options: &Options) -> Result<(), Error> {
    let stack = Stack::new();
    let parsed = parse::parse(text.as_bytes(), &mut self.tree, &stack);
    if stack.ran_out() {
      return Err(Error::TooDeep);
    }
    parsed?;
    comments::attach(text.as_bytes(), &self.tree, &mut self.attached);
    self.bump.reset();
    let mut docs = crate::doc::Docs::reuse(&self.bump, &mut self.docs);
    docs.reserve(text.len());
    let mut printer = print::Printer {
      text,
      tree: &self.tree,
      attached: &self.attached,
      docs: &mut docs,
      stack: &stack,
      options,
    };
    printer.document();
    if stack.ran_out() {
      return Err(Error::TooDeep);
    }
    let print = crate::doc::Options {
      width: options.width,
      tab_width: options.tab_width,
      tabs: options.tabs,
      eol: options.eol,
    };
    self.output = crate::doc::print(&docs, &print, std::mem::take(&mut self.output));
    self.docs = docs.into_scratch();
    Ok(())
  }
}

/// Parses `text` into the tree that prints. For `bench/parse` only.
#[cfg(tidy_bench)]
pub fn bench_parse(text: &str) -> bool {
  let stack = Stack::new();
  let mut tree = parse::Tree::default();
  let parsed = parse::parse(text.as_bytes(), &mut tree, &stack).is_ok();
  std::hint::black_box(tree);
  parsed
}
