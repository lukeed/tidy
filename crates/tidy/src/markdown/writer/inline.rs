use std::borrow::Cow;

use super::Writer;

/// A link whose text is being written. What follows the text is copied as written, unless
/// Prettier's form of it, `tail`, reads back the same.
pub(super) struct Link {
  /// Where the text ends so far.
  pub(super) text: usize,
  pub(super) end: usize,
  pub(super) tail: Option<String>,
}

/// An image whose alt text is skipped to be copied as written, before Prettier's form of the rest.
pub(super) struct Image {
  pub(super) start: usize,
  /// Where the alt text ends so far.
  pub(super) text: usize,
  pub(super) tail: String,
}

/// The `](destination "title")` after an inline link's text, as Prettier writes it.
pub(super) fn tail(url: &str, title: &str) -> Option<String> {
  let url = destination(url, true)?;
  match title {
    "" => Some(format!("]({url})")),
    title => Some(format!("]({url} {})", quote(title)?)),
  }
}

/// A link reference definition as Prettier writes it, from the source it spans.
pub(super) fn definition(source: &str, url: &str, title: Option<&str>) -> Option<String> {
  let label = label(source)?;
  let url = destination(url, false)?;
  let label = squash(label);
  match title {
    None | Some("") => Some(format!("[{label}]: {url}")),
    Some(title) => Some(format!("[{label}]: {url} {}", quote(title)?)),
  }
}

/// `label` with each run of whitespace as one space, as Prettier writes labels. Unlike `collapse`,
/// it joins lines too, and like Prettier it keeps one space at either edge.
pub(super) fn squash(label: &str) -> String {
  let mut out = String::with_capacity(label.len());
  for char in label.chars() {
    let space = char.is_ascii_whitespace();
    if !(space && out.ends_with(' ')) {
      out.push(if space { ' ' } else { char });
    }
  }
  out
}

/// An image reference with its label as Prettier writes it, or `None` to keep `raw` as written:
/// `![alt][label]`, `![label][]`, or `![label]`.
pub(super) fn image(raw: &str, link_type: pulldown_cmark::LinkType) -> Option<String> {
  use pulldown_cmark::LinkType;
  let open = match link_type {
    LinkType::Reference => raw.rfind('[')?,
    LinkType::Collapsed | LinkType::Shortcut => 1,
    _ => return None,
  };
  let label = label(&raw[open..])?;
  if label.contains('\\') {
    return None;
  }
  let after = &raw[open + label.len() + 2..];
  Some(format!("{}[{}]{after}", &raw[..open], squash(label)))
}

/// The label of a definition or full reference that `source` starts with, as written.
pub(super) fn label(source: &str) -> Option<&str> {
  let rest = source.strip_prefix('[')?;
  let mut escaped = false;
  for (index, char) in rest.char_indices() {
    match char {
      ']' if !escaped => return Some(&rest[..index]),
      '[' if !escaped => return None,
      _ => escaped = char == '\\' && !escaped,
    }
  }
  None
}

/// A destination written bare, or in `<>` when it holds a space, or a `)` that would end an inline
/// link. `None` where writing the parsed URL back would read differently.
fn destination(url: &str, inline: bool) -> Option<Cow<'_, str>> {
  if url.is_empty() {
    return Some(Cow::Borrowed("<>"));
  }
  let (mut depth, mut balanced, mut space, mut close, mut amp) =
    (0usize, true, false, false, false);
  for byte in url.bytes() {
    match byte {
      b'\\' | b'<' | b'>' | b'\n' | b'\r' => return None,
      b' ' => space = true,
      b'&' => amp = true,
      b'(' => depth += 1,
      b')' => {
        close = true;
        match depth.checked_sub(1) {
          Some(less) => depth = less,
          None => balanced = false,
        }
      }
      _ => {}
    }
  }
  if amp && entity(url) {
    return None;
  }
  if space || (inline && close) {
    return Some(Cow::Owned(format!("<{url}>")));
  }
  (balanced && depth == 0).then_some(Cow::Borrowed(url))
}

