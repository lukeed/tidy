//! Text helpers of the JS printer: tidy's `crate::text`, which is bun's `src/format/text.rs` (MIT), and the few that only
//! the printer uses.

pub(crate) use crate::text::*;

use crate::js::strings;

pub(crate) const BOM: &[u8] = b"\xEF\xBB\xBF";

/// `text.toLowerCase().endsWith(suffix)` for an ASCII `suffix`.
pub(crate) fn ends_with_ignore_ascii_case(text: &[u8], suffix: &[u8]) -> bool {
    text.len() >= suffix.len() && text[text.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

/// `/^\s*#[^\S\n]*@(?:a|b)\s*?(?:\n|$)/`: YAML's and GraphQL's `hasPragma` and `hasIgnorePragma`.
pub(crate) fn has_pragma_in_hash_comment(text: &[u8], pragmas: [&[u8]; 2]) -> bool {
    fn trim_inline_whitespace_start(mut text: &[u8]) -> &[u8] {
        while !text.starts_with(b"\n")
            && let len @ 1.. = strings::js_whitespace_len(text)
        {
            text = &text[len..];
        }
        text
    }
    let Some(comment) = strings::trim_js_whitespace_start(text).strip_prefix(b"#") else {
        return false;
    };
    let Some(name) = trim_inline_whitespace_start(comment).strip_prefix(b"@") else {
        return false;
    };
    pragmas.iter().any(|pragma| {
        name.strip_prefix(*pragma)
            .is_some_and(|rest| matches!(trim_inline_whitespace_start(rest), [] | [b'\n', ..]))
    })
}

/// The check in `printLeadingComment`: true if the line that ends at `at`, ignoring spaces and
/// tabs, is followed by an empty line.
#[inline]
pub(crate) fn is_followed_by_empty_line(text: &[u8], at: usize) -> bool {
    has_newline(text, skip_newline(text, skip_spaces(text, at)))
}

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
