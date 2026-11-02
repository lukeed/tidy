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
mod tag;
mod utilities;

use crate::{Error, Whitespace};

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

/// The `options` that Prettier's functions receive.
pub(crate) struct Context<'o> {
  pub(crate) parser: Parser,
  /// `originalText`, where every line break is `\n`.
  pub(crate) original_text: &'o [u8],
  pub(crate) whitespace: Whitespace,
  pub(crate) filepath: Option<&'o [u8]>,
  pub(crate) width: usize,
  pub(crate) tab_width: usize,
  pub(crate) tabs: bool,
  pub(crate) single_attribute_per_line: bool,
  pub(crate) bracket_same_line: bool,
  pub(crate) vue_indent_script_and_style: bool,
  pub(crate) ignore: &'o [u8],
  /// `parentParser`: the text is embedded in another language.
  pub(crate) embedded: bool,
  /// A Vue file with a TypeScript `<script>`, whose expressions are TypeScript.
  pub(crate) typescript: bool,
  /// `embeddedLanguageFormatting` is `"auto"`.
  pub(crate) formats_embedded: bool,
  pub(crate) scripts: &'o crate::embed::Scripts,
}

/// How to format a markup file.
#[derive(Clone, Debug)]
pub struct Options {
  pub parser: Parser,
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: crate::doc::Eol,
  pub whitespace: Whitespace,
  pub single_attribute_per_line: bool,
  pub bracket_same_line: bool,
  pub vue_indent_script_and_style: bool,
  /// The ignore directive, Prettier's `prettier-ignore` by default. `<!-- name -->` keeps the next node as it is written,
  /// and `<!-- name-attribute -->` the attributes of the next element.
  pub ignore: String,
  /// The file's path, which decides some details, like the case of `<!doctype html>`.
  pub filepath: Option<String>,
  /// Prettier's `embeddedLanguageFormatting`: `"auto"` if true, `"off"` if false. Off, neither the embed hook nor
  /// tidy's own formatters see the code of other languages, which stays as it is written.
  pub format_embedded: bool,
  /// How tidy formats scripts and the expressions of templates, inside the width and indent of the markup.
  #[cfg(feature = "js")]
  pub js: crate::js::Options,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      parser: Parser::Html,
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: crate::doc::Eol::Lf,
      whitespace: Whitespace::Auto,
      single_attribute_per_line: false,
      bracket_same_line: false,
      vue_indent_script_and_style: false,
      ignore: "prettier-ignore".into(),
      filepath: None,
      format_embedded: true,
      #[cfg(feature = "js")]
      js: crate::js::Options::default(),
    }
  }
}

/// Formats `text`, handing the code of other languages to `embed`.
pub fn format(
  text: &str,
  options: &Options,
  embed: &mut dyn crate::embed::Embed,
) -> Result<String, Error> {
  Formatter::default().format(text, options, embed).map(str::to_owned)
}

/// Formats one text after another, reusing its buffers.
#[derive(Default)]
pub struct Formatter {
  bump: bumpalo::Bump,
  docs: crate::doc::Scratch,
  nodes: Vec<ast::Node<'static>>,
  attrs: Vec<ast::Attribute<'static>>,
  tokens: Vec<lexer::Token>,
  output: String,
}

impl Formatter {
  /// Formats `text`, handing the code of other languages to `embed`.
  pub fn format(
    &mut self,
    text: &str,
    options: &Options,
    embed: &mut dyn crate::embed::Embed,
  ) -> Result<&str, Error> {
    let (bom, text) = crate::without_bom(text);
    let embed = if options.format_embedded { embed } else { &mut crate::embed::Verbatim };
    self.format_text(text, options, embed)?;
    Ok(crate::with_bom(bom, &mut self.output))
  }

