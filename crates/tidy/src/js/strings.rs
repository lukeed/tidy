//! The string helpers of bun's `bun_core::strings` that the JS printer uses: tidy's own, and the rest ported from bun
//! (MIT). The text is UTF-8, so the WTF-8 helpers are the standard ones.

pub(crate) use crate::strings::*;

/// What `\s` matches: ECMAScript's `WhiteSpace` and `LineTerminator`.
pub(crate) fn is_js_whitespace(cp: u32) -> bool {
  match cp {
    0x09..=0x0D | 0x20 => true,
    0..0x80 => false,
    0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF => true,
    _ => false,
  }
}

/// `/^\s*$/u.test(text)`
pub(crate) fn is_all_js_whitespace(text: &[u8]) -> bool {
  trim_js_whitespace_start(text).is_empty()
}

/// `text.split(/\r\n|\n|\r/)`: the lines without their line breaks. There is always at least one.
pub(crate) fn split_crlf_lines(text: &[u8]) -> impl Iterator<Item = &[u8]> + Clone {
  let mut rest = Some(text);
  std::iter::from_fn(move || {
    let text = rest?;
    let Some(at) = index_of_any(text, b"\n\r") else {
      rest = None;
      return Some(text);
    };
    let len = if text[at..].starts_with(b"\r\n") { 2 } else { 1 };
    rest = Some(&text[at + len..]);
    Some(&text[..at])
  })
}

/// `buf += text.replace(/\r\n?/g, "\n")`
pub(crate) fn push_crlf_as_lf(buf: &mut Vec<u8>, text: &[u8]) {
  let mut rest = text;
  while let Some(at) = index_of_char_usize(rest, b'\r') {
    buf.extend_from_slice(&rest[..at]);
    buf.push(b'\n');
    rest = &rest[at + 1..];
    rest = rest.strip_prefix(b"\n").unwrap_or(rest);
  }
  buf.extend_from_slice(rest);
}

/// `text.replace(/\r\n?/g, "\n")`
pub(crate) fn crlf_as_lf(text: &[u8]) -> std::borrow::Cow<'_, [u8]> {
  if !contains_char(text, b'\r') {
    return std::borrow::Cow::Borrowed(text);
  }
  let mut normalized = Vec::with_capacity(text.len());
  push_crlf_as_lf(&mut normalized, text);
  std::borrow::Cow::Owned(normalized)
}

/// `str::trim_start` for bytes.
pub(crate) fn trim_unicode_whitespace_start(text: &[u8]) -> &[u8] {
  match std::str::from_utf8(text) {
    Ok(text) => text.trim_start().as_bytes(),
    Err(_) => text,
  }
}

/// `str::trim_end` for bytes, for `chars`.
pub(crate) fn trim_right<'t>(text: &'t [u8], chars: &[u8]) -> &'t [u8] {
  let end = text.iter().rposition(|byte| !chars.contains(byte)).map_or(0, |at| at + 1);
  &text[..end]
}

/// `str::split_once` for bytes.
pub(crate) fn split_once_char(text: &[u8], delimiter: u8) -> Option<(&[u8], &[u8])> {
  let at = index_of_char_usize(text, delimiter)?;
  Some((&text[..at], &text[at + 1..]))
}

/// The number of non-overlapping `needle`s in `text`.
pub(crate) fn count(text: &[u8], needle: &[u8]) -> usize {
  let mut count = 0;
  let mut rest = text;
  while let Some(at) = index_of(rest, needle) {
    count += 1;
    rest = &rest[at + needle.len()..];
  }
  count
}

/// The index of the first byte that isn't ASCII.
pub(crate) fn first_non_ascii(text: &[u8]) -> Option<u32> {
  text.iter().position(|byte| !byte.is_ascii()).map(|at| at as u32)
}

pub(crate) fn without_utf8_bom(text: &[u8]) -> &[u8] {
  text.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(text)
}

/// The number of UTF-16 code units of `text`, which is JavaScript's `length`.
pub(crate) fn element_length_utf8_into_utf16(text: &[u8]) -> usize {
  String::from_utf8_lossy(text).encode_utf16().count()
}

pub(crate) fn wtf8_to_utf16(text: &[u8]) -> Vec<u16> {
  String::from_utf8_lossy(text).encode_utf16().collect()
}

/// The code points of `text` with their byte offsets.
pub(crate) fn wtf8_codepoints(text: &[u8]) -> impl Iterator<Item = (usize, u32)> + '_ {
  std::str::from_utf8(text).unwrap_or_default().char_indices().map(|(at, c)| (at, c as u32))
}
