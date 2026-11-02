//! A fast formatter for markup, style sheets, data and Markdown, with Prettier's style by default.

#[cfg(feature = "config")]
pub mod config;
pub mod css;
pub mod doc;
pub mod dockerfile;
pub mod embed;
mod front_matter;
pub mod handlebars;
pub mod html;
pub mod ini;
pub mod json;
mod languages;
pub mod markdown;
mod options;
mod stack;
mod strings;
mod syntax;
mod text;
pub mod toml;
pub mod xml;
pub mod yaml;

pub use options::{Formatter, Language, Options, Shared, format};

/// Where whitespace is significant, with the same names in each language. Each language has its own default: `Auto`
/// for HTML, Vue and Handlebars, `Strict` for XML.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Whitespace {
  /// Wherever it could be. Prettier's `strict`.
  Strict,
  /// Where the language says it is. In HTML, Vue and Handlebars, where the default `display` of an element makes it
  /// significant: Prettier's `css`. In XML, in the text of an element, but not between elements: the plugin's
  /// `preserve`.
  Auto,
  /// Nowhere. Prettier's `ignore`.
  Ignore,
}

/// How prose breaks into lines: the words of Markdown paragraphs, and of YAML's flow scalars and folded block scalars.
/// Prettier's `proseWrap`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Wrap {
  /// Keep the line breaks as written.
  #[default]
  Preserve,
  /// Fill each line up to the line width.
  Always,
  /// Put each paragraph on one line.
  Never,
}

/// Why a text can't be formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  /// A syntax error: its message, and the byte offset where it is.
  Syntax { message: String, at: usize },
  /// The text nests too deeply to format.
  TooDeep,
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::Syntax { message, .. } => f.write_str(message),
      Error::TooDeep => f.write_str("the text nests too deeply to format"),
    }
  }
}

impl std::error::Error for Error {}

impl From<syntax::SyntaxError> for Error {
  fn from(error: syntax::SyntaxError) -> Self {
    Error::Syntax { message: error.0.text().to_owned(), at: error.1 as usize }
  }
}

/// Splits off a byte order mark. Prettier's `formatWithCursor` puts it back in front of the output.
pub(crate) fn without_bom(text: &str) -> (bool, &str) {
  match text.strip_prefix('\u{FEFF}') {
    Some(rest) => (true, rest),
    None => (false, text),
  }
}

/// Reads `text` without its byte order mark, if it has one, with the offsets of errors in `text`.
#[cfg(feature = "config")]
pub(crate) fn read_without_bom<T>(
  text: &str,
  read: fn(&str) -> Result<T, Error>,
) -> Result<T, Error> {
  let (bom, rest) = without_bom(text);
  let shift = if bom { '\u{FEFF}'.len_utf8() } else { 0 };
  read(rest).map_err(|error| match error {
    Error::Syntax { message, at } => Error::Syntax { message, at: at + shift },
    error => error,
  })
}

/// Puts back the byte order mark that `without_bom` split off.
pub(crate) fn with_bom(had: bool, output: &mut String) -> &str {
  if had {
    output.insert(0, '\u{FEFF}');
  }
  output
}

/// The parsers that aren't public, for `bench/parse` only: built with `--cfg tidy_bench`, never as a feature.
#[cfg(tidy_bench)]
pub mod bench {
  pub use crate::css::bench_parse as css;
  pub use crate::dockerfile::bench_parse as dockerfile;
  pub use crate::handlebars::bench_parse as handlebars;
  pub use crate::html::bench_parse as html;
  pub use crate::xml::bench_parse as xml;
}
