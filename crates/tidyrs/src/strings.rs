//! Byte string helpers, with the names of bun's `bun_core::strings` that the ported modules use.

pub(crate) fn index_of_char_usize(text: &[u8], char: u8) -> Option<usize> {
  memchr::memchr(char, text)
}

pub(crate) fn contains_char(text: &[u8], char: u8) -> bool {
  memchr::memchr(char, text).is_some()
}

pub(crate) fn index_of_any(text: &[u8], chars: &[u8]) -> Option<usize> {
  text.iter().position(|byte| chars.contains(byte))
}

pub(crate) fn index_of(text: &[u8], needle: &[u8]) -> Option<usize> {
  let Some((&first, rest)) = needle.split_first() else { return Some(0) };
  if needle.len() > 8 {
    return memchr::memmem::find(text, needle);
  }
  // A searcher costs more to set up than a short needle saves.
  let mut from = 0;
  while let Some(at) = memchr::memchr(first, &text[from..]) {
    let at = from + at;
    if text[at + 1..].starts_with(rest) {
      return Some(at);
    }
    from = at + 1;
  }
  None
}

pub(crate) fn contains(text: &[u8], needle: &[u8]) -> bool {
  index_of(text, needle).is_some()
}

/// `text.split(delimiter)`, with the empty fields.
pub(crate) fn split<'a>(text: &'a [u8], delimiter: &'a [u8]) -> impl Iterator<Item = &'a [u8]> {
  debug_assert_eq!(delimiter.len(), 1);
  let delimiter = delimiter[0];
  text.split(move |&byte| byte == delimiter)
}

/// The length in bytes of the character that `text` starts with, if `\s` matches it, and 0 if not.
pub(crate) fn js_whitespace_len(text: &[u8]) -> usize {
  match *text {
    [0x09..=0x0D | b' ', ..] => 1,
    [0xC2, 0xA0, ..] => 2,
    [0xE1, 0x9A, 0x80, ..]
    | [0xE2, 0x80, 0x80..=0x8A | 0xA8 | 0xA9 | 0xAF, ..]
    | [0xE2, 0x81, 0x9F, ..]
    | [0xE3, 0x80, 0x80, ..]
    | [0xEF, 0xBB, 0xBF, ..] => 3,
    _ => 0,
  }
}

fn js_whitespace_len_back(text: &[u8]) -> usize {
  match *text {
    [.., 0x09..=0x0D | b' '] => 1,
    [.., 0..0x80] | [] => 0,
    [.., 0xC2, 0xA0] => 2,
    [.., a, b, c] if js_whitespace_len(&[a, b, c]) == 3 => 3,
    _ => 0,
  }
}

/// `text.trimStart()`
pub(crate) fn trim_js_whitespace_start(mut text: &[u8]) -> &[u8] {
  while let len @ 1.. = js_whitespace_len(text) {
    text = &text[len..];
  }
  text
}

/// `text.trimEnd()`
pub(crate) fn trim_js_whitespace_end(mut text: &[u8]) -> &[u8] {
  loop {
    match js_whitespace_len_back(text) {
      0 => return text,
      len => text = &text[..text.len() - len],
    }
  }
}

/// `text.trim()`
pub(crate) fn trim_js_whitespace(text: &[u8]) -> &[u8] {
  trim_js_whitespace_end(trim_js_whitespace_start(text))
}

/// `text.length`
pub(crate) fn wtf8_len_utf16(text: &[u8]) -> u32 {
  match std::str::from_utf8(text) {
    Ok(text) => text.encode_utf16().count() as u32,
    Err(_) => text.len() as u32,
  }
}

/// `htmlWhitespace.trimEnd(text)`: without the trailing `[\t\n\f\r ]`.
pub(crate) fn trim_html_whitespace_end(text: &[u8]) -> &[u8] {
  let kept = text.iter().rposition(|byte| !matches!(byte, b'\t' | b'\n' | b'\x0C' | b'\r' | b' '));
  &text[..kept.map_or(0, |at| at + 1)]
}

/// `htmlWhitespace.trim(text)`
pub(crate) fn trim_html_whitespace(text: &[u8]) -> &[u8] {
  let is_space = |byte: &u8| matches!(byte, b'\t' | b'\n' | b'\x0C' | b'\r' | b' ');
  let start = text.iter().position(|byte| !is_space(byte)).unwrap_or(text.len());
  trim_html_whitespace_end(&text[start..])
}

