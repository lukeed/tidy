//! Lines as prettier-plugin-ini reads them, with its regular expressions written out by hand. JavaScript's `\s` and
//! `.` differ from Rust's whitespace, so the helpers at the end follow JavaScript.

use super::Dialect;

/// A file as the plugin reads it, or as tidy reads a file that the plugin rejects.
pub(crate) enum Tree<'a> {
  Plugin(Vec<Node<'a>>),
  Kept(Vec<Row<'a>>),
}

/// A node of the plugin's tree. `start` and `end` are the plugin's line numbers, which the blank lines between nodes
/// come from: a list's are those of its last value, and a section's are those of its header.
pub(crate) struct Node<'a> {
  pub(crate) start: usize,
  pub(crate) end: usize,
  pub(crate) kind: Kind<'a>,
}

pub(crate) enum Kind<'a> {
  Comment(&'a str),
  Param {
    key: &'a str,
    value: &'a str,
  },
  /// `key =`, then values on the indented lines after it.
  List {
    key: &'a str,
    values: Vec<&'a str>,
  },
  Section {
    name: &'a str,
    children: Vec<Node<'a>>,
  },
}

/// A line of a file in kept mode, with its indent.
pub(crate) struct Row<'a> {
  pub(crate) indent: &'a str,
  pub(crate) line: Line<'a>,
}

/// A line as the plugin's patterns read it.
pub(crate) enum Line<'a> {
  Blank,
  Comment(&'a str),
  Param {
    key: &'a str,
    value: &'a str,
  },
  /// `[name]`, and in kept mode, a comment after it, or a `]` in its name.
  Section {
    name: &'a str,
    comment: Option<&'a str>,
  },
  /// A line that the plugin rejects, or that continues the value of a key above it.
  Text(&'a str),
}

pub(crate) fn parse(text: &str, dialect: Dialect) -> Tree<'_> {
  let lines: Vec<&str> = split(text).collect();
  match dialect {
    Dialect::Ini => plugin(&lines).map_or_else(|| Tree::Kept(kept(&lines)), Tree::Plugin),
    Dialect::Git => Tree::Kept(kept(&lines)),
  }
}

/// The plugin's tree, or `None` where the plugin throws. A blank line ends a section: the lines after it go to the
/// root. A list without values is a key with an empty value: the plugin prints a blank line after it that it doesn't
/// read back.
fn plugin<'a>(lines: &[&'a str]) -> Option<Vec<Node<'a>>> {
  let mut root: Vec<Node> = Vec::new();
  let mut section = false;
  let mut number = 0;
  while number < lines.len() {
    let line = lines[number];
    number += 1;
    let node = match classify(line) {
      Line::Blank => {
        section = false;
        continue;
      }
      Line::Comment(text) => Node { start: number, end: number, kind: Kind::Comment(text) },
      Line::Param { key, value } => {
        match lines.get(number).filter(|_| value.is_empty()).and_then(|next| list_indent(next)) {
          Some(indent) => {
            let mut values = Vec::new();
            while let Some(rest) = lines.get(number).and_then(|line| line.strip_prefix(indent)) {
              let value = trim(rest);
              if value.is_empty() {
                break;
              }
              values.push(value);
              number += 1;
            }
            let kind = match values.is_empty() {
              true => Kind::Param { key, value },
              false => Kind::List { key, values },
            };
            Node { start: number, end: number, kind }
          }
          _ => Node { start: number, end: number, kind: Kind::Param { key, value } },
        }
      }
      Line::Section { name, comment: None } => {
        section = true;
        let kind = Kind::Section { name, children: Vec::new() };
        root.push(Node { start: number, end: number, kind });
        continue;
      }
      Line::Section { .. } | Line::Text(_) => return None,
    };
    match root.last_mut() {
      Some(Node { kind: Kind::Section { children, .. }, .. }) if section => children.push(node),
      _ => root.push(node),
    }
  }
  Some(root)
}