  fn format_text(
    &mut self,
    text: &str,
    options: &Options,
    embed: &mut dyn crate::embed::Embed,
  ) -> Result<(), Error> {
    if text.trim().is_empty() {
      self.output.clear();
      return Ok(());
    }
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
    self.bump.reset();
    let Self { bump, docs: scratch, nodes, attrs, tokens, output: buffer } = self;
    let mut tree = ast::Tree {
      nodes: crate::doc::recycle(std::mem::take(nodes)),
      attrs: crate::doc::recycle(std::mem::take(attrs)),
      tokens: std::mem::take(tokens),
      ..ast::Tree::default()
    };
    tree.nodes.reserve(content.len() / 32);
    tree.attrs.reserve(content.len() / 32);
    let mut docs = crate::doc::Docs::reuse(bump, scratch);
    docs.reserve(source.len());
    let output =
      run(&text, &content, matter, options, embed, &mut tree, &mut docs, std::mem::take(buffer));
    *scratch = docs.into_scratch();
    *nodes = crate::doc::recycle(tree.nodes);
    *attrs = crate::doc::recycle(tree.attrs);
    *tokens = tree.tokens;
    *buffer = output?;
    Ok(())
  }
}

#[allow(clippy::too_many_arguments)]
fn run<'a>(
  text: &'a str,
  content: &'a [u8],
  matter: Option<usize>,
  options: &Options,
  embed: &mut dyn crate::embed::Embed,
  tree: &mut ast::Tree<'a>,
  docs: &mut crate::doc::Docs<'a>,
  buffer: String,
) -> Result<String, Error> {
  let source = text.as_bytes();
  let clock = std::time::Instant::now();
  parse::parse(content, matter, options.parser, tree).map_err(|error| match error {
    parse::ParseError::Syntax(error) => Error::Syntax {
      message: String::from_utf8_lossy(&error.message()).into_owned(),
      at: error.at as usize,
    },
    parse::ParseError::NestedTooDeeply => Error::TooDeep,
  })?;
  let parsed = clock.elapsed();
  let typescript = options.parser == Parser::Vue && has_typescript_script(tree);
  let context = Context {
    parser: options.parser,
    original_text: source,
    whitespace: options.whitespace,
    filepath: options.filepath.as_deref().map(str::as_bytes),
    width: options.width,
    tab_width: options.tab_width,
    tabs: options.tabs,
    single_attribute_per_line: options.single_attribute_per_line,
    bracket_same_line: options.bracket_same_line,
    vue_indent_script_and_style: options.vue_indent_script_and_style,
    ignore: options.ignore.as_bytes(),
    embedded: false,
    typescript,
    formats_embedded: options.format_embedded,
    #[cfg(feature = "js")]
    scripts: &options.js,
    #[cfg(not(feature = "js"))]
    scripts: &(),
  };
  if !preprocess::preprocess(tree, &context) {
    return Err(Error::TooDeep);
  }
  let preprocessed = clock.elapsed();
  print::Printer::new(text, tree, &context, docs, embed).print_root();
  let print = crate::doc::Options {
    width: options.width,
    tab_width: options.tab_width,
    tabs: options.tabs,
    eol: options.eol,
  };
  let built = clock.elapsed();
  let output = crate::doc::print(docs, &print, buffer);
  if std::env::var_os("TIDY_STATS").is_some() {
    let printed = clock.elapsed();
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    eprintln!(
      "phases {:.3} {:.3} {:.3} {:.3}",
      ms(parsed),
      ms(preprocessed - parsed),
      ms(built - preprocessed),
      ms(printed - built)
    );
  }
  Ok(output)
}

/// `isVueSfcWithTypescriptScript`: a root `<script>` with `lang="ts"` or `lang="tsx"`.
fn has_typescript_script(tree: &ast::Tree<'_>) -> bool {
  tree.children(tree.root).any(|id| {
    let node = &tree[id];
    node.kind == ast::Kind::Element
      && node.is_full_name(b"script")
      && tree
        .attribute_value(id, b"lang")
        .is_some_and(|lang| lang == b"ts" || lang == b"typescript")
  })
}

/// Parses `text` into the tree that Prettier's `print-preprocess` then changes. For `bench/parse` only.
#[cfg(tidy_bench)]
pub fn bench_parse(text: &str) -> bool {
  let text = crate::strings::lf(crate::without_bom(text).1.as_bytes());
  let mut tree = ast::Tree::default();
  let result = parse::parse(&text, None, Parser::Html, &mut tree);
  std::hint::black_box((result.is_ok(), tree)).0
}
