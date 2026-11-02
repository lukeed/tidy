//! YAML, formatted the way Prettier does, with ports of the parsers it uses: `yaml` 2.9.0, for the lexer, the
//! concrete syntax tree and the composer, and `yaml-unist-parser` 3.2.0, for the tree that Prettier prints.
//!
//! ```text
//! text ─ lexer ─→ lexemes ─ cst::Parser ─→ cst::Tree ─ compose::Composer ─→ Composition
//!      ─ transform::Transformer ─→ Root ─ attach ─→ Root with comments
//! ```

mod ast;
mod attach;
mod compose;
mod cst;
mod lexer;
mod print;
mod scalar;
mod transform;

pub use ast::{Chomping, Id, Kind, Node, Point, Position, Root};

use crate::Error;
use crate::stack::Stack;
use crate::syntax::{Rejected, Rejection};

#[derive(Copy, Clone, Debug)]
enum Abort {
  Syntax(Rejected),
  TooDeep,
}

/// Parses `text` into the tree that Prettier prints, with each comment attached to a node.
pub fn parse(text: &str) -> Result<Root, Error> {
  let mut parser = Parser::default();
  parser.parse(text)?;
  Ok(parser.root)
}

/// The buffers of parsing, reused from one text to the next.
#[derive(Default)]
struct Parser {
  lexemes: Vec<lexer::Lexeme>,
  tree: cst::Tree,
  composition: compose::Composition,
  root: Root,
}

impl Parser {
  fn parse(&mut self, text: &str) -> Result<(), Error> {
    if text.len() > u32::MAX as usize {
      return Err(Error::Syntax { message: "The file is too large".into(), at: 0 });
    }
    self.lexemes.clear();
    self.tree.clear();
    self.composition.clear();
    self.root.clear();
    lexer::Lexer::lex(text.as_bytes(), &mut self.lexemes);
    cst::Parser::parse(text.as_bytes(), &self.lexemes, &mut self.tree);
    let rejection = Rejection::default();
    let stack = Stack::new();
    let mut composer =
      compose::Composer::new(text, &self.tree, &mut self.composition, &rejection, &stack);
    composer.compose().map_err(|abort| match abort {
      Abort::Syntax(_) => {
        let reason = rejection.reason();
        Error::Syntax { message: reason.0.text().to_owned(), at: reason.1 as usize }
      }
      Abort::TooDeep => Error::TooDeep,
    })?;
    let mut transformer = transform::Transformer {
      text,
      tree: &self.tree,
      composition: &self.composition,
      lines: ast::Lines::new(text),
      root: &mut self.root,
    };
    transformer.transform();
    attach::attach(&mut self.root);
    let root = self.root.root;
    attach::update_positions(&mut self.root, root);
    Ok(())
  }
}

/// Options of the YAML printer, Prettier's by default.
#[derive(Clone, Debug)]
pub struct Options {
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: crate::doc::Eol,
  /// Prefer single quotes, where neither kind is in the text.
  pub single_quote: bool,
  /// Spaces inside the braces of a flow mapping.
  pub bracket_spacing: bool,
  /// A trailing comma in flow collections that break.
  pub trailing_comma: bool,
  pub wrap: crate::Wrap,
  /// The ignore directive, Prettier's `prettier-ignore` by default: a node after `# name` prints as it is written.
  pub ignore: String,
  /// The file's path. `.prettierrc`, `.stylelintrc` and `.lintstagedrc` are formatted as JSON when they are JSON.
  pub filepath: Option<String>,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: crate::doc::Eol::Lf,
      single_quote: false,
      bracket_spacing: true,
      trailing_comma: true,
      wrap: crate::Wrap::Preserve,
      ignore: "prettier-ignore".into(),
      filepath: None,
    }
  }
}

/// True if Prettier formats the file at `path` as YAML.
pub fn is_yaml_path(path: &str) -> bool {
  let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_ascii_lowercase();
  NAMES.contains(&name.as_str()) || EXTENSIONS.iter().any(|extension| name.ends_with(extension))
}

