use unicode_width::UnicodeWidthStr;

/// A paragraph collected as words, to be filled into lines.
#[derive(Debug, Default)]
pub(crate) struct Paragraph {
  tokens: Vec<Token>,
}

#[derive(Debug)]
enum Token {
  Word(String),
  Space,
  /// A hard line break, after its mark (`\` or two spaces) on the word before.
  Hard,
}

impl Paragraph {
  /// Add text from the source: whitespace in it becomes places to break.
  pub(crate) fn text(&mut self, text: &str) {
    let mut words = text.split([' ', '\t', '\n', '\r']).peekable();
    while let Some(word) = words.next() {
      if !word.is_empty() {
        self.glue(word);
      }
      if words.peek().is_some() {
        self.space();
      }
    }
  }

  /// Add something that must not break, like a code span or a link. A line ending inside it is
  /// the same as a space, so it becomes one.
  pub(crate) fn atom(&mut self, text: &str) {
    let mut lines = text.split('\n');
    if let Some(first) = lines.next() {
      self.glue(first.trim_end_matches('\r'));
    }
    for line in lines {
      self.glue(" ");
      self.glue(line.trim_start_matches([' ', '\t']).trim_end_matches('\r'));
    }
  }

  pub(crate) fn space(&mut self) {
    if matches!(self.tokens.last(), Some(Token::Word(_))) {
      self.tokens.push(Token::Space);
    }
  }

  pub(crate) fn hard(&mut self, mark: &str) {
    if matches!(self.tokens.last(), Some(Token::Space)) {
      self.tokens.pop();
    }
    self.glue(mark);
    self.tokens.push(Token::Hard);
  }

  /// The last character added, to tell whether emphasis sits inside a word.
  pub(crate) fn last(&self) -> Option<char> {
    match self.tokens.last()? {
      Token::Word(word) => word.chars().next_back(),
      Token::Space | Token::Hard => Some(' '),
    }
  }

  fn glue(&mut self, text: &str) {
    match self.tokens.last_mut() {
      Some(Token::Word(word)) => word.push_str(text),
      _ => self.tokens.push(Token::Word(text.to_string())),
    }
  }

  /// Fill the words into lines no wider than `width`, or into one line per hard break without it.
  pub(crate) fn fill(&self, width: Option<usize>) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut used = 0;
    let mut space = false;
    for token in &self.tokens {
      match token {
        Token::Space => space = !line.is_empty(),
        Token::Hard => {
          lines.push(std::mem::take(&mut line));
          used = 0;
          space = false;
        }
        Token::Word(word) => {
          let size = word.width();
          let over = width.is_some_and(|width| used + 1 + size > width);
          if space && over && !opens_block(word) {
            lines.push(std::mem::take(&mut line));
            used = 0;
          } else if space {
            line.push(' ');
            used += 1;
          }
          line.push_str(word);
          used += size;
          space = false;
        }
      }
    }
    if !line.is_empty() || lines.is_empty() {
      lines.push(line);
    }
    lines
  }
}

/// Whether `word` at the start of a line could begin another block and change what the
/// paragraph means, so the line must not break before it.
fn opens_block(word: &str) -> bool {
  let bytes = word.as_bytes();
  let Some(&first) = bytes.first() else { return false };
  let run = bytes.iter().take_while(|&&byte| byte == first).count();
  match first {
    b'>' | b'<' => true,
    b'#' => run <= 6 && run == bytes.len(),
    b'-' | b'+' | b'*' if bytes.len() == 1 => true,
    b'-' | b'*' | b'_' | b'=' => run == bytes.len() && (run >= 3 || matches!(first, b'-' | b'=')),
    b'`' | b'~' => run >= 3,
    byte if byte.is_ascii_digit() => {
      let digits = bytes.iter().take_while(|byte| byte.is_ascii_digit()).count();
      digits + 1 == bytes.len() && matches!(bytes[digits], b'.' | b')')
    }
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn paragraph(parts: &[&str]) -> Paragraph {
    let mut paragraph = Paragraph::default();
    for part in parts {
      match *part {
        "\n" => paragraph.space(),
        "\\" => paragraph.hard("\\"),
        text => paragraph.text(text),
      }
    }
    paragraph
  }

  #[test]
  fn fills_to_the_width() {
    let paragraph = paragraph(&["one two three", "\n", "four five"]);
    assert_eq!(paragraph.fill(Some(9)), ["one two", "three", "four five"]);
    assert_eq!(paragraph.fill(None), ["one two three four five"]);
  }

  #[test]
  fn keeps_hard_breaks() {
    let paragraph = paragraph(&["one", "\\", "two"]);
    assert_eq!(paragraph.fill(None), ["one\\", "two"]);
  }

  #[test]
  fn never_starts_a_line_with_a_block_marker() {
    assert_eq!(paragraph(&["aaaa - bbbb 1. cccc"]).fill(Some(4)), ["aaaa -", "bbbb 1.", "cccc"]);
    assert_eq!(paragraph(&["a ## b"]).fill(Some(1)), ["a ##", "b"]);
    assert_eq!(paragraph(&["a #b"]).fill(Some(1)), ["a", "#b"]);
  }

  #[test]
  fn keeps_atoms_whole() {
    let mut paragraph = Paragraph::default();
    paragraph.text("a ");
    paragraph.atom("`b\n  c`");
    assert_eq!(paragraph.fill(Some(1)), ["a", "`b c`"]);
  }
}
