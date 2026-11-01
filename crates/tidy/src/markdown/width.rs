use unicode_width::UnicodeWidthStr;

/// How many columns `text` takes. Printable ASCII takes one each, which skips the Unicode tables.
pub(crate) fn width(text: &str) -> usize {
  if text.bytes().all(|byte| (b' '..=b'~').contains(&byte)) { text.len() } else { text.width() }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn measures_columns() {
    assert_eq!(width("abc"), 3);
    assert_eq!(width("日本"), 4);
    assert_eq!(width("a\tb"), "a\tb".width());
  }
}