/// The file names and extensions of Prettier's YAML language. Prettier compares them with the lowercased file name.
const NAMES: [&str; 10] = [
  ".clang-format",
  ".clang-tidy",
  ".clangd",
  ".gemrc",
  "citation.cff",
  "glide.lock",
  "pixi.lock",
  ".prettierrc",
  ".stylelintrc",
  ".lintstagedrc",
];
const EXTENSIONS: [&str; 10] = [
  ".yml",
  ".mir",
  ".reek",
  ".rviz",
  ".sublime-syntax",
  ".syntax",
  ".yaml",
  ".yaml-tmlanguage",
  ".yaml.sed",
  ".yml.mysql",
];

/// Formats `text`.
pub fn format(text: &str, options: &Options) -> Result<String, Error> {
  Formatter::default().format(text, options).map(str::to_owned)
}

/// Formats one stream after another, reusing its buffers.
#[derive(Default)]
pub struct Formatter {
  parser: Parser,
  bump: bumpalo::Bump,
  docs: crate::doc::Scratch,
  output: String,
}

impl Formatter {
  /// Formats `text`.
  pub fn format(&mut self, text: &str, options: &Options) -> Result<&str, Error> {
    let (bom, text) = crate::without_bom(text);
    let text = String::from_utf8_lossy(&crate::strings::lf(text.as_bytes())).into_owned();
    // Prettier's core prints nothing for a text of white space only.
    if crate::strings::trim_js_whitespace(text.as_bytes()).is_empty() {
      self.output.clear();
      return Ok(crate::with_bom(bom, &mut self.output));
    }
    if let Some(json) = self.json(&text, options) {
      self.output = json;
      return Ok(crate::with_bom(bom, &mut self.output));
    }
    self.format_text(&text, options, 0)?;
    Ok(crate::with_bom(bom, &mut self.output))
  }

  /// Formats `text` as code that starts `depth` indents in, as an [`Embed`](crate::embed::Embed) hook answers.
  pub(crate) fn embedded(&mut self, text: &str, options: &Options, depth: usize) -> Option<&str> {
    let text = String::from_utf8_lossy(&crate::strings::lf(text.as_bytes())).into_owned();
    self.format_text(&text, options, depth).ok()?;
    self.output.truncate(self.output.trim_end().len());
    Some(&self.output)
  }

  /// Prettier's `embed` of YAML: a `.prettierrc`, `.stylelintrc` or `.lintstagedrc` that is JSON is formatted as JSON.
  fn json(&self, text: &str, options: &Options) -> Option<String> {
    let path = options.filepath.as_deref()?;
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if !matches!(name, ".prettierrc" | ".stylelintrc" | ".lintstagedrc") {
      return None;
    }
    let json = crate::json::Options {
      width: options.width,
      tab_width: options.tab_width,
      tabs: options.tabs,
      eol: options.eol,
      ..crate::json::Options::default()
    };
    crate::json::format(text, &json).ok()
  }

  fn format_text(&mut self, text: &str, options: &Options, depth: usize) -> Result<(), Error> {
    self.parser.parse(text)?;
    self.bump.reset();
    let mut docs = crate::doc::Docs::reuse(&self.bump, &mut self.docs);
    docs.reserve(text.len());
    let stack = Stack::new();
    let mut printer = print::Printer {
      root: &self.parser.root,
      text,
      options,
      docs: &mut docs,
      stack: &stack,
      printed: Default::default(),
      ancestors: Vec::new(),
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
    self.output = match depth {
      0 => crate::doc::print(&docs, &print, std::mem::take(&mut self.output)),
      _ => {
        let mut bytes = std::mem::take(&mut self.output).into_bytes();
        bytes.clear();
        crate::doc::print_at(&docs, &print, &mut bytes, depth, depth * options.tab_width);
        String::from_utf8(bytes)
          .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
      }
    };
    self.docs = docs.into_scratch();
    Ok(())
  }
}
