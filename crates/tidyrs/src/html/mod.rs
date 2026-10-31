//! HTML and Vue, ported from Prettier's `src/language-html` by way of bun's port.
//!
//! ```text
//! text ─ lexer ─→ tokens ─ parser, parse ─→ tree ─ preprocess ─→ white space sensitivity ─ printer ─→ doc
//! ```

mod ast;
mod data;
mod embed;
mod entities;
mod error;
mod lexer;
mod parse;
mod parser;
mod preprocess;
mod print;
mod strings;
mod tag;
mod utilities;

/// How deep elements may nest before formatting gives up, rather than overflow the stack.
const MAX_DEPTH: u32 = 1000;

/// Prettier's `parser` option.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Parser {
  Html,
  Vue,
  Angular,
  Lwc,
  Mjml,
}

/// Prettier's `htmlWhitespaceSensitivity`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Whitespace {
  /// Decided by the default value of the CSS property `display`.
  #[default]
  Css,
  /// All of it.
  Strict,
  /// None of it.
  Ignore,
}

/// The `options` that Prettier's functions receive.
pub(crate) struct Options<'o> {
  pub(crate) parser: Parser,
  /// `originalText`, where every line break is `\n`.
  pub(crate) original_text: &'o [u8],
  pub(crate) whitespace: Whitespace,
  pub(crate) filepath: Option<&'o [u8]>,
  pub(crate) width: usize,
  pub(crate) tab_width: usize,
  pub(crate) single_attribute_per_line: bool,
  pub(crate) bracket_same_line: bool,
  pub(crate) vue_indent_script_and_style: bool,
  /// `parentParser`: the text is embedded in another language.
  pub(crate) embedded: bool,
  /// A Vue file with a TypeScript `<script>`, whose expressions are TypeScript.
  pub(crate) typescript: bool,
}

/// How to format a markup file.
#[derive(Clone, Debug)]
pub struct Settings {
  pub parser: Parser,
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: crate::doc::Eol,
  pub whitespace: Whitespace,
  pub single_attribute_per_line: bool,
  pub bracket_same_line: bool,
  pub vue_indent_script_and_style: bool,
  /// The file's path, which decides some details, like the case of `<!doctype html>`.
  pub filepath: Option<String>,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      parser: Parser::Html,
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: crate::doc::Eol::Lf,
      whitespace: Whitespace::Css,
      single_attribute_per_line: false,
      bracket_same_line: false,
      vue_indent_script_and_style: false,
      filepath: None,
    }
  }
}

/// Why a text can't be formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  /// A syntax error, with Prettier's message and the byte offset where it is.
  Syntax { message: String, at: usize },
  /// The elements nest too deeply.
  TooDeep,
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::Syntax { message, .. } => f.write_str(message),
      Error::TooDeep => f.write_str("the elements nest too deeply"),
    }
  }
}

impl std::error::Error for Error {}

/// Formats `text`, handing the code of other languages to `embed`.
pub fn format(
  text: &str,
  settings: &Settings,
  embed: &mut dyn crate::embed::Embed,
) -> Result<String, Error> {
  let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
  let text: std::borrow::Cow<str> = if text.contains('\r') {
    text.replace("\r\n", "\n").replace('\r', "\n").into()
  } else {
    text.into()
  };
  let source = text.as_bytes();
  let matter = crate::front_matter::parse(&text).map(|matter| matter.len);
  let content: std::borrow::Cow<[u8]> = match matter {
    Some(len) => {
      let mut blanked = source.to_vec();
      for byte in &mut blanked[..len] {
        if *byte != b'\n' {
          *byte = b' ';
        }
      }
      blanked.into()
    }
    None => source.into(),
  };
  let mut tree = ast::Tree::default();
  parse::parse(&content, matter, settings.parser, &mut tree).map_err(|error| match error {
    parse::ParseError::Syntax(error) => Error::Syntax {
      message: String::from_utf8_lossy(&error.message()).into_owned(),
      at: error.at as usize,
    },
    parse::ParseError::NestedTooDeeply => Error::TooDeep,
  })?;
  let typescript = settings.parser == Parser::Vue && has_typescript_script(&tree);
  let options = Options {
    parser: settings.parser,
    original_text: source,
    whitespace: settings.whitespace,
    filepath: settings.filepath.as_deref().map(str::as_bytes),
    width: settings.width,
    tab_width: settings.tab_width,
    single_attribute_per_line: settings.single_attribute_per_line,
    bracket_same_line: settings.bracket_same_line,
    vue_indent_script_and_style: settings.vue_indent_script_and_style,
    embedded: false,
    typescript,
  };
  if !preprocess::preprocess(&mut tree, &options) {
    return Err(Error::TooDeep);
  }
  let bump = bumpalo::Bump::new();
  let mut docs = crate::doc::Docs::new(&bump);
  let doc = print::Printer::new(&tree, &options, &mut docs, embed).print_root();
  let print = crate::doc::Options {
    width: settings.width,
    tab_width: settings.tab_width,
    tabs: settings.tabs,
    eol: settings.eol,
  };
  Ok(crate::doc::print(&mut docs, doc, &print))
}

/// `isVueSfcWithTypescriptScript`: a root `<script>` with `lang="ts"` or `lang="tsx"`.
fn has_typescript_script(tree: &ast::Tree<'_>) -> bool {
  tree.children(tree.root).any(|id| {
    let node = &tree[id];
    node.kind == ast::Kind::Element
      && node.is_full_name(b"script")
      && tree.attribute_value(id, b"lang").is_some_and(|lang| lang == b"ts" || lang == b"tsx")
  })
}
