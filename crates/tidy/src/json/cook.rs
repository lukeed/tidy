//! The values of identifiers, strings and template literals, as JavaScript reads their escapes, and `JSON.stringify`.

use std::borrow::Cow;

/// The name of an identifier, with its `\u` escapes read.
pub(crate) fn identifier(raw: &[u8]) -> Cow<'_, str> {
  let text = String::from_utf8_lossy(raw);
  if !raw.contains(&b'\\') {
    return text;
  }
  let units = string(text.as_bytes());
  Cow::Owned(String::from_utf16_lossy(&units))
}

/// The value of a string, from the text between its quotes, as UTF-16 code units: an escape can make a lone
/// surrogate.
pub(crate) fn string(content: &[u8]) -> Vec<u16> {
  cook(content, false)
}

/// The value of a template literal, from the text between its backticks.
pub(crate) fn template(content: &[u8]) -> Vec<u16> {
  cook(content, true)
}

fn cook(content: &[u8], template: bool) -> Vec<u16> {
  let text = String::from_utf8_lossy(content);
  let mut units = Vec::with_capacity(text.len());
  let mut characters = text.char_indices().peekable();
  while let Some((at, character)) = characters.next() {
    if character == '\r' && template {
      if characters.peek().is_some_and(|(_, next)| *next == '\n') {
        characters.next();
      }
      units.push(b'\n' as u16);
      continue;
    }
    if character != '\\' {
      let mut buffer = [0; 2];
      units.extend_from_slice(character.encode_utf16(&mut buffer));
      continue;
    }
    let Some((_, escaped)) = characters.next() else { break };
    let mut unit = |value: u32| match char::from_u32(value) {
      Some(character) => {
        let mut buffer = [0; 2];
        units.extend_from_slice(character.encode_utf16(&mut buffer));
      }
      None => units.push(value as u16),
    };
    match escaped {
      'n' => unit(0x0A),
      'r' => unit(0x0D),
      't' => unit(0x09),
      'b' => unit(0x08),
      'v' => unit(0x0B),
      'f' => unit(0x0C),
      'x' => {
        let digits = &text[at + 2..at + 4];
        unit(u32::from_str_radix(digits, 16).unwrap_or(0));
        characters.nth(1);
      }
      'u' => {
        let rest = &text[at + 2..];
        let (digits, skip) = match rest.strip_prefix('{') {
          Some(braced) => {
            let end = braced.find('}').unwrap_or(braced.len());
            (&braced[..end], end + 2)
          }
          None => (&rest[..4.min(rest.len())], 4),
        };
        unit(u32::from_str_radix(digits, 16).unwrap_or(0));
        for _ in 0..skip {
          characters.next();
        }
      }
      '\r' => {
        if characters.peek().is_some_and(|(_, next)| *next == '\n') {
          characters.next();
        }
      }
      '\n' | '\u{2028}' | '\u{2029}' => {}
      '0'..='7' => {
        let digits =
          text[at + 1..].bytes().take(3).take_while(|byte| matches!(byte, b'0'..=b'7')).count();
        let mut value = u32::from_str_radix(&text[at + 1..at + 1 + digits], 8).unwrap_or(0);
        let mut length = digits;
        if value > 255 {
          length -= 1;
          value = u32::from_str_radix(&text[at + 1..at + 1 + length], 8).unwrap_or(0);
        }
        unit(value);
        for _ in 1..length {
          characters.next();
        }
      }
      other => unit(other as u32),
    }
  }
  units
}

/// `JSON.stringify` of a string, from its UTF-16 code units.
pub(crate) fn stringify(units: &[u16]) -> String {
  let mut out = String::with_capacity(units.len() + 2);
  out.push('"');
  for decoded in char::decode_utf16(units.iter().copied()) {
    match decoded {
      Ok('"') => out.push_str("\\\""),
      Ok('\\') => out.push_str("\\\\"),
      Ok('\u{8}') => out.push_str("\\b"),
      Ok('\u{c}') => out.push_str("\\f"),
      Ok('\n') => out.push_str("\\n"),
      Ok('\r') => out.push_str("\\r"),
      Ok('\t') => out.push_str("\\t"),
      Ok(character) if (character as u32) < 0x20 => {
        out.push_str(&format!("\\u{:04x}", character as u32));
      }
      Ok(character) => out.push(character),
      Err(error) => out.push_str(&format!("\\u{:04x}", error.unpaired_surrogate())),
    }
  }
  out.push('"');
  out
}
