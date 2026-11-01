use std::collections::VecDeque;
use std::ops::Range;

use pulldown_cmark::{Event, OffsetIter, Tag};

/// The parser's events with lookahead.
pub(super) struct Events<'s> {
  iter: OffsetIter<'s>,
  ahead: VecDeque<(Event<'s>, Range<usize>)>,
}

impl<'s> Events<'s> {
  pub(super) fn new(iter: OffsetIter<'s>) -> Self {
    Self { iter, ahead: VecDeque::new() }
  }

  pub(super) fn next(&mut self) -> Option<(Event<'s>, Range<usize>)> {
    self.ahead.pop_front().or_else(|| self.iter.next())
  }

  pub(super) fn peek(&mut self, index: usize) -> Option<&(Event<'s>, Range<usize>)> {
    while self.ahead.len() <= index {
      let event = self.iter.next()?;
      self.ahead.push_back(event);
    }
    self.ahead.get(index)
  }
}

/// Whether a backslash escapes the character at `at`: an odd run of them comes right before it.
pub(super) fn escaped(source: &str, at: usize) -> bool {
  let run = source.as_bytes()[..at].iter().rev().take_while(|&&byte| byte == b'\\').count();
  run % 2 == 1
}

/// Whether `tag` opens a block, as opposed to inline content.
pub(super) fn is_block(tag: &Tag<'_>) -> bool {
  matches!(
    tag,
    Tag::Paragraph
      | Tag::Heading { .. }
      | Tag::BlockQuote(_)
      | Tag::CodeBlock(_)
      | Tag::HtmlBlock
      | Tag::List(_)
      | Tag::Table(_)
      | Tag::FootnoteDefinition(_)
  )
}

/// Whether a blank line, holding nothing but whitespace and block quote markers, separates the
/// block that ended at `last` from the one starting at `at`.
pub(super) fn blank(source: &str, last: usize, at: usize) -> bool {
  let end = source[..last].trim_end_matches(['\n', '\r', ' ', '\t', '>']).len();
  let from = source[end..].find('\n').map_or(source.len(), |index| end + index + 1);
  let to = source[..at].rfind('\n').map_or(0, |index| index + 1);
  from < to
    && source[from..to].lines().any(|line| line.trim_matches([' ', '\t', '\r', '>']).is_empty())
}
