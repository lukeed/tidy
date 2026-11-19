//! The ignore directive: a comment that keeps the code after it as written.

/// Whether the trimmed text of a comment is the ignore directive `name`: the name alone, as Prettier's
/// `prettier-ignore`, or the name and the tools it applies to, one of which is `format`, as
/// `starship-ignore format,lint`.
pub(crate) fn is(text: &[u8], name: &[u8]) -> bool {
  let Some(rest) = text.strip_prefix(name) else {
    return false;
  };
  match rest.first() {
    None => true,
    Some(byte) if byte.is_ascii_whitespace() => is_formatting(rest),
    Some(_) => false,
  }
}

/// Whether `tools`, the words after a directive's name, separated by commas or white space, include `format`.
pub(crate) fn is_formatting(tools: &[u8]) -> bool {
  tools.split(|byte| *byte == b',' || byte.is_ascii_whitespace()).any(|word| word == b"format")
}

#[cfg(test)]
mod tests {
  use super::is;

  #[test]
  fn matches_the_name_alone_or_with_format_among_its_tools() {
    for text in ["x-ignore", "x-ignore format", "x-ignore lint,format", "x-ignore lint format"] {
      assert!(is(text.as_bytes(), b"x-ignore"), "{text}");
    }
    for text in ["x-ignore lint", "x-ignore-start", "x-ignored", "x-ignore formats", "y-ignore"] {
      assert!(!is(text.as_bytes(), b"x-ignore"), "{text}");
    }
  }
}
