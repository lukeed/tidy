use pulldown_cmark::{Event, Tag};

use super::{Events, Writer};
use crate::markdown::Wrap;
use crate::markdown::width::width;

/// Where a footnote definition's text starts, as Prettier decides.
pub(super) enum Start {
  /// On the next line, indented.
  Below,
  /// Beside the marker, with the lines after it indented.
  Beside,
  /// Beside the marker, with the lines after it at the margin: one paragraph with `Wrap::Never`,
  /// which Prettier prints without the indent.
  Flush,
}

impl<'s> Writer<'s, '_> {
  /// Where the footnote definition that just started puts its text. One paragraph stays beside its
  /// `marker` when it is on one line, or always with `Wrap::Never`. Otherwise the first block stays
  /// only if it prints as one line that fits.
  pub(super) fn start(&self, events: &mut Events<'s>, marker: &str) -> Start {
    let preserve = self.options.wrap == Wrap::Preserve;
    let mut depth = 0usize;
    let mut index = 0;
    let mut children = 0;
    let mut paragraph = false;
    let mut first = 0..0;
    let mut lines = false;
    let mut leaves = 0;
    while let Some((event, range)) = events.peek(index) {
      index += 1;
      let opening = depth == 0 && matches!(event, Event::Start(_) | Event::Rule);
      if opening {
        children += 1;
        if children == 1 {
          paragraph = matches!(event, Event::Start(Tag::Paragraph));
          first = range.clone();
        }
      }
      let inside = children == 1 && (depth > 0 || opening);
      match event {
        Event::Start(tag) => {
          if inside {
            leaves += usize::from(leaf(tag));
            lines |= matches!(tag, Tag::CodeBlock(_) | Tag::Table(_));
          }
          depth += 1;
        }
        Event::End(_) if depth == 0 => break,
        Event::End(_) => depth -= 1,
        Event::HardBreak if inside => lines = true,
        Event::SoftBreak if inside => lines |= preserve,
        Event::Html(html) if inside => lines |= html.trim_end().contains('\n'),
        Event::Rule if inside => leaves += 1,
        _ => {}
      }
    }
    if children == 0 {
      return Start::Beside;
    }
    let single = children == 1 && paragraph;
    if single && self.options.wrap == Wrap::Never {
      return Start::Flush;
    }
    if single && preserve && !lines {
      return Start::Beside;
    }
    let flat: usize = self.source[first].split_whitespace().map(|word| width(word) + 1).sum();
    let room = self.options.width.saturating_sub(self.indent() + marker.len());
    if lines || leaves > 1 || flat.saturating_sub(1) > room { Start::Below } else { Start::Beside }
  }
}

/// Whether `tag` starts a block that prints on lines of its own: one that holds no other blocks,
/// or a list item, whose content may be bare text.
fn leaf(tag: &Tag<'_>) -> bool {
  matches!(
    tag,
    Tag::Paragraph
      | Tag::Heading { .. }
      | Tag::CodeBlock(_)
      | Tag::HtmlBlock
      | Tag::Table(_)
      | Tag::Item
  )
}
