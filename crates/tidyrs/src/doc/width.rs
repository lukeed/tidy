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
  let mut width = 0;
  let mut chars = text.chars().map(u32::from).peekable();
  let mut ahead: Vec<u32> = Vec::new();
  while let Some(c) = chars.next() {
    if !may_start_emoji(c) {
      width += match c {
        0..=0x1F | 0x7F..=0x9F | 0x300..=0x36F | 0xFE00..=0xFE0F => 0,
        _ if c >= 0x1100 && within(WIDE, c) => 2,
        _ => 1,
      };
      continue;
    }
    // An emoji is at most a few characters, joined by joiners and selectors: look at those.
    ahead.clear();
    ahead.push(c);
    while let Some(&next) = chars.peek() {
      if ahead.len() >= 64 || (next < 0xA9 && !matches!(next, 0x23 | 0x2A | 0x30..=0x39)) {
        break;
      }
      ahead.push(next);
      chars.next();
    }
    let mut at = 0;
    while at < ahead.len() {
      let c = ahead[at];
      let len = match emoji(&ahead[at..]) {
        2.. if skin(c) => 1,
        len => len,
      };
      if len > 0 {
        width += if len == 1 && within(NARROW_EMOJI, c) { 1 } else { 2 };
        at += len;
        continue;
      }
      at += 1;
      width += match c {
        0..=0x1F | 0x7F..=0x9F | 0x300..=0x36F | 0xFE00..=0xFE0F => 0,
        _ if within(WIDE, c) => 2,
        _ => 1,
      };
    }
  }
  width
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
