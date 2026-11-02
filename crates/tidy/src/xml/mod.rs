//! XML as `@prettier/plugin-xml` formats it, with a port of the parser it uses, `@xml-tools/parser`.
//!
//! ```text
//! text ─ lexer::lex ─→ [Token] ─ parser::Parser ─→ Tree ─ print::Printer ─→ Docs
//! ```
//!
//! The plugin formats the content of any element named after a parser it knows, such as `<json>`, as code in that
//! language. This only does it for `<style>` and `<script>` with a `type` of `text/...`.

mod ast;
mod lexer;
mod parser;
mod print;
mod sort;

use crate::Error;
use crate::Whitespace;
use crate::doc::{self, Docs};
use crate::embed::Embed;
use crate::stack::Stack;
use crate::syntax::{Message, Rejected, Rejection};

#[derive(Copy, Clone, Debug)]
enum Abort {
  Syntax(Rejected),
  TooDeep,
}

/// The extensions of the languages that the plugin formats, without those that other languages mostly use: `.rs`,
/// `.mm`, `.mojo`, `.typ`, `.res`, `.mod`, `.sw`, `.gml`, `.ncl`, and `.mjml`, which Prettier formats as HTML.
const EXTENSIONS: &[&str] = &[
  "adml",
  "admx",
  "ant",
  "axaml",
  "axml",
  "brd",
  "builds",
  "ccproj",
  "ccxml",
  "clixml",
  "cproject",
  "cscfg",
  "csdef",
  "csl",
  "csproj",
  "ct",
  "dae",
  "depproj",
  "dita",
  "ditamap",
  "ditaval",
  "dll.config",
  "dotsettings",
  "filters",
  "fsproj",
  "fxml",
  "glade",
  "gmx",
  "gpx",
  "grxml",
  "gst",
  "hzp",
  "iml",
  "inx",
  "ivy",
  "jelly",
  "jsproj",
  "kid",
  "kml",
  "launch",
  "lvclass",
  "lvlib",
  "lvproj",
  "mdpolicy",
  "mpl",
  "mps",
  "msd",
  "mxml",
  "natvis",
  "ndproj",
  "nproj",
  "nuspec",
  "odd",
  "osm",
  "owl",
  "pkgproj",
  "plist",
  "pluginspec",
  "proj",
  "props",
  "ps1xml",
  "psc1",
  "pt",
  "qhelp",
  "rdf",
  "resx",
  "rss",
  "runsettings",
  "sch",
  "scxml",
  "sfproj",
  "shproj",
  "slnx",
  "srdf",
  "sttheme",
  "storyboard",
  "sublime-snippet",
  "svg",
  "targets",
  "tmcommand",
  "tmlanguage",
  "tmpreferences",
  "tmsnippet",
  "tmtheme",
  "tml",
  "ui",
  "urdf",
  "ux",
  "vbproj",
  "vcxproj",
  "vsixmanifest",
  "vssettings",
  "vstemplate",
  "vxml",
  "wixproj",
  "workflow",
  "wsdl",
  "wsf",
  "wxi",
  "wxl",
  "wxs",
  "x3d",
  "xacro",
  "xaml",
  "xib",
  "xlf",
  "xliff",
  "xmi",
  "xml",
  "xml.dist",
  "xmp",
  "xpl",
  "xproc",
  "xproj",
  "xsd",
  "xsl",
  "xslt",
  "xsp-config",
  "xsp.metadata",
  "xspec",
  "xul",
  "zcml",
];

/// The file names of the languages that the plugin formats.
const NAMES: &[&str] = &[
  ".classpath",
  ".cproject",
  ".project",
  "app.config",
  "nuget.config",
  "options.stylecop",
  "web.debug.config",
  "web.release.config",
  "web.config",
  "ant.xml",
  "build.xml",
  "packages.config",
  "pom.xml",
];

/// True if the plugin formats the file at `path`.
pub fn is_xml_path(path: &str) -> bool {
  let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_ascii_lowercase();
  NAMES.contains(&name.as_str())
    || EXTENSIONS.iter().any(|extension| {
      name.strip_suffix(extension).is_some_and(|stem| stem.len() > 1 && stem.ends_with('.'))
    })
}

/// `xmlQuoteAttributes`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Quotes {
  #[default]
  Preserve,
  /// Single quotes, with each `'` in the value as `&apos;`.
  Single,
  /// Double quotes, with each `"` in the value as `&quot;`.
  Double,
}

