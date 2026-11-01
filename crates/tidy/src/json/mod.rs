//! JSON, JSON with comments and JSON5, formatted the way Prettier does.
//!
//! Prettier's four JSON parsers read the same text: an expression that Babel parses, which JSON allows. They differ in
//! how they print it, which [`Dialect`] chooses.
//!
//! ```text
//! text ─ parse::Parser ─→ Tree ─ comments::attach ─→ Comments ─ print::Printer ─→ Docs
//! ```

mod ast;
mod comments;
mod cook;
mod identifier;
mod number;
mod parse;
mod print;

pub use ast::{Comment, Items, Kind, Node, NodeId, Span, Tree};

use crate::Error;
use crate::doc::{self, Docs};
use crate::stack::Stack;
use crate::syntax::{Message, Rejected, Rejection};

#[derive(Copy, Clone, Debug)]
enum Abort {
  Syntax(Rejected),
  TooDeep,
}

/// How a JSON text prints: one of Prettier's JSON parsers.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Dialect {
  /// `json`: keys in double quotes, and no trailing commas.
  #[default]
  Json,
  /// `jsonc`: as `json`, with trailing commas when `trailing_comma` is set. The text can be empty.
  Jsonc,
  /// `json5`: keys without quotes where they can be.
  Json5,
  /// `json-stringify`: as `JSON.stringify(value, null, 2)` prints it, without comments.
  Stringify,
}

impl Dialect {
  /// The dialect of the file at `path`, as Prettier infers it, or `None` if it isn't JSON.
  pub fn of_path(path: &str) -> Option<Dialect> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_ascii_lowercase();
    let (_, dialect) =
      NAMES.iter().find(|(names, _)| names.contains(&name.as_str())).or_else(|| {
        EXTENSIONS.iter().find(|(extensions, _)| extensions.iter().any(|it| name.ends_with(it)))
      })?;
    Some(*dialect)
  }
}

/// The file names of each dialect, from Prettier's languages. These come before extensions.
const NAMES: [(&[&str], Dialect); 2] = [
  (&["package.json", "package-lock.json", "composer.json"], Dialect::Stringify),
  (
    &[
      ".all-contributorsrc",
      ".arcconfig",
      ".auto-changelog",
      ".c8rc",
      ".htmlhintrc",
      ".imgbotconfig",
      ".nycrc",
      ".tern-config",
      ".tern-project",
      ".watchmanconfig",
      ".babelrc",
      ".jscsrc",
      ".jshintrc",
      ".jslintrc",
      ".swcrc",
    ],
    Dialect::Json,
  ),
];

/// The extensions of each dialect, from Prettier's languages. Prettier compares them with the lowercased file name,
/// so those with capitals never match, and are left out: `.4DForm`, `.4DProject` and `.JSON-tmLanguage`.
const EXTENSIONS: [(&[&str], Dialect); 4] = [
  (&[".importmap"], Dialect::Stringify),
  (
    &[
      ".json",
      ".avsc",
      ".geojson",
      ".gltf",
      ".har",
      ".ice",
      ".json.example",
      ".mcmeta",
      ".sarif",
      ".slnlaunch",
      ".tact",
      ".tfstate",
      ".tfstate.backup",
      ".topojson",
      ".webapp",
      ".webmanifest",
      ".yy",
      ".yyp",
    ],
    Dialect::Json,
  ),
  (
    &[
      ".jsonc",
      ".code-snippets",
      ".code-workspace",
      ".sublime-build",
      ".sublime-color-scheme",
      ".sublime-commands",
      ".sublime-completions",
      ".sublime-keymap",
      ".sublime-macro",
      ".sublime-menu",
      ".sublime-mousemap",
      ".sublime-project",
      ".sublime-settings",
      ".sublime-theme",
      ".sublime-workspace",
      ".sublime_metrics",
      ".sublime_session",
    ],
    Dialect::Jsonc,
  ),
  (&[".json5"], Dialect::Json5),
];

/// `quoteProps`, which only JSON5 follows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum QuoteProps {
  /// Quotes only the keys that need them.
  #[default]
  AsNeeded,
  /// Quotes all keys of an object if one needs quotes.
  Consistent,
  /// Keeps each key as it is written.
  Preserve,
}

/// `objectWrap`
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum ObjectWrap {
  /// An object with a line break before its first key stays on more than one line.
  #[default]
  Preserve,
  /// An object goes on one line if it fits.
  Collapse,
}

