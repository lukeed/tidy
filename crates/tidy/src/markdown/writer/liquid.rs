use pulldown_cmark::TagEnd;

use super::{Writer, escaped};
use crate::opener::{Fix, interrupts, liquid, starts_html};

impl<'s> Writer<'s, '_> {
  /// Write a soft line break at `at` before the line whose content starts at `next`. A line that
  /// would open a block without its indent joins this one (`opener`). Prettier's parser reads a
  /// Liquid tag alone on a line as a block, which changes three things, kept here without changing
  /// how CommonMark reads the lines: an HTML block start after it keeps the rest of the paragraph
  /// as written, and a lazy Liquid line, with the lazy lines after it, stays at the left margin.
  pub(super) fn soft(&mut self, at: usize, next: usize) {
    let line = self.source[next..].split('\n').next().unwrap_or("");
    match interrupts(line).filter(|_| !escaped(self.source, next)) {
      Some(Fix::Join) => return self.atom(" "),
      Some(Fix::Escape) => {
        self.newline();
        return self.write("\\");
      }
      None => {}
    }
    self.newline();
    let quiet = self.marks.is_empty() && self.link.is_none();
    if quiet && starts_html(line) && liquid(self.current(at)) {
      self.raw = Some(next);
      return;
    }
    let margin = !self.containers.is_empty() && self.source[..next].ends_with('\n');
    self.lazy = margin && (self.lazy || liquid(line));
    self.bare = self.lazy;
  }

  /// Whether the soft break at `at`, before the line starting at `next`, touches a Liquid line.
  /// Filling must keep it, since Prettier keeps such a line as a block of its own.
  pub(super) fn liquid_break(&self, at: usize, next: usize) -> bool {
    liquid(self.current(at)) || liquid(self.source[next..].split('\n').next().unwrap_or(""))
  }

  /// The line that ends at `at`, without its container prefix.
  fn current(&self, at: usize) -> &'s str {
    let start = self.source[..at].rfind('\n').map_or(0, |index| index + 1);
    self.source[start..at].trim_start_matches([' ', '\t', '>'])
  }

  /// Whether the paragraph ending at `end` ends with a Liquid line, which Prettier keeps beside the
  /// next block.
  pub(super) fn ends_liquid(&self, end: usize) -> bool {
    let text = self.source[..end].trim_end();
    let line = text.rsplit_once('\n').map_or(text, |(_, line)| line);
    liquid(line.trim_start_matches([' ', '\t', '>']))
  }

  /// Whether the paragraph starting at `at` starts with a Liquid line.
  pub(super) fn starts_liquid(&self, at: usize) -> bool {
    liquid(self.source[at..].split('\n').next().unwrap_or(""))
  }
}

/// Whether `tag` ends a block, which ends a stretch of the paragraph copied as written.
pub(super) fn ends_block(tag: &TagEnd) -> bool {
  !matches!(
    tag,
    TagEnd::Emphasis
      | TagEnd::Strong
      | TagEnd::Strikethrough
      | TagEnd::Link
      | TagEnd::Image
      | TagEnd::Superscript
      | TagEnd::Subscript
  )
}