/// Options of the XML printer, the plugin's by default.
#[derive(Clone, Debug)]
pub struct Options {
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: doc::Eol,
  /// `xmlWhitespaceSensitivity`. `Auto` is the plugin's `preserve`: the text of an element prints as it is written,
  /// and an element without text puts each child on its own line.
  pub whitespace: Whitespace,
  /// `xmlSelfClosingSpace`: a space before `/>` and `?>`.
  pub self_closing_space: bool,
  /// `xmlSortAttributesByKey`
  pub sort_attributes: bool,
  pub quotes: Quotes,
  pub bracket_same_line: bool,
  pub single_attribute_per_line: bool,
  /// Prettier's `embeddedLanguageFormatting`: `"auto"` if true, `"off"` if false. Off, scripts and style sheets stay
  /// as they are written.
  pub format_embedded: bool,
  /// The ignore directive, Prettier's `prettier-ignore` by default. The content of an element between
  /// `<!-- name-start -->` and `<!-- name-end -->` prints as it is written.
  pub ignore: String,
  /// How tidy formats scripts, inside the width and indent of the document.
  #[cfg(feature = "js")]
  pub js: crate::js::Options,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: doc::Eol::Lf,
      whitespace: Whitespace::Strict,
      self_closing_space: true,
      sort_attributes: false,
      quotes: Quotes::Preserve,
      bracket_same_line: false,
      single_attribute_per_line: false,
      format_embedded: true,
      ignore: "prettier-ignore".into(),
      #[cfg(feature = "js")]
      js: crate::js::Options::default(),
    }
  }
}

/// Formats `text`, handing the code of other languages to `embed`.
pub fn format(text: &str, options: &Options, embed: &mut dyn Embed) -> Result<String, Error> {
  Formatter::default().format(text, options, embed).map(str::to_owned)
}

/// Formats one document after another, reusing its buffers.
#[derive(Default)]
pub struct Formatter {
  bump: bumpalo::Bump,
  docs: doc::Scratch,
  tokens: Vec<lexer::Token>,
  pending: Vec<ast::Item>,
  tree: ast::Tree,
  output: String,
}

impl Formatter {
  /// Formats `text`, handing the code of other languages to `embed`.
  pub fn format(
    &mut self,
    text: &str,
    options: &Options,
    embed: &mut dyn Embed,
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
    embed: &mut dyn Embed,
  ) -> Result<(), Error> {
    let text = match crate::strings::lf(text.as_bytes()) {
      std::borrow::Cow::Borrowed(_) => std::borrow::Cow::Borrowed(text),
      std::borrow::Cow::Owned(bytes) => {
        std::borrow::Cow::Owned(String::from_utf8_lossy(&bytes).into_owned())
      }
    };
    if crate::strings::trim_js_whitespace(text.as_bytes()).is_empty() {
      self.output.clear();
      return Ok(());
    }
    let rejection = Rejection::default();
    let stack = Stack::new();
    self.parse(&text, &rejection, &stack).map_err(|error| match error {
      Abort::Syntax(_) => {
        let reason = rejection.reason();
        Error::Syntax { message: reason.0.text().to_owned(), at: reason.1 as usize }
      }
      Abort::TooDeep => Error::TooDeep,
    })?;
    self.bump.reset();
    let mut docs = Docs::reuse(&self.bump, &mut self.docs);
    docs.reserve(text.len());
    let ignore =
      (format!("<!-- {}-start -->", options.ignore), format!("<!-- {}-end -->", options.ignore));
    let has_ignored_ranges = text.contains(&ignore.0);
    #[cfg(feature = "js")]
    let scripts = crate::js::Options {
      sort_imports: crate::js::sort_imports::in_markup(options.js.sort_imports.as_ref(), false),
      ..options.js.clone()
    };
    let mut printer = print::Printer {
      tree: &self.tree,
      text: &text,
      options,
      docs: &mut docs,
      embed,
      stack,
      has_ignored_ranges,
      ignore: (&ignore.0, &ignore.1),
      #[cfg(feature = "js")]
      scripts: &scripts,
      #[cfg(not(feature = "js"))]
      scripts: &(),
    };
    printer.document();
    if printer.stack.ran_out() {
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

  fn parse(&mut self, text: &str, rejection: &Rejection, stack: &Stack) -> Result<(), Abort> {
    if text.len() > u32::MAX as usize {
      return Err(Abort::Syntax(rejection.note(Message::TooLarge, 0)));
    }
    self.tokens.clear();
    self.tokens.reserve(text.len() / 8);
    self.pending.clear();
    self.tree.clear();
    lexer::lex(text.as_bytes(), &mut self.tokens, rejection).map_err(Abort::Syntax)?;
    let mut parser = parser::Parser {
      text,
      tokens: &self.tokens,
      tree: &mut self.tree,
      pending: &mut self.pending,
      rejection,
      stack,
      at: 0,
    };
    parser.document()
  }
}

/// Parses `text` into the tree that prints. For `bench/parse` only.
#[cfg(tidy_bench)]
pub fn bench_parse(text: &str) -> bool {
  let text = crate::without_bom(text).1;
  let text = match crate::strings::lf(text.as_bytes()) {
    std::borrow::Cow::Borrowed(_) => std::borrow::Cow::Borrowed(text),
    std::borrow::Cow::Owned(bytes) => {
      std::borrow::Cow::Owned(String::from_utf8_lossy(&bytes).into_owned())
    }
  };
  if crate::strings::trim_js_whitespace(text.as_bytes()).is_empty() {
    return true;
  }
  let mut formatter = Formatter::default();
  let result = formatter.parse(&text, &Rejection::default(), &Stack::new());
  std::hint::black_box((result.is_ok(), formatter)).0
}
