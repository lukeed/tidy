//! A fast markup formatter with Prettier's style by default.

#[cfg(feature = "config")]
pub mod config;
pub mod css;
pub mod doc;
pub mod embed;
mod front_matter;
pub mod handlebars;
pub mod html;
mod languages;
mod settings;
mod stack;
mod strings;
mod syntax;
mod text;
pub mod xml;

pub use settings::{Formatter, Language, Settings, Shared, format};

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

/// Puts back the byte order mark that `without_bom` split off.
pub(crate) fn with_bom(had: bool, output: &mut String) -> &str {
  if had {
    output.insert(0, '\u{FEFF}');
  }
  output
}
