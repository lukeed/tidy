//! Stable sorts: bun's `src/format/sort.rs` (MIT). [`sort_by`] accepts any comparison, where the standard library's
//! may panic if the comparison isn't a total order.

use std::cmp::Ordering;

pub(crate) fn sort<T: Ord>(items: &mut [T]) {
  items.sort();
}

pub(crate) fn sort_by_key<T, K: Ord>(items: &mut [T], key: impl FnMut(&T) -> K) {
  items.sort_by_key(key);
}

/// A merge sort, which is stable for any comparison.
pub(crate) fn sort_by<T: Clone>(items: &mut [T], mut compare: impl FnMut(&T, &T) -> Ordering) {
  let mut buffer = items.to_vec();
  merge(items, &mut buffer, &mut compare);
}

fn merge<T: Clone>(items: &mut [T], buffer: &mut [T], compare: &mut impl FnMut(&T, &T) -> Ordering) {
  let len = items.len();
  if len < 2 {
    return;
  }
  let middle = len / 2;
  merge(&mut items[..middle], &mut buffer[..middle], compare);
  merge(&mut items[middle..], &mut buffer[middle..], compare);
  buffer[..len].clone_from_slice(items);
  let (mut left, mut right) = (0, middle);
  for slot in items.iter_mut() {
    let take_right = right < len && (left >= middle || compare(&buffer[right], &buffer[left]) == Ordering::Less);
    if take_right {
      *slot = buffer[right].clone();
      right += 1;
    } else {
      *slot = buffer[left].clone();
      left += 1;
    }
  }
}
