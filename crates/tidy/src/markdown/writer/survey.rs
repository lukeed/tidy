use std::collections::VecDeque;

use pulldown_cmark::{CodeBlockKind, Event, Tag};

use super::{Events, Writer, blank, is_block};

/// What the lists ahead must know before their first line is written, found in one pass over a
/// top-level list, so that nested lists and items don't read ahead again: each list's marker
/// width and whether it stays loose, and whether each item is loose. They are kept in document
/// order by where they start.
#[derive(Default)]
pub(super) struct Survey {
  lists: VecDeque<(usize, (usize, bool))>,
  items: VecDeque<(usize, bool)>,
}

impl Survey {
  /// The marker width of the list starting at `at`, and whether it stays loose without the blank
  /// lines Prettier drops before a nested list, if surveyed.
  pub(super) fn list(&mut self, at: usize) -> Option<(usize, bool)> {
    take(&mut self.lists, at)
  }

  /// Whether the item starting at `at` is loose, if surveyed.
  pub(super) fn item(&mut self, at: usize) -> Option<bool> {
    take(&mut self.items, at)
  }
}

/// The value for `at`, dropping values for anything before it that was never asked for, like a
/// list kept as written by an ignore directive.
fn take<T: Copy>(queue: &mut VecDeque<(usize, T)>, at: usize) -> Option<T> {
  while let Some(&(start, value)) = queue.front() {
    if start > at {
      return None;
    }
    queue.pop_front();
    if start == at {
      return Some(value);
    }
  }
  None
}

/// An item being surveyed: Prettier's `isLooseListItem` looks for a blank line between its direct
/// children, or between it and the next item.
struct Item {
  slot: usize,
  base: usize,
  last: Option<usize>,
  inline: bool,
  /// The last direct child is a paragraph or code, after which Prettier keeps a blank line before a
  /// nested list, or HTML, which only a blank line ends.
  prose: bool,
  /// Loose even without the blank lines Prettier drops.
  firm: bool,
}

impl<'s> Writer<'s, '_> {
  /// Survey the list that just started at `start`, reading ahead to its end once.
  pub(super) fn survey(&self, events: &mut Events<'s>, start: usize) -> Survey {
    let mut lists = vec![(start, (0, false))];
    let mut items: Vec<(usize, bool)> = Vec::new();
    let mut open_lists = vec![(0usize, 0usize)];
    let mut open_items: Vec<Item> = Vec::new();
    let mut depth = 1;
    let mut index = 0;
    let gap = |last: Option<usize>, at: usize| last.is_some_and(|end| blank(self.source, end, at));
    while let Some((event, range)) = events.peek(index) {
      let (start, end) = (range.start, range.end);
      index += 1;
      let direct = open_items.last().is_some_and(|item| depth == item.base + 1);
      match event {
        Event::Start(tag) => {
          let block = is_block(tag);
          if direct && let Some(item) = open_items.last_mut() {
            if (block || !item.inline) && gap(item.last, start) {
              items[item.slot].1 = true;
              item.firm |= !matches!(tag, Tag::List(_)) || item.prose;
            }
            if block || !item.inline {
              item.inline = !block;
              item.prose =
                !block || matches!(tag, Tag::Paragraph | Tag::CodeBlock(_) | Tag::HtmlBlock);
            }
          }
          match tag {
            Tag::List(_) => {
              open_lists.push((lists.len(), depth));
              lists.push((start, (0, false)));
            }
            Tag::Item => {
              let slot = items.len();
              let item =
                Item { slot, base: depth, last: None, inline: false, prose: false, firm: false };
              open_items.push(item);
              items.push((start, false));
            }
            _ => {}
          }
          depth += 1;
        }
        Event::End(_) => {
          depth -= 1;
          if open_items.last().is_some_and(|item| item.base == depth) {
            let item = open_items.pop().expect("an item is open");
            let next = match events.peek(index) {
              Some((Event::Start(Tag::Item), next)) => Some(next.start),
              _ => None,
            };
            let apart = next.is_some_and(|next| gap(item.last, next));
            if apart {
              items[item.slot].1 = true;
            }
            if let Some(&(list, _)) = open_lists.last() {
              let spaced = items[item.slot].1 && next.is_some();
              lists[list].1.1 |= apart || item.firm || spaced;
            }
          } else if open_lists.last().is_some_and(|&(_, base)| base == depth) {
            let (slot, _) = open_lists.pop().expect("a list is open");
            lists[slot].1.0 = least(events, index);
            if open_lists.is_empty() {
              break;
            }
          }
          if let Some(item) = open_items.last_mut()
            && depth == item.base + 1
          {
            item.last = Some(end);
          }
        }
        _ => {
          if direct && let Some(item) = open_items.last_mut() {
            if !item.inline {
              if gap(item.last, start) {
                items[item.slot].1 = true;
                item.firm = true;
              }
              item.inline = true;
              item.prose = true;
            }
            item.last = Some(end);
          }
        }
      }
    }
    Survey { lists: lists.into(), items: items.into() }
  }
}

/// The marker width Prettier gives a list when the event at `index`, right after it, starts an
/// indented code block: wider than the code's indent, so the code doesn't join the last item.
fn least(events: &mut Events<'_>, index: usize) -> usize {
  if !matches!(events.peek(index), Some((Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)), _)))
  {
    return 0;
  }
  let indent = match events.peek(index + 1) {
    Some((Event::Text(text), _)) => text
      .chars()
      .take_while(|char| matches!(char, ' ' | '\t'))
      .map(|char| if char == '\t' { 4 } else { 1 })
      .sum(),
    _ => 0,
  };
  4 + indent + 1
}
