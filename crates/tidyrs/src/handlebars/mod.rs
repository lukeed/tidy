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
mod lexer;
mod parser;
mod printer;
mod tempura;
mod text;
mod tokenizer;

use crate::Error;
use crate::doc::{self, Docs};
use crate::embed::Embed;
use crate::stack::Stack;
use crate::syntax::{Message, Rejected, Rejection};

#[derive(Copy, Clone, Debug)]
enum Abort {
  Syntax(Rejected),
  NestedTooDeeply,
}

/// True if Prettier treats the file at `path` as Handlebars.
pub fn is_handlebars_path(path: &str) -> bool {
  let path = path.to_ascii_lowercase();
  path.ends_with(".hbs") || path.ends_with(".handlebars")
}

/// True if the template at `path` is of another language than HTML: `robots.txt.hbs`, `sitemap.xml.hbs`.
fn is_text(path: &str) -> bool {
  let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_ascii_lowercase();
  let stem =
    name.strip_suffix(".hbs").or_else(|| name.strip_suffix(".handlebars")).unwrap_or(&name);
  stem
    .rsplit_once('.')
    .is_some_and(|(base, extension)| !base.is_empty() && !matches!(extension, "html" | "htm"))
}

/// Allocations that are reused for the next file.
#[derive(Default)]
pub struct Scratch {
  tokens: Vec<lexer::Token>,
  statements: Vec<parser::Statement>,
  tree: ast::Tree,
}

/// The template language of a file.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Dialect {
  /// Tempura if the template uses syntax that only tempura has, or if its tags aren't Handlebars. Otherwise
  /// Handlebars.
  #[default]
  Auto,
  /// Handlebars as Glimmer reads it, printed as Prettier prints it: `{{value}}`.
  Handlebars,
  /// [Tempura](https://github.com/lukeed/tempura), whose tags hold JavaScript. Its values print with a space inside
  /// the braces: `{{ value }}`.
  Tempura,
}

impl Dialect {
  /// True if `content` is tempura in this dialect.
  fn is_tempura(self, content: &[u8]) -> bool {
    match self {
      Dialect::Auto => tempura::is_tempura(content),
      dialect => dialect == Dialect::Tempura,
    }
  }
}

/// Prettier's `parse`. `content` has no `\r`, and its front matter, which ends at `front_matter_end`, has been replaced
/// with spaces. Returns the template node, and whether the template is tempura.
fn parse(
  content: &[u8],
  front_matter_end: usize,
  dialect: Dialect,
  scratch: &mut Scratch,
  rejection: &Rejection,
) -> Result<(ast::NodeId, bool), Abort> {
  // Offsets are stored in 31 bits.
  if content.len() > i32::MAX as usize {
    return Err(Abort::Syntax(rejection.note(Message::TooLarge, 0)));
  }
  let mut is_tempura = dialect.is_tempura(content);
  let result = statements(content, is_tempura, scratch, rejection);
  // Not Handlebars at all, as in `{{ $utils.year() }}`. Handlebars that Glimmer doesn't support, such as a partial,
  // fails later, as it does in Prettier.
  if result.is_err() && dialect == Dialect::Auto && !is_tempura {
    let handlebars = rejection.reason();
    if statements(content, true, scratch, rejection).is_err() {
      rejection.note(handlebars.0, handlebars.1);
      return Err(result.err().unwrap_or(Abort::NestedTooDeeply));
    }
    is_tempura = true;
  } else {
    result?;
  }
  Ok((build(content, front_matter_end, scratch, rejection)?, is_tempura))
}

/// Parses the tags of the template into `scratch.statements`.
fn statements(
  content: &[u8],
  is_tempura: bool,
  scratch: &mut Scratch,
  rejection: &Rejection,
) -> Result<(), Abort> {
  let Scratch { tokens, statements, tree } = scratch;
  tokens.clear();
  statements.clear();
  tree.clear();
  match is_tempura {
    true => tempura::parse(content, tree, statements, rejection),
    false => {
      lexer::lex(content, tokens, rejection)?;
      parser::parse(content, tokens, tree, statements, rejection)
    }
  }
}

