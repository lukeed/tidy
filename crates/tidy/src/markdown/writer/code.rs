use super::Writer;
use crate::markdown::front::Front;
use crate::markdown::{Code, Fence};

/// A code block collected whole before it's written, for the caller to format.
pub(super) enum Block {
  Fenced { info: String, fence: String, text: String },
  Indented,
}

/// The opening fence of a fenced code block written at the start of `text`.
pub(super) fn fence(text: &str) -> String {
  let text = text.trim_start_matches([' ', '\t']);
  let char = text.chars().next().filter(|&char| char == '~').unwrap_or('`');
  let count = text.chars().take_while(|&next| next == char).count().max(3);
  std::iter::repeat_n(char, count).collect()
}

/// The longest run of `char` in `text`.
pub(super) fn longest(text: &str, char: char) -> usize {
  let mut longest = 0;
  let mut run = 0;
  for next in text.chars() {
    run = if next == char { run + 1 } else { 0 };
    longest = longest.max(run);
  }
  longest
}

impl<'s> Writer<'s, '_> {
  pub(super) fn fenced(&mut self, info: &str, original: String, mut text: String) {
    let (lang, meta) = info.trim().split_once(char::is_whitespace).unwrap_or((info.trim(), ""));
    let meta = meta.trim_start();
    let width = self.options.width.saturating_sub(self.indent());
    if !lang.is_empty()
      && let Some(mut formatted) = (self.code)(Code { lang, text: &text, width })
    {
      if !formatted.is_empty() && !formatted.ends_with('\n') {
        formatted.push('\n');
      }
      text = formatted;
    }
    let mut char = match self.options.fence {
      Fence::Keep => original.chars().next().unwrap_or('`'),
      Fence::Backtick => '`',
      Fence::Tilde => '~',
    };
    if char == '`' && info.contains('`') {
      char = '~';
    }
    let longest = longest(&text, char);
    let least = if self.options.fence == Fence::Keep { original.len() } else { 3 };
    let fence: String = std::iter::repeat_n(char, least.max(longest + 1)).collect();
    self.write(&fence);
    if self.options.fence == Fence::Keep {
      self.write(info.trim());
    } else if !lang.is_empty() {
      self.write(lang);
      if !meta.is_empty() {
        self.write(" ");
        self.write(meta);
      }
    }
    self.newline();
    self.lines(&text, true);
    self.finish();
    self.write(&fence);
    self.newline();
  }

  /// Write front matter as written, unless the caller formats it, then end the block.
  pub(super) fn front(&mut self, front: &Front<'_>) {
    let width = self.options.width;
    let formatted = (self.code)(Code { lang: front.lang, text: front.text, width });
    let text = formatted.as_deref().unwrap_or(front.text);
    self.out.push_str(front.open);
    self.out.push('\n');
    self.out.push_str(text);
    if !text.is_empty() && !text.ends_with('\n') {
      self.out.push('\n');
    }
    self.out.push_str(front.close);
    self.out.push('\n');
    self.start = true;
    self.end();
  }

  /// Verbatim lines of code or HTML, each behind the container prefix. Code loses the spaces and
  /// tabs at the end of its lines, as in Prettier.
  pub(super) fn lines(&mut self, text: &str, trim: bool) {
    for line in text.split_inclusive('\n') {
      let body = line.trim_end_matches(['\n', '\r']);
      let ending = &line[body.len()..];
      let body = if trim { body.trim_end_matches([' ', '\t']) } else { body };
      if body.is_empty() && !ending.is_empty() {
        self.blank();
        continue;
      }
      if self.start {
        self.prefix();
      }
      self.out.push_str(body);
      self.out.push_str(ending);
      self.start = !ending.is_empty();
    }
  }
}
