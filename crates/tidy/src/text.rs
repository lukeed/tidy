//! Text helpers shared by every language's printer: byte-slice versions of JavaScript's string and
//! regular expression operations, and Prettier's `src/utilities/`.
//!
//! An index is a byte offset. Where Prettier scans backwards from a character's index, these
//! functions take the offset just after that character:
//! `hasNewline(text, index, { backwards: true })`, which starts at `index - 1`, is
//! `has_newline_backwards(text, index)`.

use crate::strings;

/// `/^\s/.test(text)`
#[inline]
pub(crate) fn starts_with_white_space(text: &[u8]) -> bool {
  strings::js_whitespace_len(text) > 0
}

/// Length in bytes of the leading part of `text` that `\s` matches.
#[inline]
pub(crate) fn leading_white_space_len(text: &[u8]) -> usize {
  text.len() - strings::trim_js_whitespace_start(text).len()
}

/// A set of bytes, used to find where a token ends. The end is only a few bytes away, and there are
/// more terminators than the vectorized search accepts.
#[derive(Copy, Clone)]
pub(crate) struct ByteSet([bool; 256]);

impl ByteSet {
  pub(crate) const fn new(bytes: &[u8]) -> ByteSet {
    let mut set = [false; 256];
    let mut at = 0;
    while at < bytes.len() {
      set[bytes[at] as usize] = true;
      at += 1;
    }
    ByteSet(set)
  }

  /// The set, with the bytes from `first` through `last` added.
  pub(crate) const fn with_range(self, first: u8, last: u8) -> ByteSet {
    let mut set = self.0;
    let mut byte = first as usize;
    while byte <= last as usize {
      set[byte] = true;
      byte += 1;
    }
    ByteSet(set)
  }

  #[inline]
  pub(crate) fn has(&self, byte: u8) -> bool {
    self.0[byte as usize]
  }

  /// Index of the first byte at or after `from` that's in the set.
  #[inline]
  pub(crate) fn find(&self, text: &[u8], from: usize) -> Option<usize> {
    let rest = text.get(from..)?;
    rest.iter().position(|&byte| self.has(byte)).map(|at| from + at)
  }
}

/// `text.indexOf(part, from)`
#[inline]
pub(crate) fn index_of_from(text: &[u8], part: &[u8], from: usize) -> Option<usize> {
  strings::index_of(text.get(from..)?, part).map(|at| at + from)
}

#[inline]
fn skip(text: &[u8], at: usize, is_skipped: impl Fn(u8) -> bool) -> usize {
  let rest = text.get(at..).unwrap_or_default();
  at + rest.iter().take_while(|byte| is_skipped(**byte)).count()
}

/// `skipSpaces`
#[inline]
pub(crate) fn skip_spaces(text: &[u8], at: usize) -> usize {
  skip(text, at, |byte| matches!(byte, b' ' | b'\t'))
}

/// `skipSpaces(.., { backwards: true })`
#[inline]
pub(crate) fn skip_spaces_backwards(text: &[u8], end: usize) -> usize {
  let before = text.get(..end).unwrap_or_default();
  let is_space = |byte: &&u8| matches!(byte, b' ' | b'\t');
  before.len() - before.iter().rev().take_while(is_space).count()
}

/// `skipNewline`
#[inline]
pub(crate) fn skip_newline(text: &[u8], at: usize) -> usize {
  at + strings::js_line_break_len(text.get(at..).unwrap_or_default())
}

/// `skipNewline(.., { backwards: true })`
#[inline]
pub(crate) fn skip_newline_backwards(text: &[u8], end: usize) -> usize {
  end - strings::js_line_break_len_back(text.get(..end).unwrap_or_default())
}

/// `skipInlineComment`
#[inline]
pub(crate) fn skip_inline_comment(text: &[u8], at: usize) -> usize {
  let rest = text.get(at..).unwrap_or_default();
  match rest.strip_prefix(b"/*").and_then(|content| strings::index_of(content, b"*/")) {
    Some(end) => at + end + 4,
    None => at,
  }
}