/// A title in double quotes, single quotes when it holds more double quotes, or parentheses when
/// it holds both but no parentheses or backslashes.
fn quote(title: &str) -> Option<String> {
  if entity(title) {
    return None;
  }
  let doubles = title.matches('"').count();
  let singles = title.matches('\'').count();
  if doubles > 0 && singles > 0 && !title.contains(['(', ')', '\\']) {
    return Some(format!("({title})"));
  }
  let mark = if doubles > singles { '\'' } else { '"' };
  let escaped = title.replace('\\', "\\\\").replace(mark, &format!("\\{mark}"));
  Some(format!("{mark}{escaped}{mark}"))
}

/// Whether `text` holds something that reads as an entity, which the parser would decode.
fn entity(text: &str) -> bool {
  text.match_indices('&').any(|(at, _)| {
    let name =
      text[at + 1..].bytes().take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'#');
    let length = name.count();
    length > 0 && text.as_bytes().get(at + 1 + length) == Some(&b';')
  })
}

/// `text` with each run of spaces and tabs as one space.
pub(super) fn collapse(text: &str) -> Cow<'_, str> {
  let bytes = text.as_bytes();
  if memchr::memchr(b'\t', bytes).is_none() && memchr::memmem::find(bytes, b"  ").is_none() {
    return Cow::Borrowed(text);
  }
  let mut out = String::with_capacity(text.len());
  for char in text.chars() {
    let space = matches!(char, ' ' | '\t');
    if !(space && out.ends_with(' ')) {
      out.push(if space { ' ' } else { char });
    }
  }
  Cow::Owned(out)
}

/// A code span rewritten as Prettier writes it, or `None` when `raw` already is: the fewest
/// backticks that no run inside matches, padded with a space only where the content needs one.
/// A `joined` span is written from its parsed value, where each line break is one space.
pub(super) fn span(raw: &str, value: &str, joined: bool) -> Option<String> {
  let ticks = raw.bytes().take_while(|&byte| byte == b'`').count();
  let inner = &raw[ticks..raw.len() - ticks];
  let unpadded = inner
    .strip_prefix([' ', '\n'])
    .and_then(|rest| rest.strip_suffix([' ', '\n']))
    .filter(|_| inner.bytes().any(|byte| !matches!(byte, b' ' | b'\n' | b'\r')))
    .unwrap_or(inner);
  let mut fewest = 1;
  while has_run(value, fewest) {
    fewest += 1;
  }
  let edge = |byte: Option<u8>| matches!(byte, Some(b' ' | b'\n'));
  let bytes = value.as_bytes();
  let pad = value.starts_with('`')
    || value.ends_with('`')
    || (edge(bytes.first().copied())
      && edge(bytes.last().copied())
      && bytes.iter().any(|byte| !matches!(byte, b' ' | b'\n')));
  let spaced = inner.starts_with(' ') && inner.ends_with(' ');
  let unpadded = if joined { value } else { unpadded };
  if !joined && ticks == fewest && (unpadded.len() < inner.len()) == pad && (!pad || spaced) {
    return None;
  }
  let fence = "`".repeat(fewest);
  let pad = if pad { " " } else { "" };
  Some(format!("{fence}{pad}{unpadded}{pad}{fence}"))
}

/// Whether `text` has a run of exactly `length` backticks.
pub(super) fn has_run(text: &str, length: usize) -> bool {
  text.split(|char| char != '`').any(|run| run.len() == length)
}

impl<'s> Writer<'s, '_> {
  /// Whether emphasis ending at `end` touches a word on either side, where `_` doesn't work.
  pub(super) fn inside(&self, end: usize) -> bool {
    let after = self.source[end..].chars().next();
    self.before().is_some_and(char::is_alphanumeric) || after.is_some_and(char::is_alphanumeric)
  }

  /// The last character written on the current line.
  pub(super) fn before(&self) -> Option<char> {
    match (&self.paragraph, &self.table) {
      (_, Some(table)) => {
        table.rows.last().and_then(|row| row.last()).and_then(|cell| cell.chars().next_back())
      }
      (Some(paragraph), None) => paragraph.last(),
      (None, None) if self.start => None,
      (None, None) => self.out.chars().next_back(),
    }
  }
}
