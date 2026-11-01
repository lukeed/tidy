use pulldown_cmark::Event;

use super::{Events, Writer};
use crate::{Bullet, Numbering};

pub(super) struct List {
  /// The current item has a blank line between its blocks, so a blank line follows it.
  pub(super) spread: bool,
  pub(super) items: usize,
  pub(super) ordered: bool,
  /// How many lists of the same kind directly precede this one, which alternates its marker.
  pub(super) nth: usize,
  pub(super) start: u64,
  /// Every item after the first repeats one number, as in `1.`, `1.`, `1.`.
  pub(super) repeats: bool,
  /// Prettier pads the markers of an ordered list whose content sits on tab stops.
  pub(super) aligned: bool,
  /// The content indent of the last item. A block after the list indented this far would join it.
  pub(super) width: usize,
  /// The width every marker needs, so an indented code block after the list stays out of it.
  pub(super) least: usize,
  /// The list stays loose without the blank lines Prettier drops before a nested list.
  pub(super) firm: bool,
}

/// The list that ended last, while nothing else followed it at the same depth.
pub(super) struct Adjacent {
  pub(super) depth: usize,
  pub(super) ordered: bool,
  pub(super) nth: usize,
  pub(super) width: usize,
}

/// The largest number an ordered list marker can have: CommonMark allows nine digits.
const LARGEST: u64 = 999_999_999;

/// The tab width that Prettier aligns ordered list content to.
pub(super) const TAB: usize = 2;

/// An ordered list item as written.
pub(super) struct Item {
  pub(super) number: u64,
  /// Spaces between the marker and the content.
  pub(super) spaces: usize,
  /// The column the content starts at, unless the item starts empty.
  pub(super) column: Option<usize>,
}

/// Prettier's `isAligned`: whether an ordered list keeps its content on tab stops.
pub(super) fn aligned(items: &[Item]) -> bool {
  let Some(first) = items.first() else { return false };
  if first.spaces > 1 {
    return true;
  }
  let Some(column) = first.column else { return false };
  match items.get(1) {
    None => column.is_multiple_of(TAB),
    Some(second) if second.column != Some(column) => false,
    Some(_) if column.is_multiple_of(TAB) => true,
    Some(second) => second.spaces > 1,
  }
}

impl<'s> Writer<'s, '_> {
  /// Read ahead through the ordered list that just started, for the first items' numbers.
  pub(super) fn scan(&self, events: &mut Events<'s>, ordered: bool) -> Vec<Item> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut index = 0;
    while ordered && items.len() < 3 {
      let Some((event, range)) = events.peek(index) else { break };
      index += 1;
      match event {
        Event::Start(_) => {
          if depth == 0 {
            items.push(self.item(range.start));
          }
          depth += 1;
        }
        Event::End(_) if depth == 0 => break,
        Event::End(_) => depth -= 1,
        _ => {}
      }
    }
    items
  }

  /// The number of the ordered item whose marker is at `at`, and where its content starts.
  pub(super) fn item(&self, at: usize) -> Item {
    let line = self.source[..at].rfind('\n').map_or(0, |index| index + 1);
    let rest = &self.source[at..];
    let indent = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let rest = &rest[indent..];
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let after = rest.get(digits + 1..).unwrap_or("");
    let spaces = after.bytes().take_while(|&byte| byte == b' ').count();
    let empty = after[spaces..].starts_with(['\n', '\r']) || after[spaces..].is_empty();
    Item {
      number: rest[..digits].parse().unwrap_or(0),
      spaces,
      column: (!empty).then_some(at - line + indent + digits + 1 + spaces),
    }
  }

  pub(super) fn marker(&self, at: usize, index: usize) -> String {
    let list = self.lists.last().expect("an item is inside a list");
    let written = self.source[at..].trim_start();
    if list.ordered {
      let digits = written.bytes().take_while(u8::is_ascii_digit).count();
      if self.options.numbering == Numbering::Keep {
        return written[..digits + 1].to_string();
      }
      let number = match index {
        0 => list.start,
        _ if list.repeats => 1,
        _ => (list.start + index as u64).min(LARGEST),
      };
      let delimiter = if list.nth.is_multiple_of(2) { '.' } else { ')' };
      return format!("{number}{delimiter}");
    }
    let (even, odd) = match self.options.bullet {
      Bullet::Keep => return written[..1].to_string(),
      Bullet::Dash => ("-", "*"),
      Bullet::Asterisk => ("*", "-"),
      Bullet::Plus => ("+", "-"),
    };
    (if list.nth.is_multiple_of(2) { even } else { odd }).to_string()
  }
}

/// Widen `marker` to `least`, as Prettier does: up to four spaces after it, then up to three before.
pub(super) fn widen(marker: &mut String, least: usize) {
  if marker.len() >= least {
    return;
  }
  marker.truncate(marker.trim_end().len());
  let after = (least - marker.len()).min(4);
  marker.extend(std::iter::repeat_n(' ', after));
  let before = least.saturating_sub(marker.len()).min(3);
  marker.insert_str(0, &" ".repeat(before));
}
