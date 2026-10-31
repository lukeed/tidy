//! Prettier's `getStringWidth`: how many columns a text takes. Ported from bun's port (MIT).

use super::width_tables::{EMOJI, EMOJI_MODIFIER_BASE, NARROW_EMOJI, WIDE};

const VARIATION_SELECTOR_16: u32 = 0xFE0F;
const ZERO_WIDTH_JOINER: u32 = 0x200D;
const COMBINING_ENCLOSING_KEYCAP: u32 = 0x20E3;

/// The width of `text`, which has no line break.
#[inline]
pub fn width(text: &str) -> usize {
  let bytes = text.as_bytes();
  if bytes.iter().all(|&byte| byte.wrapping_sub(0x20) < 0x60) {
    return bytes.len();
  }
  wide(text)
}

#[cold]
fn wide(text: &str) -> usize {
  let bytes = text.as_bytes();
  let special = special();
  let mut width = 0;
  let mut at = 0;
  let mut ahead: Vec<u32> = Vec::new();
  while at < bytes.len() {
    // A run of printable ASCII, where a keycap's base counts unless a non-ASCII mark follows it.
    let run = bytes[at..]
      .iter()
      .enumerate()
      .take_while(|&(index, &byte)| {
        (0x20..0x7F).contains(&byte)
          && (!matches!(byte, b'#' | b'*' | b'0'..=b'9')
            || bytes.get(at + index + 1).is_none_or(u8::is_ascii))
      })
      .count();
    width += run;
    at += run;
    let Some(c) = text[at..].chars().next() else { break };
    let code = u32::from(c);
    if code < 0x10000 && special[code as usize / 64] >> (code % 64) & 1 == 0 {
      width += 1;
      at += c.len_utf8();
      continue;
    }
    if !may_start_emoji(code) {
      width += match code {
        0..=0x1F | 0x7F..=0x9F | 0x300..=0x36F | 0xFE00..=0xFE0F => 0,
        _ if code >= 0x1100 && within(WIDE, code) => 2,
        _ => 1,
      };
      at += c.len_utf8();
      continue;
    }
    // An emoji is at most a few characters, joined by joiners and selectors: look at those.
    ahead.clear();
    let mut end = at;
    for next in text[at..].chars() {
      let next_code = u32::from(next);
      if !ahead.is_empty()
        && (ahead.len() >= 64
          || (next_code < 0xA9 && !matches!(next_code, 0x23 | 0x2A | 0x30..=0x39)))
      {
        break;
      }
      ahead.push(next_code);
      end += next.len_utf8();
    }
    at = end;
    let mut index = 0;
    while index < ahead.len() {
      let c = ahead[index];
      let len = match emoji(&ahead[index..]) {
        2.. if skin(c) => 1,
        len => len,
      };
      if len > 0 {
        width += if len == 1 && within(NARROW_EMOJI, c) { 1 } else { 2 };
        index += len;
        continue;
      }
      index += 1;
      width += match c {
        0..=0x1F | 0x7F..=0x9F | 0x300..=0x36F | 0xFE00..=0xFE0F => 0,
        _ if within(WIDE, c) => 2,
        _ => 1,
      };
    }
  }
  width
}

/// A bit for each character of the Basic Multilingual Plane whose width isn't simply 1: control, combining, and wide
/// characters, variation selectors, and what can start an emoji.
fn special() -> &'static [u64; 1024] {
  static SPECIAL: std::sync::OnceLock<Box<[u64; 1024]>> = std::sync::OnceLock::new();
  SPECIAL.get_or_init(|| {
    let mut bits = Box::new([0u64; 1024]);
    let mut mark = |start: u32, end: u32| {
      for c in start..=end.min(0xFFFF) {
        bits[c as usize / 64] |= 1 << (c % 64);
      }
    };
    for (start, end) in [
      (0, 0x1F),
      (0x23, 0x23),
      (0x2A, 0x2A),
      (0x30, 0x39),
      (0x7F, 0x9F),
      (0x300, 0x36F),
      (0xFE00, 0xFE0F),
    ] {
      mark(start, end);
    }
    for &(start, end) in WIDE.iter().chain(EMOJI) {
      if start <= 0xFFFF {
        mark(start, end);
      }
    }
    bits
  })
}

/// Whether `c` can be the first character of an emoji: a keycap's base, a regional indicator, or an emoji.
fn may_start_emoji(c: u32) -> bool {
  matches!(c, 0x23 | 0x2A | 0x30..=0x39) || (c >= 0xA9 && (regional(c) || within(EMOJI, c)))
}

fn within(table: &[(u32, u32)], c: u32) -> bool {
  let after = table.partition_point(|range| range.0 <= c);
  after > 0 && c <= table[after - 1].1
}

fn skin(c: u32) -> bool {
  (0x1F3FB..=0x1F3FF).contains(&c)
}

fn regional(c: u32) -> bool {
  (0x1F1E6..=0x1F1FF).contains(&c)
}

/// How many characters at the start of `chars` form one emoji, as `emoji-regex` sees it.
fn emoji(chars: &[u32]) -> usize {
  let at = |i: usize| chars.get(i).copied().unwrap_or(0);
  let first = at(0);
  if matches!(first, 0x23 | 0x2A | 0x30..=0x39) {
    let mark = if at(1) == VARIATION_SELECTOR_16 { 2 } else { 1 };
    return if at(mark) == COMBINING_ENCLOSING_KEYCAP { mark + 1 } else { 0 };
  }
  if regional(first) {
    return if regional(at(1)) { 2 } else { 0 };
  }
  if !within(EMOJI, first) {
    return 0;
  }
  let mut len = 0;
  loop {
    let base = at(len);
    len += 1;
    if at(len) == VARIATION_SELECTOR_16 || (skin(at(len)) && within(EMOJI_MODIFIER_BASE, base)) {
      len += 1;
    }
    if base == 0x1F3F4 && (0xE0020..=0xE007E).contains(&at(len)) {
      while (0xE0020..=0xE007E).contains(&at(len)) {
        len += 1;
      }
      if at(len) == 0xE007F {
        len += 1;
      }
    }
    if at(len) == ZERO_WIDTH_JOINER && within(EMOJI, at(len + 1)) {
      len += 1;
    } else {
      return len;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::width;

  #[test]
  fn measures_columns() {
    assert_eq!(width("hello"), 5);
    assert_eq!(width("\u{7f}"), 1);
    assert_eq!(width("日本"), 4);
    assert_eq!(width("e\u{301}"), 1);
    assert_eq!(width("👍🏽"), 2);
    assert_eq!(width("👨‍👩‍👧"), 2);
    assert_eq!(width("\u{1}"), 0);
  }
}
