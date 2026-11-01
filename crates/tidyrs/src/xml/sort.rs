//! `xmlSortAttributesByKey`: the plugin's order of attribute names, which compares them with `localeCompare`.

use std::cmp::Ordering;

/// The plugin's comparison of two attribute names: `xmlns` first, then names with a namespace, by namespace (`xmlns`
/// first) and then by key, then the rest.
pub(crate) fn attributes(left: &str, right: &str) -> Ordering {
  if left == "xmlns" {
    return Ordering::Less;
  }
  if right == "xmlns" {
    return Ordering::Greater;
  }
  match (namespaced(left), namespaced(right)) {
    (Some((left_space, left_key)), Some((right_space, right_key))) => {
      if left_space == right_space {
        return compare(left_key, right_key);
      }
      if left_space == "xmlns" {
        return Ordering::Less;
      }
      if right_space == "xmlns" {
        return Ordering::Greater;
      }
      compare(left_space, right_space)
    }
    (Some(_), None) => Ordering::Less,
    (None, Some(_)) => Ordering::Greater,
    (None, None) => compare(left, right),
  }
}

/// `const [namespace, key] = name.split(":")`
fn namespaced(name: &str) -> Option<(&str, &str)> {
  let (namespace, rest) = name.split_once(':')?;
  Some((namespace, rest.split(':').next().unwrap_or(rest)))
}

/// `left.localeCompare(right)` in the root collation of ICU, for the characters of a name: punctuation (`_ - : . ·`),
/// then digits, then letters, compared without case first, and then with lower case first.
pub(crate) fn compare(left: &str, right: &str) -> Ordering {
  let case = |char: char| char.is_ascii_uppercase();
  (left.chars().map(weight).cmp(right.chars().map(weight)))
    .then_with(|| left.chars().map(case).cmp(right.chars().map(case)))
}

/// The primary weight of a character. Characters that a name can't have come after letters, by code point.
fn weight(char: char) -> u32 {
  match char {
    '_' => 0,
    '-' => 1,
    ':' => 2,
    '.' => 3,
    '\u{B7}' => 4,
    '0'..='9' => 5 + (char as u32 - '0' as u32),
    'a'..='z' => 15 + (char as u32 - 'a' as u32),
    'A'..='Z' => 15 + (char as u32 - 'A' as u32),
    other => 0x100 + other as u32,
  }
}
