use std::ops::Range;

use pulldown_cmark::{Event, Tag, TagEnd};

use super::{Events, Writer, escaped};
use crate::markdown::Wrap;

/// Prettier's `[[wiki link]]` syntax in one paragraph: the links, which never break, and whether
/// Prettier keeps all the paragraph's text as written. It does that when a text has `[[` and the
/// same or a later text has `]]`, or a later wiki link, because filling could join them into a link.
pub(super) struct Wiki {
  links: Vec<Range<usize>>,
  pub(super) raw: bool,
  /// A text so far has `[[`.
  opened: bool,
}

impl Wiki {
  fn seen(&mut self, text: &str) {
    self.opened |= text.contains("[[");
    self.raw |= self.opened && text.contains("]]");
  }
}

impl<'s> Writer<'s, '_> {
  /// Read the paragraph at `range` ahead for wiki links, if it has `[[` at all.
  pub(super) fn scan_wiki(&self, events: &mut Events<'s>, range: &Range<usize>) -> Option<Wiki> {
    memchr::memmem::find(&self.source.as_bytes()[range.clone()], b"[[")?;
    let mut wiki = Wiki { links: Vec::new(), raw: false, opened: false };
    let mut run: Option<Range<usize>> = None;
    let mut images = 0;
    let mut depth = 0;
    let mut index = 0;
    while let Some((event, span)) = events.peek(index) {
      index += 1;
      let text = images == 0 && matches!(event, Event::Text(_) | Event::SoftBreak);
      if text {
        let start = run.as_ref().map_or(span.start, |run| run.start);
        run = Some(start..span.end);
        continue;
      }
      if let Some(run) = run.take() {
        self.texts(run, &mut wiki);
      }
      match event {
        Event::Start(Tag::Image { .. }) => images += 1,
        Event::End(TagEnd::Image) => images -= 1,
        Event::Start(_) => depth += 1,
        Event::End(_) if depth == 0 => break,
        Event::End(_) => depth -= 1,
        _ => {}
      }
    }
    Some(wiki)
  }

  /// Split one run of text at its wiki links, as Prettier's parser does, and note what Prettier's
  /// check for a link that filling could create sees.
  fn texts(&self, run: Range<usize>, wiki: &mut Wiki) {
    let mut cursor = run.start;
    while let Some(link) = link(self.source, cursor..run.end) {
      wiki.seen(&self.source[cursor..link.start]);
      wiki.raw |= wiki.opened;
      cursor = link.end;
      wiki.links.push(link);
    }
    wiki.seen(&self.source[cursor..run.end]);
  }

  /// Write `text` at `at` if it is in a paragraph with wiki links: as written when Prettier keeps
  /// the paragraph's text, or with each link whole. Returns whether it was written.
  pub(super) fn wiki_text(&mut self, text: &'s str, at: usize) -> bool {
    let Some(wiki) = &self.wiki else { return false };
    if wiki.raw {
      self.atom(text);
      return true;
    }
    let end = at + text.len();
    let links: Vec<Range<usize>> =
      wiki.links.iter().filter(|link| link.start < end && link.end > at).cloned().collect();
    if links.is_empty() {
      return false;
    }
    let mut cursor = at;
    for link in links {
      let (start, stop) = (link.start.max(at), link.end.min(end));
      if cursor < start {
        self.prose(&self.source[cursor..start], cursor);
      }
      let piece = &self.source[start..stop];
      if self.options.wrap == Wrap::Preserve {
        self.atom(piece);
      } else {
        self.atom(&piece.replace('\t', " "));
      }
      cursor = stop;
    }
    if cursor < end {
      self.prose(&self.source[cursor..end], cursor);
    }
    true
  }
}

/// The first wiki link in `range` of `source`: `[[`, then a target with something other than spaces
/// and no line break or `]`, then `]]`.
fn link(source: &str, range: Range<usize>) -> Option<Range<usize>> {
  let mut from = range.start;
  while let Some(found) = source[from..range.end].find("[[") {
    let start = from + found;
    from = start + 1;
    if escaped(source, start) {
      continue;
    }
    let target = &source[start + 2..range.end];
    let close = target.find([']', '\n', '\r'])?;
    let filled = target[..close].bytes().any(|byte| !matches!(byte, b' ' | b'\t'));
    if filled && target[close..].starts_with("]]") {
      return Some(start..start + 2 + close + 2);
    }
  }
  None
}