/// `text.replaceAll(/\s+/g, " ")`
pub(crate) fn collapse_js_whitespace(mut text: &[u8]) -> std::borrow::Cow<'_, [u8]> {
  let single = text.iter().enumerate().all(|(at, &byte)| match byte {
    b' ' => text.get(at + 1) != Some(&b' '),
    0x09..=0x0D | 0x80.. => false,
    _ => true,
  });
  if single {
    return std::borrow::Cow::Borrowed(text);
  }
  let mut out = Vec::with_capacity(text.len());
  while !text.is_empty() {
    let len = js_whitespace_len(text);
    if len == 0 {
      out.push(text[0]);
      text = &text[1..];
      continue;
    }
    out.push(b' ');
    while let len @ 1.. = js_whitespace_len(text) {
      text = &text[len..];
    }
  }
  std::borrow::Cow::Owned(out)
}

pub(crate) fn count_char(text: &[u8], char: u8) -> usize {
  memchr::memchr_iter(char, text).count()
}

/// `/\r\n|[\r\n  ]/u.test(text)`
pub(crate) fn contains_js_line_break(text: &[u8]) -> bool {
  memchr::memchr3_iter(b'\r', b'\n', 0xE2, text)
    .any(|at| text[at] != 0xE2 || matches!(text.get(at + 1..at + 3), Some([0x80, 0xA8 | 0xA9])))
}

pub(crate) fn index_of_char_pos(text: &[u8], char: u8, from: usize) -> Option<usize> {
  memchr::memchr(char, text.get(from..)?).map(|at| at + from)
}

pub(crate) fn last_index_of_char(text: &[u8], char: u8) -> Option<usize> {
  memchr::memrchr(char, text)
}

pub(crate) fn last_index_of(text: &[u8], needle: &[u8]) -> Option<usize> {
  memchr::memmem::rfind(text, needle)
}

pub(crate) fn last_index_of_any(text: &[u8], chars: &[u8]) -> Option<usize> {
  text.iter().rposition(|byte| chars.contains(byte))
}

pub(crate) fn contains_any(text: &[u8], chars: &[u8]) -> bool {
  index_of_any(text, chars).is_some()
}

/// Splits `text` at each byte that is one of `chars`.
pub(crate) fn split_any<'a>(text: &'a [u8], chars: &'a [u8]) -> impl Iterator<Item = &'a [u8]> {
  text.split(move |byte| chars.contains(byte))
}

/// `\w` of a regular expression without flags.
pub(crate) fn is_regexp_word_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Prettier's `normalizeEndOfLine`: the text with each `\r\n` and `\r` as `\n`.
pub(crate) fn lf(text: &[u8]) -> std::borrow::Cow<'_, [u8]> {
  if !contains_char(text, b'\r') {
    return std::borrow::Cow::Borrowed(text);
  }
  let mut normalized = Vec::with_capacity(text.len());
  let mut rest = text;
  while let Some(at) = memchr::memchr(b'\r', rest) {
    normalized.extend_from_slice(&rest[..at]);
    normalized.push(b'\n');
    rest = rest[at + 1..].strip_prefix(b"\n").unwrap_or(&rest[at + 1..]);
  }
  normalized.extend_from_slice(rest);
  std::borrow::Cow::Owned(normalized)
}

/// The length in bytes of the line break that `text` starts with: `\r\n`, `\r`, `\n`, U+2028 or U+2029. 0 if none.
pub(crate) fn js_line_break_len(text: &[u8]) -> usize {
  match text {
    [b'\r', b'\n', ..] => 2,
    [b'\r' | b'\n', ..] => 1,
    [0xE2, 0x80, 0xA8 | 0xA9, ..] => 3,
    _ => 0,
  }
}

/// The same for the line break that `text` ends with.
pub(crate) fn js_line_break_len_back(text: &[u8]) -> usize {
  match text {
    [.., b'\r', b'\n'] => 2,
    [.., b'\r' | b'\n'] => 1,
    [.., 0xE2, 0x80, 0xA8 | 0xA9] => 3,
    _ => 0,
  }
}