/// `skipTrailingComment`
#[inline]
pub(crate) fn skip_trailing_comment(text: &[u8], at: usize) -> usize {
  let rest = text.get(at..).unwrap_or_default();
  match rest.starts_with(b"//") {
    true => at + strings::index_of_any(rest, b"\r\n").unwrap_or(rest.len()),
    false => at,
  }
}

/// `hasNewline`: only spaces and tabs are between `at` and the end of the line.
#[inline]
pub(crate) fn has_newline(text: &[u8], at: usize) -> bool {
  let line_end = skip_spaces(text, at);
  skip_newline(text, line_end) != line_end
}

/// `hasNewline(.., { backwards: true })`: only spaces and tabs are between the start of the line
/// and `end`. False on the first line.
#[inline]
pub(crate) fn has_newline_backwards(text: &[u8], end: usize) -> bool {
  let line_start = skip_spaces_backwards(text, end);
  skip_newline_backwards(text, line_start) != line_start
}

/// `isNextLineEmpty`: true if the line after `at`'s line is empty. Commas, semicolons and comments
/// after `at` are skipped.
#[inline]
pub(crate) fn is_next_line_empty(text: &[u8], at: usize) -> bool {
  match text.get(at..).unwrap_or_default() {
    // Fast paths: the line nearly always ends here or has more code on it.
    [b'\n', ..] => has_newline(text, at + 1),
    [b',', b' ', next, ..]
      if !matches!(next, b',' | b';' | b' ' | b'\t' | b'/' | b'\n' | b'\r' | 0xE2) =>
    {
      false
    }
    _ => is_next_line_empty_slow(text, at),
  }
}

fn is_next_line_empty_slow(text: &[u8], mut at: usize) -> bool {
  loop {
    let line_end = skip(text, at, |byte| matches!(byte, b',' | b';' | b' ' | b'\t'));
    let next = skip_spaces(text, skip_inline_comment(text, line_end));
    if next == at {
      break;
    }
    at = next;
  }
  has_newline(text, skip_newline(text, skip_trailing_comment(text, at)))
}

/// `isPreviousLineEmpty`: the line before the one of `start` has only spaces and tabs.
pub(crate) fn is_previous_line_empty(text: &[u8], start: usize) -> bool {
  let end = skip_spaces_backwards(text, start);
  let end = skip_newline_backwards(text, end);
  let end = skip_spaces_backwards(text, end);
  skip_newline_backwards(text, end) != end
}

/// `makeString`: writes `content`, the text between a string's quotes of either kind, wrapped in
/// `quote`.
pub(crate) fn make_string(content: &[u8], quote: u8, out: &mut Vec<u8>) {
  let other = if quote == b'"' { b'\'' } else { b'"' };
  out.push(quote);
  let mut bytes = content.iter().copied();
  while let Some(byte) = bytes.next() {
    match byte {
      b'\\' => match bytes.next() {
        // The other quote no longer needs escaping.
        Some(escaped) if escaped == other => out.push(escaped),
        Some(escaped) => out.extend([b'\\', escaped]),
        None => out.push(b'\\'),
      },
      _ if byte == quote => out.extend([b'\\', byte]),
      _ => out.push(byte),
    }
  }
  out.push(quote);
}

/// `text.toLowerCase()`
pub(crate) fn to_lower_case(text: &[u8]) -> std::borrow::Cow<'_, [u8]> {
  use std::borrow::Cow;
  if text.is_ascii() {
    return match text.iter().any(u8::is_ascii_uppercase) {
      true => Cow::Owned(text.to_ascii_lowercase()),
      false => Cow::Borrowed(text),
    };
  }
  let mut out = Vec::with_capacity(text.len());
  for chunk in text.utf8_chunks() {
    out.extend_from_slice(chunk.valid().to_lowercase().as_bytes());
    out.extend_from_slice(chunk.invalid());
  }
  Cow::Owned(out)
}
