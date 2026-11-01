use super::{BOX, Container, Gap, Writer, blank};
use crate::Wrap;

impl<'s> Writer<'s, '_> {
  /// Start a block: emit pending reference definitions before it, then separate it.
  pub(super) fn block(&mut self, at: usize) {
    self.finish();
    if let Some(Container::Item { hang, .. }) = self.containers.last_mut() {
      *hang = false;
    }
    self.flush(at);
    if self.adjacent.as_ref().is_some_and(|adjacent| adjacent.depth == self.containers.len()) {
      self.adjacent = None;
    }
    let loose = matches!(self.containers.last(), Some(Container::Item { loose: true, .. }));
    self.settle(at, loose);
    let liquid = self.liquid.take() == Some(self.containers.len());
    if liquid && self.gap == Gap::Blank && !blank(self.source, self.last.min(at), at) {
      self.gap = Gap::Line;
    }
    self.lazy = false;
    self.separate();
  }

  /// Settle the gap before a block in a list item: a blank line if the item is loose, or if the
  /// source has one.
  pub(super) fn settle(&mut self, at: usize, loose: bool) {
    if self.gap != Gap::Source {
      return;
    }
    let blank = loose || blank(self.source, self.last.min(at), at);
    self.gap = if blank { Gap::Blank } else { Gap::Line };
    if blank && let Some(list) = self.lists.last_mut() {
      list.spread = true;
    }
  }

  pub(super) fn separate(&mut self) {
    if !self.start {
      self.newline();
    }
    if self.gap == Gap::Blank {
      self.blank();
    }
    self.gap = Gap::None;
  }

  /// A block ended: the next one needs a blank line, except inside a list item, where Prettier
  /// keeps blocks together unless the source had a blank line between them.
  pub(super) fn end(&mut self) {
    let item = matches!(self.containers.last(), Some(Container::Item { .. }));
    self.gap = if item { Gap::Source } else { Gap::Blank };
  }

  /// Write out a collected paragraph, and end the line.
  pub(super) fn finish(&mut self) {
    if let Some(paragraph) = self.paragraph.take() {
      let width = match self.options.wrap {
        Wrap::Always => Some(self.options.width.saturating_sub(self.indent())),
        Wrap::Never | Wrap::Preserve => None,
      };
      for line in paragraph.fill(width) {
        self.write(&line);
        self.newline();
      }
    }
    if !self.start {
      self.newline();
    }
  }

  /// Write `text` on the current line, behind the container prefix at its start.
  pub(super) fn write(&mut self, text: &str) {
    if self.start {
      self.prefix();
    }
    self.out.push_str(text);
  }

  pub(super) fn prefix(&mut self) {
    if std::mem::take(&mut self.bare) {
      self.start = false;
      return;
    }
    for container in &mut self.containers {
      match container {
        Container::Quote => self.out.push_str("> "),
        Container::Code => self.out.push_str("    "),
        Container::Item { width, marker, hang, .. } => match marker.take() {
          Some(marker) => self.out.push_str(&marker),
          None => {
            let width = if *hang { *width + BOX } else { *width };
            self.out.extend(std::iter::repeat_n(' ', width));
          }
        },
        Container::Footnote { marker } => match marker.take() {
          Some(marker) => self.out.push_str(&marker),
          None => self.out.push_str("    "),
        },
      }
    }
    self.start = false;
  }

  /// The width of the container prefix.
  pub(super) fn indent(&self) -> usize {
    self
      .containers
      .iter()
      .map(|container| match container {
        Container::Quote => 2,
        Container::Item { width, hang: true, .. } => width + BOX,
        Container::Item { width, .. } => *width,
        Container::Footnote { .. } | Container::Code => 4,
      })
      .sum()
  }

  pub(super) fn newline(&mut self) {
    let trimmed = self.out.trim_end_matches([' ', '\t']).len();
    if !self.out.ends_with("  ") || self.out[trimmed..].len() != 2 {
      self.out.truncate(trimmed);
    }
    self.out.push('\n');
    self.start = true;
  }

  pub(super) fn blank(&mut self) {
    let mark = self.out.len();
    self.prefix();
    self.out.truncate(mark + self.out[mark..].trim_end().len());
    self.out.push('\n');
    self.start = true;
  }
}