/// The rows of a file in kept mode. A line indented deeper than the key above it continues that key's value, as in
/// Python's configparser and in a git value after a `\`, and stays as it is written.
fn kept<'a>(lines: &[&'a str]) -> Vec<Row<'a>> {
  let mut owner: Option<usize> = None;
  lines
    .iter()
    .map(|line| {
      let body = line.trim_start_matches(space);
      let indent = &line[..line.len() - body.len()];
      let depth = indent.chars().count();
      let line = match classify(line) {
        Line::Blank => {
          owner = None;
          Line::Blank
        }
        _ if owner.is_some_and(|owner| depth > owner) => Line::Text(trim(body)),
        Line::Param { key, .. } if trim(key).is_empty() => {
          owner = Some(depth);
          Line::Text(trim(body))
        }
        Line::Comment(text) => Line::Comment(text),
        Line::Text(_) => match section(body) {
          Some(line) => {
            owner = None;
            line
          }
          None => {
            owner = Some(depth);
            Line::Text(trim(body))
          }
        },
        line @ Line::Param { .. } => {
          owner = Some(depth);
          line
        }
        line @ Line::Section { .. } => {
          owner = None;
          line
        }
      };
      Row { indent, line }
    })
    .collect()
}

/// A line, with the plugin's patterns in the plugin's order.
fn classify(line: &str) -> Line<'_> {
  let body = line.trim_start_matches(space);
  if trim(body).is_empty() {
    return Line::Blank;
  }
  if body.starts_with([';', '#']) && !body.contains(terminator) {
    return Line::Comment(trim(body));
  }
  if let Some(line) = param(line) {
    return line;
  }
  match body.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
    Some((name, after)) if trim(after).is_empty() => {
      Line::Section { name: name.trim_start_matches(space), comment: None }
    }
    _ => Line::Text(trim(body)),
  }
}

/// `^\s*([^=]+?)\s*=\s*(.*?)\s*$`. When only white space comes before the `=`, the key is its last character.
fn param(line: &str) -> Option<Line<'_>> {
  let (before, after) = line.split_once('=')?;
  let start = before.trim_start_matches(space);
  let key = match start.trim_end_matches(space) {
    "" => &before[before.char_indices().next_back()?.0..],
    key => key,
  };
  let value = trim(after);
  (!value.contains(terminator)).then_some(Line::Param { key, value })
}

/// A section header that the plugin rejects: `[name] ; comment`, or `]` in its name. Its name ends at the first `]`
/// after which only white space or a comment follows.
fn section(body: &str) -> Option<Line<'_>> {
  let inner = body.strip_prefix('[')?;
  inner.match_indices(']').find_map(|(at, _)| {
    let rest = trim(&inner[at + 1..]);
    let name = inner[..at].trim_start_matches(space);
    match rest {
      "" => Some(Line::Section { name, comment: None }),
      _ if rest.starts_with([';', '#']) => Some(Line::Section { name, comment: Some(rest) }),
      _ => None,
    }
  })
}

/// The indent of the first value of a list, from `^(\s+).+?\s*$`: the longest run of leading white space that leaves
/// one character which isn't a line terminator, and no line terminator before the trailing white space.
fn list_indent(line: &str) -> Option<&str> {
  let body = line.trim_start_matches(space);
  let lead = &line[..line.len() - body.len()];
  lead.char_indices().rev().map(|(at, c)| at + c.len_utf8()).find_map(|at| {
    let rest = &line[at..];
    let first = rest.chars().next()?;
    let text = rest.trim_end_matches(space);
    (!terminator(first) && !text.contains(terminator)).then(|| &line[..at])
  })
}

/// Lines split at `\r\n`, `\r` and `\n`, as Prettier reads them.
fn split(text: &str) -> impl Iterator<Item = &str> {
  let mut rest = Some(text);
  std::iter::from_fn(move || {
    let text = rest?;
    match memchr::memchr2(b'\r', b'\n', text.as_bytes()) {
      Some(at) => {
        let skip = if text[at..].starts_with("\r\n") { 2 } else { 1 };
        rest = Some(&text[at + skip..]);
        Some(&text[..at])
      }
      None => {
        rest = None;
        Some(text)
      }
    }
  })
}

/// JavaScript's `String.prototype.trim`.
pub(crate) fn trim(text: &str) -> &str {
  text.trim_matches(space)
}

/// JavaScript's `\s`.
fn space(c: char) -> bool {
  matches!(
    c,
    '\t' | '\n' | '\u{B}' | '\u{C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
      ..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
  )
}

/// The characters that JavaScript's `.` doesn't match, but for `\r` and `\n`, which end lines before this.
fn terminator(c: char) -> bool {
  matches!(c, '\u{2028}' | '\u{2029}')
}
