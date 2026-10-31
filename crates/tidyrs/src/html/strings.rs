//! Byte string helpers, with the names of bun's `bun_core::strings` that the ported modules use.

pub(crate) fn index_of_char_usize(text: &[u8], char: u8) -> Option<usize> {
  text.iter().position(|&byte| byte == char)
}

pub(crate) fn contains_char(text: &[u8], char: u8) -> bool {
  text.contains(&char)
}

pub(crate) fn index_of_any(text: &[u8], chars: &[u8]) -> Option<usize> {
  text.iter().position(|byte| chars.contains(byte))
}

pub(crate) fn index_of(text: &[u8], needle: &[u8]) -> Option<usize> {
  if needle.is_empty() {
    return Some(0);
  }
  text.windows(needle.len()).position(|window| window == needle)
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
pub(crate) fn collapse_js_whitespace(mut text: &[u8]) -> Vec<u8> {
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
  out
}
