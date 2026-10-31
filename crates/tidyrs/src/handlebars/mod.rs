//! Handlebars as Glimmer parses it: Prettier's `language-handlebars`, with ports of the parsers it uses. Ported from
//! bun's `src/format/handlebars` (MIT).
//!
//! ```text
//! text ─ lexer::lex ─→ [Token] ─ parser::parse ─→ [Statement] ─ tokenizer::build ─→ Tree ─ printer::Printer ─→ Docs
//!        @handlebars/parser                                     simple-html-tokenizer, @glimmer/syntax
//! ```
//!
//! Prettier prints what the parsers give it, and they drop and change things: there's no `<!DOCTYPE html>` in the tree,
//! and `{{this/a}}` becomes `{{a}}`. Its printer changes more, for example by joining the lines of a `<script>`. Here
//! the output always means the same as the template. Where Prettier's output does too, this matches it. Where it
//! doesn't, this prints what Prettier should have printed, or the source text of the smallest node around the problem
//! (`ast::Node::is_as_written`).

mod ast;
mod error;
mod lexer;
mod parser;
mod printer;
mod tokenizer;

use crate::doc::{self, Docs};
use crate::embed::Embed;
use crate::stack::Stack;
use error::{Message, Rejected, Rejection};

#[derive(Copy, Clone, Debug)]
enum Error {
  Syntax(Rejected),
  NestedTooDeeply,
}

/// True if Prettier treats the file at `path` as Handlebars.
pub fn is_handlebars_path(path: &str) -> bool {
  let path = path.to_ascii_lowercase();
  path.ends_with(".hbs") || path.ends_with(".handlebars")
}

/// Allocations that are reused for the next file.
#[derive(Default)]
pub struct Scratch {
  tokens: Vec<lexer::Token>,
  statements: Vec<parser::Statement>,
  tree: ast::Tree,
}

/// Prettier's `parse`. `content` has no `\r`, and its front matter, which ends at `front_matter_end`, has been replaced
/// with spaces. Returns the template node.
fn parse(
  content: &[u8],
  front_matter_end: usize,
  scratch: &mut Scratch,
  rejection: &Rejection,
) -> Result<ast::NodeId, Error> {
  let Scratch { tokens, statements, tree } = scratch;
  tokens.clear();
  statements.clear();
  tree.clear();
  // Offsets are stored in 31 bits.
  if content.len() > i32::MAX as usize {
    return Err(Error::Syntax(rejection.note(Message::TooLarge, 0)));
  }
  lexer::lex(content, tokens, rejection)?;
  parser::parse(content, tokens, tree, statements, rejection)?;
  let front_matter =
    (front_matter_end > 0).then(|| tree.add(ast::Kind::FrontMatter, 0, front_matter_end));
  tokenizer::build(content, statements, front_matter, tree, rejection)
}

/// Options of the Handlebars printer, Prettier's by default.
#[derive(Clone, Debug)]
pub struct Settings {
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: doc::Eol,
  /// `htmlWhitespaceSensitivity`: Glimmer only tells `ignore` from the rest.
  pub whitespace: crate::html::Whitespace,
  pub single_quote: bool,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: doc::Eol::Lf,
      whitespace: crate::html::Whitespace::Css,
      single_quote: false,
    }
  }
}

/// Options of the printer, as it uses them.
pub(crate) struct Options {
  pub(crate) width: usize,
}

/// Why a template could not be formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
  /// A syntax error, with its message and the byte offset where it is.
  Syntax { message: String, at: usize },
  /// The template nests too deeply.
  TooDeep,
}

impl std::fmt::Display for Failure {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Failure::Syntax { message, .. } => f.write_str(message),
      Failure::TooDeep => f.write_str("the template nests too deeply"),
    }
  }
}

impl std::error::Error for Failure {}

/// Formats `text`, handing the code of other languages to `embed`.
pub fn format(text: &str, settings: &Settings, embed: &mut dyn Embed) -> Result<String, Failure> {
  Formatter::default().format(text, settings, embed).map(str::to_owned)
}

/// Formats one template after another, reusing its buffers.
#[derive(Default)]
pub struct Formatter {
  bump: bumpalo::Bump,
  docs: doc::Scratch,
  scratch: Scratch,
  output: String,
}

impl Formatter {
  /// Formats `text`, handing the code of other languages to `embed`. Prettier's `formatWithCursor`, without the cursor.
  pub fn format(
    &mut self,
    text: &str,
    settings: &Settings,
    embed: &mut dyn Embed,
  ) -> Result<&str, Failure> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    // Prettier also counts a no-break space as whitespace here.
    if text.trim_ascii().is_empty() {
      return Ok("");
    }
    let text: std::borrow::Cow<str> = match text.contains('\r') {
      true => text.replace("\r\n", "\n").replace('\r', "\n").into(),
      false => text.into(),
    };
    let front_matter_end = crate::front_matter::parse(&text).map_or(0, |matter| matter.len);
    let mut content = String::from(&*text);
    if front_matter_end > 0 {
      let blanked: String =
        content[..front_matter_end].chars().map(|c| if c == '\n' { '\n' } else { ' ' }).collect();
      content.replace_range(..front_matter_end, &blanked);
    }
    let rejection = Rejection::default();
    let template = parse(content.as_bytes(), front_matter_end, &mut self.scratch, &rejection)
      .map_err(|error| match error {
        Error::Syntax(_) => {
          let reason = rejection.reason();
          Failure::Syntax { message: reason.0.text().to_owned(), at: reason.1 as usize }
        }
        Error::NestedTooDeeply => Failure::TooDeep,
      })?;
    if self.scratch.tree.has_lost_text {
      self.output.clear();
      self.output.push_str(&text);
      return Ok(&self.output);
    }
    self.bump.reset();
    let mut docs = Docs::reuse(&self.bump, &mut self.docs);
    docs.reserve(content.len());
    let options = Options { width: settings.width };
    let mut printer = printer::Printer {
      tree: &self.scratch.tree,
      source: &content,
      front_matter: &text[..front_matter_end],
      options: &options,
      is_white_space_sensitive: settings.whitespace != crate::html::Whitespace::Ignore,
      single_quote: settings.single_quote,
      docs: &mut docs,
      embed,
      open: Vec::new(),
      indents: 0,
      pre_depth: 0,
      stack: Stack::new(),
    };
    printer.template(template);
    if printer.stack.ran_out() {
      return Err(Failure::TooDeep);
    }
    let print = doc::Options {
      width: settings.width,
      tab_width: settings.tab_width,
      tabs: settings.tabs,
      eol: settings.eol,
    };
    self.output = doc::print(&docs, &print, std::mem::take(&mut self.output));
    self.docs = docs.into_scratch();
    Ok(&self.output)
  }
}
