//! Templates that aren't HTML, such as `robots.txt.hbs`: the text stays as it is written. Only the tags change: a block
//! tag alone on its line is indented by how deep it is in blocks, and the inside of each tag is trimmed. A tag alone on
//! its line leaves no trace when the template renders, so the output renders the same.

use super::tempura::{self, Form, Tag};

/// The template's text, with its tags formatted. `indent` is one level of indentation. Returns `None` if a tag doesn't
/// close.
pub(crate) fn format(source: &str, is_tempura: bool, indent: &str) -> Option<String> {
  let bytes = source.as_bytes();
  let mut out = String::with_capacity(source.len());
  let mut depth = 0usize;
  let mut from = 0;
  while let Some(tag) = tempura::next_tag(bytes, from).ok()? {
    let (kind, name) = kind(bytes, tag, is_tempura);
    let (start, end) = line(bytes, tag);
    let is_standalone = kind != Kind::Value
      && start == line_start(bytes, tag.start)
      && (end > tag.end || end == bytes.len());
    let level = match kind {
      Kind::Branch | Kind::Close => depth.saturating_sub(1),
      _ => depth,
    };
    match is_standalone {
      true => {
        out.push_str(&source[from..start]);
        out.push_str(&indent.repeat(level));
        out.push_str(&written(source, tag, kind, name, is_tempura));
        out.push_str(&source[tag.end..end]);
        from = end;
      }
      false => {
        out.push_str(&source[from..tag.start]);
        out.push_str(&written(source, tag, kind, name, is_tempura));
        from = tag.end;
      }
    }
    match kind {
      Kind::Open => depth += 1,
      Kind::Close => depth = depth.saturating_sub(1),
      _ => {}
    }
  }
  out.push_str(&source[from..]);
  Some(out)
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Kind {
  Open,
  Branch,
  Close,
  /// A comment, or a tag with no body.
  Single,
  Value,
}

fn kind(source: &[u8], tag: Tag, is_tempura: bool) -> (Kind, usize) {
  let inner = source[tag.inner.0..tag.inner.1].trim_ascii();
  match tempura::form(source, tag) {
    Form::Comment => (Kind::Single, 0),
    Form::Open { name, .. } => {
      let kind = match (is_tempura, name) {
        (true, b"if" | b"each") | (false, _) => Kind::Open,
        (true, b"elif" | b"else") => Kind::Branch,
        (true, _) => Kind::Single,
      };
      (kind, name.len())
    }
    Form::Close { .. } => (Kind::Close, 0),
    Form::Value
      if !is_tempura && (inner == b"else" || inner.starts_with(b"else ") || inner == b"^") =>
    {
      (Kind::Branch, 0)
    }
    Form::Value => (Kind::Value, 0),
  }
}

/// The tag with its inside trimmed. In tempura, a value gets a space inside its braces.
fn written(source: &str, tag: Tag, kind: Kind, name: usize, is_tempura: bool) -> String {
  let inner = source[tag.inner.0..tag.inner.1].trim_matches([' ', '\t', '\n', '\r']);
  let (open, close) = if tag.is_raw { ("{{{", "}}}") } else { ("{{", "}}") };
  match kind {
    Kind::Value if is_tempura => format!("{open} {inner} {close}"),
    Kind::Value => format!("{open}{inner}{close}"),
    Kind::Single if inner.starts_with('!') => source[tag.start..tag.end].to_owned(),
    Kind::Open | Kind::Branch | Kind::Single if inner.starts_with('#') => {
      let after = inner[1..].trim_start_matches([' ', '\t', '\n', '\r']);
      let (name, rest) = after.split_at(name.min(after.len()));
      let rest = rest.trim_matches([' ', '\t', '\n', '\r']);
      match rest.is_empty() {
        true => format!("{open}#{name}{close}"),
        false => format!("{open}#{name} {rest}{close}"),
      }
    }
    _ => format!("{open}{inner}{close}"),
  }
}

/// The start of the line that `at` is on.
fn line_start(source: &[u8], at: usize) -> usize {
  source[..at].iter().rposition(|byte| *byte == b'\n').map_or(0, |at| at + 1)
}

/// tempura's `line`: the tag's line, from its start through its line break, if nothing but blanks share it. Otherwise
/// the tag itself.
fn line(source: &[u8], tag: Tag) -> (usize, usize) {
  let blank = |byte: &u8| matches!(byte, b' ' | b'\t');
  let start = tag.start - source[..tag.start].iter().rev().take_while(|byte| blank(byte)).count();
  let mut end = tag.end + source[tag.end..].iter().take_while(|byte| blank(byte)).count();
  if start > 0 && source[start - 1] != b'\n' {
    return (tag.start, tag.end);
  }
  if source.get(end) == Some(&b'\r') {
    end += 1;
  }
  match source.get(end) {
    None => (start, end),
    Some(b'\n') => (start, end + 1),
    Some(_) => (tag.start, tag.end),
  }
}