/// Builds the tree of HTML around the statements. Returns the template node.
fn build(
  content: &[u8],
  front_matter_end: usize,
  scratch: &mut Scratch,
  rejection: &Rejection,
) -> Result<ast::NodeId, Abort> {
  let Scratch { statements, tree, .. } = scratch;
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
  pub whitespace: crate::Whitespace,
  pub single_quote: bool,
  pub dialect: Dialect,
  /// The file's path. A template of another language than HTML, such as `robots.txt.hbs`, keeps its text as it is
  /// written.
  pub filepath: Option<String>,
  /// Prettier's `embeddedLanguageFormatting`: `"auto"` if true, `"off"` if false. Off, neither the embed hook nor
  /// tidyrs' own formatters see the code of other languages, which stays as it is written.
  pub format_embedded: bool,
  /// The ignore directive, Prettier's `prettier-ignore` by default. `{{! name }}` keeps the next node as it is written.
  pub ignore: String,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: doc::Eol::Lf,
      whitespace: crate::Whitespace::Auto,
      single_quote: false,
      dialect: Dialect::Auto,
      filepath: None,
      format_embedded: true,
      ignore: "prettier-ignore".into(),
    }
  }
}

/// Options of the printer, as it uses them.
pub(crate) struct Options {
  pub(crate) width: usize,
  pub(crate) tab_width: usize,
  /// `embeddedLanguageFormatting` is `"auto"`.
  pub(crate) formats_embedded: bool,
  /// The template is tempura.
  pub(crate) is_tempura: bool,
}

/// Formats `text`, handing the code of other languages to `embed`.
pub fn format(text: &str, settings: &Settings, embed: &mut dyn Embed) -> Result<String, Error> {
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
  ) -> Result<&str, Error> {
    let (bom, text) = crate::without_bom(text);
    let embed = if settings.format_embedded { embed } else { &mut crate::embed::Verbatim };
    self.format_text(text, settings, embed)?;
    Ok(crate::with_bom(bom, &mut self.output))
  }

  fn format_text(
    &mut self,
    text: &str,
    settings: &Settings,
    embed: &mut dyn Embed,
  ) -> Result<(), Error> {
    // Prettier also counts a no-break space as whitespace here.
    if text.trim_ascii().is_empty() {
      self.output.clear();
      return Ok(());
    }
    let text: std::borrow::Cow<str> = match text.contains('\r') {
      true => text.replace("\r\n", "\n").replace('\r', "\n").into(),
      false => text.into(),
    };
    if settings.filepath.as_deref().is_some_and(is_text) {
      let is_tempura = settings.dialect.is_tempura(text.as_bytes());
      let indent = if settings.tabs { "\t".to_owned() } else { " ".repeat(settings.tab_width) };
      let Some(formatted) = text::format(&text, is_tempura, &indent) else {
        return Err(Error::Syntax {
          message: Message::ExpectedEndOfMustache.text().to_owned(),
          at: 0,
        });
      };
      self.output = formatted;
      return Ok(());
    }
    let front_matter_end = crate::front_matter::parse(&text).map_or(0, |matter| matter.len);
    let mut content = String::from(&*text);
    if front_matter_end > 0 {
      let blanked: String =
        content[..front_matter_end].chars().map(|c| if c == '\n' { '\n' } else { ' ' }).collect();
      content.replace_range(..front_matter_end, &blanked);
    }
    let rejection = Rejection::default();
    let (template, is_tempura) =
      parse(content.as_bytes(), front_matter_end, settings.dialect, &mut self.scratch, &rejection)
        .map_err(|error| match error {
          Abort::Syntax(_) => {
            let reason = rejection.reason();
            Error::Syntax { message: reason.0.text().to_owned(), at: reason.1 as usize }
          }
          Abort::NestedTooDeeply => Error::TooDeep,
        })?;
    if self.scratch.tree.has_lost_text {
      self.output.clear();
      self.output.push_str(&text);
      return Ok(());
    }
    self.bump.reset();
    let mut docs = Docs::reuse(&self.bump, &mut self.docs);
    docs.reserve(content.len());
    let options = Options {
      width: settings.width,
      tab_width: settings.tab_width,
      formats_embedded: settings.format_embedded,
      is_tempura,
    };
    let mut printer = printer::Printer {
      tree: &self.scratch.tree,
      source: &content,
      front_matter: &text[..front_matter_end],
      options: &options,
      is_white_space_sensitive: settings.whitespace != crate::Whitespace::Ignore,
      single_quote: settings.single_quote,
      ignore: settings.ignore.as_bytes(),
      docs: &mut docs,
      embed,
      open: Vec::new(),
      indents: 0,
      pre_depth: 0,
      stack: Stack::new(),
    };
    printer.template(template);
    if printer.stack.ran_out() {
      return Err(Error::TooDeep);
    }
    let print = doc::Options {
      width: settings.width,
      tab_width: settings.tab_width,
      tabs: settings.tabs,
      eol: settings.eol,
    };
    self.output = doc::print(&docs, &print, std::mem::take(&mut self.output));
    self.docs = docs.into_scratch();
    Ok(())
  }
}