/// Options of the JSON printer, Prettier's by default.
#[derive(Clone, Debug)]
pub struct Options {
  pub dialect: Dialect,
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: doc::Eol,
  /// `trailingComma` is not `"none"`: objects and arrays that break get a trailing comma in JSONC and JSON5.
  pub trailing_comma: bool,
  pub quote_props: QuoteProps,
  /// Prefer single quotes in JSON5, unless `quote_props` is `Preserve`.
  pub single_quote: bool,
  /// Spaces inside the braces of an object on one line.
  pub bracket_spacing: bool,
  pub object_wrap: ObjectWrap,
  /// The ignore directive, Prettier's `prettier-ignore` by default. A value or property with `// name` or `/* name */`
  /// before or after it prints as it is written.
  pub ignore: String,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      dialect: Dialect::Json,
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: doc::Eol::Lf,
      trailing_comma: true,
      quote_props: QuoteProps::AsNeeded,
      single_quote: false,
      bracket_spacing: true,
      object_wrap: ObjectWrap::Preserve,
      ignore: "prettier-ignore".into(),
    }
  }
}

/// Formats `text`.
pub fn format(text: &str, options: &Options) -> Result<String, Error> {
  Formatter::default().format(text, options).map(str::to_owned)
}

/// Parses `text` as any dialect does: comments, and an empty text, are allowed.
pub fn parse(text: &str) -> Result<Tree, Error> {
  let mut tree = Tree::default();
  parse_into(text, &mut tree, &mut Vec::new())?;
  Ok(tree)
}

fn parse_into(text: &str, tree: &mut Tree, pending: &mut Vec<Option<NodeId>>) -> Result<(), Error> {
  let rejection = Rejection::default();
  if text.len() > u32::MAX as usize {
    return Err(syntax(rejection.note(Message::TooLarge, 0), &rejection));
  }
  tree.clear();
  pending.clear();
  let stack = Stack::new();
  let mut parser = parse::Parser {
    text: text.as_bytes(),
    tree,
    pending,
    rejection: &rejection,
    stack: &stack,
    at: 0,
  };
  parser.document().map_err(|abort| match abort {
    Abort::Syntax(rejected) => syntax(rejected, &rejection),
    Abort::TooDeep => Error::TooDeep,
  })
}

fn syntax(_: Rejected, rejection: &Rejection) -> Error {
  let reason = rejection.reason();
  Error::Syntax { message: reason.0.text().to_owned(), at: reason.1 as usize }
}

/// Formats one text after another, reusing its buffers.
#[derive(Default)]
pub struct Formatter {
  bump: bumpalo::Bump,
  docs: doc::Scratch,
  tree: Tree,
  pending: Vec<Option<NodeId>>,
  comments: comments::Comments,
  output: String,
}

impl Formatter {
  /// Formats `text`.
  pub fn format(&mut self, text: &str, options: &Options) -> Result<&str, Error> {
    let (bom, text) = crate::without_bom(text);
    let text = match crate::strings::lf(text.as_bytes()) {
      std::borrow::Cow::Borrowed(_) => std::borrow::Cow::Borrowed(text),
      std::borrow::Cow::Owned(bytes) => {
        std::borrow::Cow::Owned(String::from_utf8_lossy(&bytes).into_owned())
      }
    };
    self.format_text(&text, options)?;
    Ok(crate::with_bom(bom, &mut self.output))
  }

  fn format_text(&mut self, text: &str, options: &Options) -> Result<(), Error> {
    if crate::strings::trim_js_whitespace(text.as_bytes()).is_empty() {
      self.output.clear();
      return Ok(());
    }
    parse_into(text, &mut self.tree, &mut self.pending)?;
    let rejected = |message, at: u32| {
      let rejection = Rejection::default();
      syntax(rejection.note(message, at), &rejection)
    };
    if options.dialect == Dialect::Stringify
      && let Some(comment) = self.tree.comments.first()
    {
      return Err(rejected(Message::CommentInJson, comment.span.start));
    }
    if self.tree.root.is_none() && options.dialect != Dialect::Jsonc {
      return Err(rejected(Message::UnexpectedEnd, text.len() as u32));
    }
    self.comments.attach(&self.tree, text.as_bytes());
    self.bump.reset();
    let mut docs = Docs::reuse(&self.bump, &mut self.docs);
    docs.reserve(text.len());
    let stack = Stack::new();
    let mut printer = print::Printer {
      tree: &self.tree,
      text,
      options,
      comments: &self.comments,
      docs: &mut docs,
      stack: &stack,
      quoted: false,
    };
    printer.document();
    if stack.ran_out() {
      return Err(Error::TooDeep);
    }
    let print = doc::Options {
      width: options.width,
      tab_width: options.tab_width,
      tabs: options.tabs,
      eol: options.eol,
    };
    self.output = doc::print(&docs, &print, std::mem::take(&mut self.output));
    self.docs = docs.into_scratch();
    Ok(())
  }
}
