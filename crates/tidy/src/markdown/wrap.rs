use std::borrow::Cow;

/// A paragraph collected as words, to be filled into lines.
#[derive(Debug, Default)]
pub(crate) struct Paragraph {
  tokens: Vec<Token>,
  /// The closer of a Liquid tag still open, across text events.
  open: Option<&'static str>,
}

#[derive(Debug)]
enum Token {
  /// A word, and whether its first and last characters are text rather than markup. Prettier only
  /// keeps a space beside Chinese or Japanese text when text is on both sides of it.
  Word {
    text: String,
    head: bool,
    tail: bool,
  },
  Space,
  /// A line break in the source, which joins the lines as Prettier does: a space, or nothing
  /// between Chinese or Japanese text.
  Line,
  /// A hard line break, after its mark (`\` or two spaces) on the word before.
  Hard,
}

impl Paragraph {
  /// Add text from the source: whitespace in it becomes places to break, except inside a Liquid
  /// tag (`{{ … }}` or `{% … %}`), which Prettier's parser keeps whole.
  pub(crate) fn text(&mut self, text: &str) {
    let mut words = text.split([' ', '\t', '\n', '\r']).peekable();
    while let Some(word) = words.next() {
      if !word.is_empty() {
        self.glue(word, true);
      }
      self.open = match self.open {
        Some(close) => match word.find(close) {
          Some(at) => unclosed(&word[at + 2..]),
          None => Some(close),
        },
        None => unclosed(word),
      };
      if words.peek().is_some() {
        if self.open.is_some() {
          self.glue(" ", true);
        } else {
          self.space();
        }
      }
    }
  }

  /// Add something that must not break, like inline HTML or a link. Line breaks inside it stay as
  /// written, as Prettier keeps them, because they can be part of what it means.
  pub(crate) fn atom(&mut self, text: &str) {
    self.glue(text, false);
  }

  pub(crate) fn space(&mut self) {
    if matches!(self.tokens.last(), Some(Token::Word { .. })) {
      self.tokens.push(Token::Space);
    }
  }

  /// A line break in the source, where the lines join.
  pub(crate) fn line(&mut self) {
    if matches!(self.tokens.last(), Some(Token::Word { .. })) {
      self.tokens.push(Token::Line);
    }
  }

  pub(crate) fn hard(&mut self, mark: &str) {
    if matches!(self.tokens.last(), Some(Token::Space | Token::Line)) {
      self.tokens.pop();
    }
    self.glue(mark, false);
    self.tokens.push(Token::Hard);
  }

  /// The last character added, to tell whether emphasis sits inside a word.
  pub(crate) fn last(&self) -> Option<char> {
    match self.tokens.last()? {
      Token::Word { text, .. } => text.chars().next_back(),
      Token::Space | Token::Line | Token::Hard => Some(' '),
    }
  }

  fn glue(&mut self, text: &str, prose: bool) {
    match self.tokens.last_mut() {
      Some(Token::Word { text: word, tail, .. }) => {
        word.push_str(text);
        *tail = prose;
      }
      _ => self.tokens.push(Token::Word { text: text.to_string(), head: prose, tail: prose }),
    }
  }

  /// Fill the words into lines no wider than `width`, or into one line per hard break without it.
  /// The first line is never an HTML tag alone, which would start an HTML block.
  pub(crate) fn fill(&self, width: Option<usize>) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut used = 0;
    let mut space = false;
    for unit in self.units() {
      match unit {
        Unit::Space => space = !line.is_empty(),
        Unit::Hard => {
          lines.push(std::mem::take(&mut line));
          used = 0;
          space = false;
        }
        Unit::Word(word) => {
          let first = crate::width::width(word.split('\n').next().unwrap_or_default());
          let over = width.is_some_and(|width| used + 1 + first > width);
          let tag = lines.is_empty() && crate::opener::starts_html(&line);
          if space && over && !tag {
            lines.push(std::mem::take(&mut line));
            used = 0;
          } else if space {
            line.push(' ');
            used += 1;
          }
          line.push_str(&word);
          used = match word.rsplit_once('\n') {
            Some((_, last)) => crate::width::width(last),
            None => used + first,
          };
          space = false;
        }
      }
    }
    if !line.is_empty() || lines.is_empty() {
      lines.push(line);
    }
    lines
  }

  /// The tokens to fill, with words joined across a space that Prettier never breaks, so they fit
  /// or move as one: a space between text and Chinese or Japanese text, and a space before a word
  /// that would open a block at the start of a line. Beside markup, a space breaks.
  fn units(&self) -> Vec<Unit<'_>> {
    let mut units: Vec<Unit<'_>> = Vec::with_capacity(self.tokens.len());
    let mut tail = false;
    let mut tokens = self.tokens.iter().peekable();
    while let Some(token) = tokens.next() {
      match token {
        Token::Word { text, tail: prose, .. } => {
          units.push(Unit::Word(Cow::Borrowed(text)));
          tail = *prose;
        }
        Token::Hard => units.push(Unit::Hard),
        Token::Space | Token::Line => {
          let next = match tokens.peek() {
            Some(Token::Word { text, head, tail }) => Some((text, *head, *tail)),
            _ => None,
          };
          let glue = match (units.last(), next) {
            (Some(Unit::Word(word)), Some((next, head, _))) => {
              let (before, after) = (word.chars().next_back(), next.chars().next());
              let beside = match token {
                Token::Line if tail && head => join(before, after),
                Token::Space if tail && head => {
                  (before.is_some_and(cj) || after.is_some_and(cj)).then_some(" ")
                }
                _ => None,
              };
              beside.or_else(|| opens_block(next).then_some(" "))
            }
            _ => None,
          };
          match (units.last_mut(), next, glue) {
            (Some(Unit::Word(word)), Some((next, _, prose)), Some(glue)) => {
              let word = word.to_mut();
              word.push_str(glue);
              word.push_str(next);
              tail = prose;
              tokens.next();
            }
            _ => units.push(Unit::Space),
          }
        }
      }
    }
    units
  }
}

