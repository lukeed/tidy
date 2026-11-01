use std::borrow::Cow;

use pulldown_cmark::TagEnd;

use super::{Writer, collapse, escaped};
use crate::markdown::opener::{Fix, interrupts, liquid, starts_html};

impl<'s> Writer<'s, '_> {
  /// Write a soft line break at `at` before the line whose content starts at `next`. A line that
  /// would open a block without its indent joins this one (`opener`). Prettier's parser reads a
  /// Liquid tag alone on a line as a block, which changes three things, kept here without changing
  /// how CommonMark reads the lines: an HTML block start after it keeps the rest of the paragraph
  /// as written, and a lazy Liquid line, with the lazy lines after it, stays at the left margin.
  pub(super) fn soft(&mut self, at: usize, next: usize) {
    if self.held.is_some() && self.containers.is_empty() {
      let start = self.source[..next].rfind('\n').map_or(0, |index| index + 1);
      self.newline();
      return self.write(&self.source[start..next]);
    }
    let line = self.source[next..].split('\n').next().unwrap_or("");
    match interrupts(line).filter(|_| !escaped(self.source, next)) {
      Some(Fix::Keep) if self.containers.is_empty() => {
        let start = self.source[..next].rfind('\n').map_or(0, |index| index + 1);
        self.newline();
        return self.write(&self.source[start..next]);
      }
      Some(Fix::Join | Fix::Keep) => return self.atom(" "),
      Some(Fix::Escape) => {
        self.newline();
        return self.write("\\");
      }
      None => {}
    }
    self.newline();
    if self.liquid_html(at, next) {
      self.raw = Some(next);
      return;
    }
    let margin = !self.containers.is_empty() && self.source[..next].ends_with('\n');
    self.lazy = margin && (self.lazy || liquid(line));
    self.bare = self.lazy;
    self.stray = self.lazy && liquid(line);
  }

  /// `text`, starting at `at` in the source, with each run of spaces collapsed except inside a
  /// Liquid tag, which Prettier keeps as written. A tag can stay open across lines and paragraphs,
  /// so `held` keeps the closer it waits for, but only if that closer comes later.
  pub(super) fn guard(&mut self, text: &'s str, at: usize) -> Cow<'s, str> {
    if self.held.is_none() && !text.contains('{') {
      return collapse(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut offset = 0;
    while offset < text.len() {
      let rest = &text[offset..];
      match self.held {
        Some(close) => match rest.find(close) {
          Some(found) => {
            out.push_str(&rest[..found + 2]);
            offset += found + 2;
            self.held = None;
          }
          None => return Cow::Owned(out + rest),
        },
        None => {
          let next = [("{{", "}}"), ("{%", "%}")]
            .into_iter()
            .filter_map(|(open, close)| rest.find(open).map(|found| (found, close)))
            .min_by_key(|&(found, _)| found);
          let Some((found, close)) = next else { return Cow::Owned(out + &collapse(rest)) };
          out.push_str(&collapse(&rest[..found]));
          out.push_str(&rest[found..found + 2]);
          offset += found + 2;
          if self.closes(close, at + offset) {
            self.held = Some(close);
          }
        }
      }
    }
    Cow::Owned(out)
  }

  /// Whether `close` comes at or after `from`. Queries move forward through the document, so the
  /// next closer found, or that none is left, is kept per closer to search each part only once.
  fn closes(&mut self, close: &'static str, from: usize) -> bool {
    let kind = usize::from(close == "%}");
    match self.closers[kind] {
      Some(found) if found >= from => true,
      None if self.searched[kind] <= from => false,
      _ => {
        let found = memchr::memmem::find(&self.source.as_bytes()[from..], close.as_bytes());
        self.closers[kind] = found.map(|found| from + found);
        self.searched[kind] = from;
        found.is_some()
      }
    }
  }

  /// Whether the line starting at `next` would start an HTML block after the Liquid line that ends
  /// at `at`, as Prettier's parser reads it, so the rest of the paragraph stays as written.
  pub(super) fn liquid_html(&self, at: usize, next: usize) -> bool {
    let line = self.source[next..].split('\n').next().unwrap_or("");
    let quiet = self.marks.is_empty() && self.link.is_none();
    quiet && starts_html(line) && liquid(self.current(at))
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
