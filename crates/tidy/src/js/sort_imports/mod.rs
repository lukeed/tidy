//! Sorts a file's imports the way Prettier's plugins and oxfmt do.
//!
//! Prettier's plugins are preprocessors: they turn a file's text into another text, which Prettier then parses and
//! formats. [`sorted_text`] returns that text. It matches the plugin's output byte for byte, so the formatter handles
//! it exactly as Prettier would, including comments in odd places. Most files already have sorted imports, which
//! `layout.rs` detects without parsing anything again.

mod babel;
mod collate;
mod compare;
mod generator;
mod ianvs;
mod layout;
mod native;
mod node;
mod organize;
mod oxfmt;
mod regex;
mod settings;
mod sort;
mod trivago;

pub(crate) use oxfmt::ImportRun;
pub use settings::Settings;

use crate::js::ast::File;

#[derive(Debug)]
enum How {
  Trivago(trivago::Options),
  Ianvs(ianvs::Options),
  Oxfmt(oxfmt::Options),
  /// `sortImports` from bunfig.toml or `--sort-imports`. Runs oxfmt's sorter plus `native.rs`.
  Native(oxfmt::Options),
  Organize(organize::Options),
}

/// How to sort imports: validated [`Settings`] with their regular expressions compiled, from [`Settings::compile`].
#[derive(Debug)]
pub struct SortImports {
  how: How,
  settings: Settings,
}

impl PartialEq for SortImports {
  fn eq(&self, other: &Self) -> bool {
    self.settings == other.settings
  }
}

impl Eq for SortImports {}

/// `file`'s text with its imports sorted. It has to be parsed and formatted in place of `file`. Returns `None` if
/// formatting `file` gives the same result.
pub(crate) fn sorted_text<'a>(file: &'a File<'a>, how: &SortImports) -> Option<Vec<u8>> {
  if file.has_parse_errors() || matches!(how.how, How::Oxfmt(_)) {
    return None;
  }
  if matches!(how.how, How::Native(_)) {
    return native::preprocess(file);
  }
  // Prettier's `guessEndOfLine`
  let text = file.text();
  let end_of_line: &[u8] = match crate::js::strings::index_of_char_usize(text, b'\n') {
    Some(at) if at > 0 && text[at - 1] == b'\r' => b"\r\n",
    _ => b"\n",
  };
  if let How::Organize(options) = &how.how {
    return organize::preprocess(file, options, end_of_line);
  }
  let mut model = babel::Model::new(file)?;
  match &how.how {
    How::Trivago(options) => trivago::preprocess(&mut model, options, end_of_line),
    How::Ianvs(options) => ianvs::preprocess(&mut model, options, end_of_line),
    How::Oxfmt(_) | How::Native(_) | How::Organize(_) => None,
  }
}