/// What a line break between `before` and `after` becomes when lines join: `None` for a space that
/// may break, or the text that glues them. Between two wide characters, other than Korean, it is
/// nothing, as Prettier writes and as CSS renders it. Beside Chinese or Japanese text, it is a space
/// that doesn't break, where Prettier sometimes writes nothing and so changes the rendered text.
fn join(before: Option<char>, after: Option<char>) -> Option<&'static str> {
  if wide(before) && wide(after) {
    return Some("");
  }
  let (Some(before), Some(after)) = (before, after) else { return None };
  (cj(before) || cj(after)).then_some(" ")
}

/// Whether `char` is wide (Chinese, Japanese, or emoji) and not Korean: a line break between two
/// of them renders as nothing in CSS.
pub(crate) fn wide(char: Option<char>) -> bool {
  char.is_some_and(|char| unicode_width::UnicodeWidthChar::width(char) == Some(2) && !korean(char))
}

/// Whether `char` is Korean, which Prettier treats like Latin text.
fn korean(char: char) -> bool {
  matches!(char, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{A960}'..='\u{A97F}' | '\u{AC00}'..='\u{D7FF}')
}

/// A word or group of words to fill, a place to break, or a hard break.
enum Unit<'a> {
  Word(Cow<'a, str>),
  Space,
  Hard,
}

/// The closer that a Liquid tag opened in `word` still waits for.
fn unclosed(word: &str) -> Option<&'static str> {
  let opened = [("{{", "}}"), ("{%", "%}")]
    .into_iter()
    .filter_map(|(open, close)| word.rfind(open).map(|at| (at, close)))
    .max_by_key(|&(at, _)| at);
  let (at, close) = opened?;
  (!word[at + 2..].contains(close)).then_some(close)
}

/// Whether `word` at the start of a line could begin another block and change what the
/// paragraph means, so the line must not break before it.
fn opens_block(word: &str) -> bool {
  crate::opener::interrupts(word).is_some()
}

/// Whether `char` is Chinese or Japanese, which Prettier never breaks a line beside. Korean breaks
/// at spaces like Latin text.
fn cj(char: char) -> bool {
  matches!(
    char,
    '\u{2E80}'..='\u{2FDF}'
      | '\u{3000}'..='\u{303F}'
      | '\u{3040}'..='\u{30FF}'
      | '\u{3100}'..='\u{312F}'
      | '\u{31A0}'..='\u{31FF}'
      | '\u{3200}'..='\u{32FF}'
      | '\u{3400}'..='\u{4DBF}'
      | '\u{4E00}'..='\u{9FFF}'
      | '\u{F900}'..='\u{FAFF}'
      | '\u{FE30}'..='\u{FE4F}'
      | '\u{FF00}'..='\u{FFEF}'
      | '\u{20000}'..='\u{3FFFF}'
  )
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
  fn keeps_chinese_and_japanese_beside_their_spaces() {
    assert_eq!(paragraph(&["aaa 不肖 bbb ccc"]).fill(Some(4)), ["aaa 不肖 bbb", "ccc"]);
    assert_eq!(paragraph(&["aaa 한국 bbb"]).fill(Some(4)), ["aaa", "한국", "bbb"]);
  }

  #[test]
  fn never_starts_a_line_with_a_block_marker() {
    assert_eq!(paragraph(&["aaaa - bbbb 1. cccc"]).fill(Some(4)), ["aaaa -", "bbbb 1.", "cccc"]);
    assert_eq!(paragraph(&["a ## b"]).fill(Some(1)), ["a ##", "b"]);
    assert_eq!(paragraph(&["a #b"]).fill(Some(1)), ["a", "#b"]);
    assert_eq!(
      paragraph(&["aaaa bbbb > 設定 > dddd"]).fill(Some(12)),
      ["aaaa", "bbbb > 設定 >", "dddd"]
    );
  }

  #[test]
  fn keeps_atoms_whole() {
    let mut paragraph = Paragraph::default();
    paragraph.text("a ");
    paragraph.atom("<b\n  c>");
    paragraph.text(" d e");
    assert_eq!(paragraph.fill(Some(5)), ["a <b\n  c>", "d e"]);
  }
}
