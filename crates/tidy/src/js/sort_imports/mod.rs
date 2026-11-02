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

use std::sync::Arc;

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

impl SortImports {
  /// True if the formatter sorts the imports as it prints them, as oxfmt does, rather than a plugin before parsing.
  pub(crate) fn is_applied_by_format(&self) -> bool {
    matches!(self.how, How::Oxfmt(_) | How::Native(_))
  }

  /// True if it also sorts the imports of code in other languages. `prettier-plugin-organize-imports` only sorts files.
  pub(crate) fn applies_to_embedded_code(&self) -> bool {
    !matches!(self.how, How::Organize(_))
  }
}

/// How to sort the imports of the scripts in markup: oxfmt sorts those of a Vue file itself, and Prettier's plugins
/// those of other markup, which Prettier formats with `babel` or `typescript`. A plugin sorts a Vue file's scripts
/// before the file is parsed.
pub(crate) fn in_markup(how: Option<&Arc<SortImports>>, vue: bool) -> Option<Arc<SortImports>> {
  how
    .filter(|how| match how.is_applied_by_format() {
      true => vue,
      false => !vue && how.applies_to_embedded_code(),
    })
    .cloned()
}

/// How to sort the imports of a code block in Markdown.
pub(crate) fn in_markdown(how: Option<&Arc<SortImports>>) -> Option<Arc<SortImports>> {
  how.filter(|how| how.applies_to_embedded_code()).cloned()
}

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
