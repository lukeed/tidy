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

/// The length in bytes of the character that `text` ends with, if `\s` matches it, and 0 if not.
pub(crate) fn js_whitespace_len_back(text: &[u8]) -> usize {
  match *text {
    [.., 0x09..=0x0D | b' '] => 1,
    [.., 0xC2, 0xA0] => 2,
    [.., a, b, c] if js_whitespace_len(&[a, b, c]) == 3 => 3,
    _ => 0,
  }
}

/// ECMAScript's `IdentifierStart`: `$`, `_`, and Unicode's `ID_Start`.
pub(crate) fn is_identifier_start(c: u32) -> bool {
  char::from_u32(c).is_some_and(|c| c == '$' || c == '_' || unicode_ident::is_xid_start(c))
}

/// ECMAScript's `IdentifierPart`: `$`, `_`, ZWNJ, ZWJ, and Unicode's `ID_Continue`.
pub(crate) fn is_type_script_identifier_part(c: i32) -> bool {
  u32::try_from(c).ok().and_then(char::from_u32).is_some_and(|c| {
    matches!(c, '$' | '_' | '\u{200C}' | '\u{200D}') || unicode_ident::is_xid_continue(c)
  })
}

/// True if `name` is an identifier.
pub(crate) fn is_identifier(name: &[u8]) -> bool {
  let mut chars = wtf8_codepoints(name);
  chars.next().is_some_and(|(_, c)| is_identifier_start(c))
    && chars.all(|(_, c)| is_type_script_identifier_part(c as i32))
}

/// The length of the name or string literal that `text` starts with: an identifier, a `#name`, a quoted string or a
/// number. 1 for anything else, and 0 for an empty text.
pub(crate) fn name_len(text: &[u8]) -> usize {
  match text.first() {
    None => 0,
    Some(&quote @ (b'"' | b'\'')) => {
      let mut at = 1;
      while let Some(&byte) = text.get(at) {
        at += if byte == b'\\' { 2 } else { 1 };
        if byte == quote {
          break;
        }
      }
      at.min(text.len())
    }
    Some(_) => {
      let start = usize::from(text[0] == b'#');
      let len = std::str::from_utf8(&text[start..])
        .unwrap_or_default()
        .char_indices()
        .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '.' || is_type_script_identifier_part(c as i32)))
        .map_or(text.len() - start, |(at, _)| at);
      (start + len).max(1)
    }
  }
}

/// JavaScript's `Number(text)`.
pub(crate) fn js_string_to_number(text: &[u8]) -> f64 {
  let text = std::str::from_utf8(trim_js_whitespace(text)).unwrap_or("x");
  match text {
    "" => 0.0,
    "Infinity" | "+Infinity" => f64::INFINITY,
    "-Infinity" => f64::NEG_INFINITY,
    _ => text.parse().unwrap_or(f64::NAN),
  }
}

/// `name` with each `\uXXXX` and `\u{X}` replaced by its character: bun's `verify::without_unicode_escapes`.
pub(crate) fn without_unicode_escapes(name: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(name.len());
  let mut rest = name;
  while let Some((&byte, after)) = rest.split_first() {
    rest = after;
    let Some(escape) = after.strip_prefix(b"u").filter(|_| byte == b'\\') else {
      out.push(byte);
      continue;
    };
    let (digits, after) = match escape.strip_prefix(b"{") {
      Some(braced) => {
        let end = index_of_char_usize(braced, b'}').unwrap_or(braced.len());
        (&braced[..end], braced.get(end + 1..).unwrap_or_default())
      }
      None => escape.split_at(escape.len().min(4)),
    };
    let code_point =
      digits.iter().try_fold(0u32, |all, &digit| Some(all.checked_mul(16)? + char::from(digit).to_digit(16)?));
    let c = code_point.and_then(char::from_u32).unwrap_or(char::REPLACEMENT_CHARACTER);
    out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
    rest = after;
  }
  out
}
