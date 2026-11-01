//! A fast Markdown formatter.
//!
//! mdtidy formats in one pass over the events of [`pulldown_cmark`], with no tree and no layout
//! document in between. It normalizes how blocks are laid out and copies inline content as
//! written, so escapes, entities, links, and HTML stay exactly as they were.
//!
//! ```
//! let output = mdtidy::format("# Title\n* one\n* two\n", &mdtidy::Options::default());
//! assert_eq!(output, "# Title\n\n- one\n- two\n");
//! ```

mod directive;
mod front;
mod opener;
mod options;
mod table;
mod wrap;
mod writer;

pub use options::{
  Bullet, Emphasis, Fence, Heading, Numbering, Options, Preset, Rule, Strong, Wrap,
};

/// A code block or front matter, handed to the caller of [`format_with`] to format.
#[derive(Clone, Copy, Debug)]
pub struct Code<'a> {
  /// The language: the first word of a code block's info string, or `yaml` / `toml` for front
  /// matter.
  pub lang: &'a str,
  pub text: &'a str,
  /// The width left for the code inside its containers.
  pub width: usize,
}

/// Format Markdown `source`. Code blocks stay as written.
pub fn format(source: &str, options: &Options) -> String {
  writer::format(source, options, &mut |_| None)
}

/// Format Markdown `source`, passing every code block with a language, and front matter, to
/// `code`. Return the formatted text, or `None` to keep it as written.
///
/// # Example
/// ```
/// let output = mdtidy::format_with("```js\nlet  x\n```\n", &mdtidy::Options::default(), |code| {
///   (code.lang == "js").then(|| code.text.replace("  ", " "))
/// });
/// assert_eq!(output, "```js\nlet x\n```\n");
/// ```
pub fn format_with(
  source: &str,
  options: &Options,
  mut code: impl FnMut(Code<'_>) -> Option<String>,
) -> String {
  writer::format(source, options, &mut code)
}

/// Whether `source` is already formatted.
pub fn check(source: &str, options: &Options) -> bool {
  format(source, options) == source
}
