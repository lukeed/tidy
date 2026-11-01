//! A fast markup formatter with Prettier's style by default.

pub mod css;
pub mod doc;
pub mod embed;
mod front_matter;
pub mod handlebars;
pub mod html;
mod languages;
mod stack;
mod strings;
mod syntax;
mod text;
pub mod xml;

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
