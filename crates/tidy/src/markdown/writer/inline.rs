use std::borrow::Cow;

use super::Writer;

/// A link whose text is being written: what follows the text is copied as written.
pub(super) struct Link {
  /// Where the text ends so far.
  pub(super) text: usize,
  pub(super) end: usize,
}

/// `text` with each run of spaces and tabs as one space.
pub(super) fn collapse(text: &str) -> Cow<'_, str> {
  let bytes = text.as_bytes();
  if memchr::memchr(b'\t', bytes).is_none() && memchr::memmem::find(bytes, b"  ").is_none() {
    return Cow::Borrowed(text);
  }
  let mut out = String::with_capacity(text.len());
  for char in text.chars() {
    let space = matches!(char, ' ' | '\t');
    if !(space && out.ends_with(' ')) {
      out.push(if space { ' ' } else { char });
    }
  }
  Cow::Owned(out)
}

/// A code span rewritten as Prettier writes it, or `None` when `raw` already is: the fewest
/// backticks that no run inside matches, padded with a space only where the content needs one.
pub(super) fn span(raw: &str, value: &str) -> Option<String> {
  let ticks = raw.bytes().take_while(|&byte| byte == b'`').count();
  let inner = &raw[ticks..raw.len() - ticks];
  let unpadded = inner
    .strip_prefix([' ', '\n'])
    .and_then(|rest| rest.strip_suffix([' ', '\n']))
    .filter(|_| inner.bytes().any(|byte| !matches!(byte, b' ' | b'\n' | b'\r')))
    .unwrap_or(inner);
  let mut fewest = 1;
  while has_run(value, fewest) {
    fewest += 1;
  }
  let edge = |byte: Option<u8>| matches!(byte, Some(b' ' | b'\n'));
  let bytes = value.as_bytes();
  let pad = value.starts_with('`')
    || value.ends_with('`')
    || (edge(bytes.first().copied())
      && edge(bytes.last().copied())
      && bytes.iter().any(|byte| !matches!(byte, b' ' | b'\n')));
  let spaced = inner.starts_with(' ') && inner.ends_with(' ');
  if ticks == fewest && (unpadded.len() < inner.len()) == pad && (!pad || spaced) {
    return None;
  }
  let fence = "`".repeat(fewest);
  let pad = if pad { " " } else { "" };
  Some(format!("{fence}{pad}{unpadded}{pad}{fence}"))
}

/// Whether `text` has a run of exactly `length` backticks.
pub(super) fn has_run(text: &str, length: usize) -> bool {
  text.split(|char| char != '`').any(|run| run.len() == length)
}

impl<'s> Writer<'s, '_> {
  /// Whether emphasis ending at `end` touches a word on either side, where `_` doesn't work.
  pub(super) fn inside(&self, end: usize) -> bool {
    let before = match (&self.paragraph, &self.table) {
      (_, Some(table)) => {
        table.rows.last().and_then(|row| row.last()).and_then(|cell| cell.chars().next_back())
      }
      (Some(paragraph), None) => paragraph.last(),
      (None, None) if self.start => None,
      (None, None) => self.out.chars().next_back(),
    };
    let after = self.source[end..].chars().next();
    before.is_some_and(char::is_alphanumeric) || after.is_some_and(char::is_alphanumeric)
  }
}
