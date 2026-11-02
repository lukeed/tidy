//! The string helpers of bun's `bun_core::strings` that `jsdoc/` uses on top of [`crate::js::strings`].

pub(crate) use crate::js::strings::*;

/// The length in bytes of the character that `text` starts with, if it has Unicode's `White_Space` property, which
/// `char::is_whitespace` goes by, and 0 if not.
fn unicode_whitespace_len(text: &[u8]) -> usize {
  match *text {
    [0x09..=0x0D | b' ', ..] => 1,
    [0xC2, 0x85 | 0xA0, ..] => 2,
    [0xE1, 0x9A, 0x80, ..]
    | [0xE2, 0x80, 0x80..=0x8A | 0xA8 | 0xA9 | 0xAF, ..]
    | [0xE2, 0x81, 0x9F, ..]
    | [0xE3, 0x80, 0x80, ..] => 3,
    _ => 0,
  }
}

/// `str::trim_start`
pub(crate) fn trim_unicode_whitespace_start(mut text: &[u8]) -> &[u8] {
  while let len @ 1.. = unicode_whitespace_len(text) {
    text = &text[len..];
  }
  text
}

/// `str::trim_end`
pub(crate) fn trim_unicode_whitespace_end(mut text: &[u8]) -> &[u8] {
  loop {
    text = match *text {
      [ref rest @ .., 0x09..=0x0D | b' '] => rest,
      [.., 0..0x80] | [] => return text,
      [ref rest @ .., 0xC2, 0x85 | 0xA0] => rest,
      [ref rest @ .., a, b, c] if unicode_whitespace_len(&[a, b, c]) == 3 => rest,
      _ => return text,
    };
  }
}

/// `str::trim`
pub(crate) fn trim_unicode_whitespace(text: &[u8]) -> &[u8] {
  trim_unicode_whitespace_end(trim_unicode_whitespace_start(text))
}

/// `text.chars().all(char::is_whitespace)`
pub(crate) fn is_all_unicode_whitespace(text: &[u8]) -> bool {
  trim_unicode_whitespace_start(text).is_empty()
}

/// `str::split_whitespace`
pub(crate) fn split_unicode_whitespace(text: &[u8]) -> impl Iterator<Item = &[u8]> + Clone {
  let mut rest = text;
  std::iter::from_fn(move || {
    rest = trim_unicode_whitespace_start(rest);
    let len =
      (0..rest.len()).find(|&at| unicode_whitespace_len(&rest[at..]) > 0).unwrap_or(rest.len());
    let (word, after) = rest.split_at(len);
    rest = after;
    (!word.is_empty()).then_some(word)
  })
}

/// `input.replace(needle, replacement)`
pub(crate) fn replace_owned(input: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
  if needle.is_empty() {
    return input.to_vec();
  }
  let mut out = Vec::with_capacity(input.len());
  let mut rest = input;
  while let Some(at) = index_of(rest, needle) {
    out.extend_from_slice(&rest[..at]);
    out.extend_from_slice(replacement);
    rest = &rest[at + needle.len()..];
  }
  out.extend_from_slice(rest);
  out
}

/// `text.split(delimiter)`, with the empty fields. `delimiter` is one byte.
pub(crate) fn split<'a>(
  text: &'a [u8],
  delimiter: &'a [u8],
) -> impl Iterator<Item = &'a [u8]> + Clone {
  let delimiter = delimiter[0];
  text.split(move |&byte| byte == delimiter)
}
