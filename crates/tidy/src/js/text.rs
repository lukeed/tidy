//! Text helpers of the JS printer: tidy's `crate::text`, which is bun's `src/format/text.rs` (MIT), and the few that only
//! the printer uses.

pub(crate) use crate::text::*;

pub(crate) const BOM: &[u8] = b"\xEF\xBB\xBF";

/// The first character of `text` and its length in bytes. Invalid UTF-8 decodes as U+FFFD.
pub(crate) fn first_char(text: &[u8]) -> Option<(char, usize)> {
  let len = match *text.first()? {
    byte if byte < 0x80 => return Some((byte as char, 1)),
    byte if byte >= 0xF0 => 4,
    byte if byte >= 0xE0 => 3,
    _ => 2,
  };
  match text.get(..len).and_then(|bytes| std::str::from_utf8(bytes).ok()) {
    Some(it) => it.chars().next().map(|c| (c, len)),
    None => Some((char::REPLACEMENT_CHARACTER, 1)),
  }
}

/// The same for the last character.
pub(crate) fn last_char(text: &[u8]) -> Option<(char, usize)> {
  let last = *text.last()?;
  if last < 0x80 {
    return Some((last as char, 1));
  }
  let start = text.iter().rposition(|byte| byte & 0xC0 != 0x80).unwrap_or(0);
  match std::str::from_utf8(&text[start..]) {
    Ok(it) => it.chars().next_back().map(|c| (c, text.len() - start)),
    Err(_) => Some((char::REPLACEMENT_CHARACTER, 1)),
  }
}
