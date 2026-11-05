//! HTML and Vue, ported from Prettier's `src/language-html` by way of bun's port.
//!
//! ```text
//! text ─ lexer ─→ tokens ─ parser, parse ─→ tree ─ preprocess ─→ white space sensitivity ─ printer ─→ doc
//! ```

mod angular;
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
    #[cfg(feature = "js")]
    let text = match with_sorted_scripts(&text, options) {
      Some(sorted) => sorted.into(),
      None => text,
    };
    let (content, matter) = without_front_matter(&text);
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
    docs.reserve(text.len());
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
  let stats = std::env::var_os("TIDY_STATS").is_some();
  let clock = stats.then(std::time::Instant::now);
  parse::parse(content, matter, options.parser, tree).map_err(|error| match error {
    parse::ParseError::Syntax(error) => Error::Syntax {
      message: String::from_utf8_lossy(&error.message()).into_owned(),
      at: error.at as usize,
    },
    parse::ParseError::NestedTooDeeply => Error::TooDeep,
  })?;
  let parsed = clock.map(|clock| clock.elapsed());
  let typescript = options.parser == Parser::Vue && has_typescript_script(tree);
  #[cfg(feature = "js")]
  let scripts = scripts_in(options);
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
    scripts: &scripts,
    #[cfg(not(feature = "js"))]
    scripts: &(),
  };
  if !preprocess::preprocess(tree, &context) {
    return Err(Error::TooDeep);
  }
  let preprocessed = clock.map(|clock| clock.elapsed());
  print::Printer::new(text, tree, &context, docs, embed).print_root();
  let print = crate::doc::Options {
    width: options.width,
    tab_width: options.tab_width,
    tabs: options.tabs,
    eol: options.eol,
  };
  let built = clock.map(|clock| clock.elapsed());
  let output = crate::doc::print(docs, &print, buffer);
  if let (Some(clock), Some(parsed), Some(preprocessed), Some(built)) =
    (clock, parsed, preprocessed, built)
  {
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

/// `text` with its front matter, if any, blanked out, and the length of the front matter.
fn without_front_matter(text: &str) -> (std::borrow::Cow<'_, [u8]>, Option<usize>) {
  let matter = crate::front_matter::parse(text).map(|matter| matter.len);
  let Some(len) = matter else { return (text.as_bytes().into(), None) };
  let mut blanked = text.as_bytes().to_vec();
  for byte in &mut blanked[..len] {
    if *byte != b'\n' {
      *byte = b' ';
    }
  }
  (blanked.into(), matter)
}

/// The options for the scripts in markup parsed with `options.parser`, with the imports sorted as they are there.
#[cfg(feature = "js")]
fn scripts_in(options: &Options) -> crate::js::Options {
  let how = options.js.sort_imports.as_ref();
  let sort_imports = crate::js::sort_imports::in_markup(how, options.parser == Parser::Vue);
  crate::js::Options { sort_imports, ..options.js.clone() }
}

/// `text`, a Vue file, with the imports of its first `<script>` and its first `<script setup>` sorted, as
/// `@trivago/prettier-plugin-sort-imports` and `@ianvs/prettier-plugin-sort-imports` sort them before the file is
/// parsed. `None` if nothing changes.
#[cfg(feature = "js")]
fn with_sorted_scripts(text: &str, options: &Options) -> Option<String> {
  let how = options.js.sort_imports.as_deref().filter(|how| {
    options.parser == Parser::Vue && !how.is_applied_by_format() && how.applies_to_embedded_code()
  })?;
  let (content, matter) = without_front_matter(text);
  let mut tree = ast::Tree::default();
  parse::parse(&content, matter, Parser::Vue, &mut tree).ok()?;
  let (mut sorted, mut from) = (String::new(), 0);
  let mut found = [false; 2];
  for id in tree.children(tree.root) {
    let node = &tree[id];
    if node.kind != ast::Kind::Element || !node.is_full_name(b"script") {
      continue;
    }
    let Some(end) = node.end_span() else { continue };
    let (start, end) = (node.start_span.end as usize, end.start as usize);
    let code = text.get(start..end)?;
    if code.trim().is_empty() && tree.attribute(id, b"src").is_none() {
      continue;
    }
    let setup = tree.attribute(id, b"setup").is_some();
    if std::mem::replace(&mut found[usize::from(setup)], true) {
      continue;
    }
    if let Some(code) = crate::js::template::sorted_script(code, how) {
      sorted.push_str(text.get(from..start)?);
      sorted.push('\n');
      sorted.push_str(&code);
      sorted.push('\n');
      from = end;
    }
  }
  (from > 0).then(|| sorted + &text[from..])
}

/// True if `text` parses as markup for `parser`.
#[cfg(feature = "js")]
pub(crate) fn parses(text: &str, parser: Parser) -> bool {
  parse::parse(text.as_bytes(), None, parser, &mut ast::Tree::default()).is_ok()
}

/// Writes the document of `text`, markup in a template of a script with placeholders for its substitutions, to `docs`:
/// Prettier's `textToDoc`, with a line break at the end. Returns the number of nodes at the top level. Fails if an
/// element whose name has a placeholder has no end tag: Prettier would add one, which evaluates the substitution twice.
#[cfg(feature = "js")]
pub(crate) fn template<'a>(
  text: &'a str,
  options: &Options,
  docs: &mut crate::doc::Docs<'a>,
) -> Result<usize, Error> {
  let mut tree = ast::Tree::default();
  parse::parse(text.as_bytes(), None, options.parser, &mut tree).map_err(|error| match error {
    parse::ParseError::Syntax(error) => Error::Syntax {
      message: String::from_utf8_lossy(&error.message()).into_owned(),
      at: error.at as usize,
    },
    parse::ParseError::NestedTooDeeply => Error::TooDeep,
  })?;
  let scripts = scripts_in(options);
  let context = Context {
    parser: options.parser,
    original_text: text.as_bytes(),
    whitespace: options.whitespace,
    filepath: options.filepath.as_deref().map(str::as_bytes),
    width: options.width,
    tab_width: options.tab_width,
    tabs: options.tabs,
    single_attribute_per_line: options.single_attribute_per_line,
    bracket_same_line: options.bracket_same_line,
    vue_indent_script_and_style: options.vue_indent_script_and_style,
    ignore: options.ignore.as_bytes(),
    embedded: true,
    typescript: false,
    formats_embedded: options.format_embedded,
    scripts: &scripts,
  };
  if !preprocess::preprocess(&mut tree, &context) {
    return Err(Error::TooDeep);
  }
  let repeats = tree.nodes.iter().any(|node| {
    node.kind == ast::Kind::Element
      && !node.flags.intersects(ast::Flags::IS_SELF_CLOSING | ast::Flags::HAS_END_SPAN)
      && crate::strings::contains(&node.name.to_ascii_uppercase(), b"PRETTIER_HTML_PLACEHOLDER_")
  });
  if repeats {
    return Err(Error::Syntax { message: "a substitution would repeat".into(), at: 0 });
  }
  print::Printer::new(text, &tree, &context, docs, &mut crate::embed::Verbatim).print_root();
  Ok(tree.children(tree.root).count())
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
