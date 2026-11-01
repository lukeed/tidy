//! The values of scalars, and their errors: `yaml`'s `resolve-flow-scalar.ts` and `resolve-block-scalar.ts`. An error
//! is a message and an offset in the scalar's source.

use super::compose::ScalarType;
use crate::syntax::Message;

type Result<T> = std::result::Result<T, (Message, usize)>;

/// `plainValue`
pub(crate) fn plain(source: &str) -> Result<String> {
  if matches!(source.as_bytes().first(), Some(b'\t' | b',' | b'%' | b'|' | b'>' | b'@' | b'`')) {
    return Err((Message::BadScalarStart, 0));
  }
  Ok(fold_lines(source))
}

/// `singleQuotedValue`
pub(crate) fn single_quoted(source: &str) -> Result<String> {
  if !source.ends_with('\'') || source.len() == 1 {
    return Err((Message::UnclosedString, source.len()));
  }
  Ok(fold_lines(&source[1..source.len() - 1]).replace("''", "'"))
}

/// `foldLines`: the lines of a flow scalar, joined by a space, or by line breaks where there are empty lines.
fn fold_lines(source: &str) -> String {
  let Some(first) = source.find('\n') else { return source.to_owned() };
  let mut out = trim_line_end(&source[..first]).to_owned();
  let mut sep = " ";
  let mut rest = &source[first + 1..];
  while let Some(end) = rest.find('\n') {
    let line = trim_line_end(&rest[..end]).trim_start_matches([' ', '\t']);
    if line.is_empty() {
      if sep == "\n" {
        out.push('\n');
      } else {
        sep = "\n";
      }
    } else {
      out.push_str(sep);
      out.push_str(line);
      sep = " ";
    }
    rest = &rest[end + 1..];
  }
  out.push_str(sep);
  out.push_str(rest.trim_start_matches([' ', '\t']));
  out
}

/// A line without the `\r` before its line break, and without spaces and tabs before that.
fn trim_line_end(line: &str) -> &str {
  line.strip_suffix('\r').unwrap_or(line).trim_end_matches([' ', '\t'])
}

/// `doubleQuotedValue`
pub(crate) fn double_quoted(source: &str) -> Result<String> {
  let bytes = source.as_bytes();
  let mut out = String::with_capacity(source.len());
  let mut i = 1;
  while i + 1 < bytes.len() {
    let ch = bytes[i];
    if ch == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
      i += 1;
      continue;
    }
    match ch {
      b'\n' => {
        let mut fold = String::new();
        while let Some(&next) = bytes.get(i + 1) {
          if !matches!(next, b' ' | b'\t' | b'\n' | b'\r')
            || (next == b'\r' && bytes.get(i + 2) != Some(&b'\n'))
          {
            break;
          }
          if next == b'\n' {
            fold.push('\n');
          }
          i += 1;
        }
        out.push_str(if fold.is_empty() { " " } else { &fold });
      }
      b'\\' => {
        i += 1;
        let next = bytes.get(i).copied();
        match next.and_then(escape) {
          Some(value) => out.push_str(value),
          None => match next {
            Some(b'\n') => {
              while matches!(bytes.get(i + 1), Some(b' ' | b'\t')) {
                i += 1;
              }
            }
            Some(b'\r') if bytes.get(i + 1) == Some(&b'\n') => {
              i += 1;
              while matches!(bytes.get(i + 1), Some(b' ' | b'\t')) {
                i += 1;
              }
            }
            Some(kind @ (b'x' | b'u' | b'U')) => {
              let length = match kind {
                b'x' => 2,
                b'u' => 4,
                _ => 8,
              };
              let digits =
                source.get(i + 1..(i + 1 + length).min(source.len())).unwrap_or_default();
              let code = (digits.len() == length
                && digits.bytes().all(|it| it.is_ascii_hexdigit()))
              .then(|| u32::from_str_radix(digits, 16).ok())
              .flatten();
              match code {
                Some(code) if code <= 0x10FFFF => {
                  out.push(char::from_u32(code).unwrap_or('\u{fffd}'))
                }
                _ => return Err((Message::InvalidEscape, i - 1)),
              }
              i += length;
            }
            _ => return Err((Message::InvalidEscape, i - 1)),
          },
        }
      }
      b' ' | b'\t' => {
        let start = i;
        while matches!(bytes.get(i + 1), Some(b' ' | b'\t')) {
          i += 1;
        }
        let next = bytes.get(i + 1).copied();
        if next != Some(b'\n') && !(next == Some(b'\r') && bytes.get(i + 2) == Some(&b'\n')) {
          out.push_str(&source[start..=i]);
        }
      }
      _ => {
        let end = i + utf8_length(ch);
        out.push_str(&source[i..end.min(source.len())]);
        i = end - 1;
      }
    }
    i += 1;
  }
  if !source.ends_with('"') || source.len() == 1 {
    return Err((Message::UnclosedString, source.len()));
  }
  Ok(out)
}

/// `escapeCodes`
fn escape(byte: u8) -> Option<&'static str> {
  Some(match byte {
    b'0' => "\0",
    b'a' => "\x07",
    b'b' => "\x08",
    b'e' => "\x1b",
    b'f' => "\x0c",
    b'n' => "\n",
    b'r' => "\r",
    b't' | b'\t' => "\t",
    b'v' => "\x0b",
    b'N' => "\u{85}",
    b'_' => "\u{a0}",
    b'L' => "\u{2028}",
    b'P' => "\u{2029}",
    b' ' => " ",
    b'"' => "\"",
    b'/' => "/",
    b'\\' => "\\",
    _ => return None,
  })
}

fn utf8_length(byte: u8) -> usize {
  match byte {
    0xF0.. => 4,
    0xE0.. => 3,
    0xC0.. => 2,
    _ => 1,
  }
}

/// `resolveBlockScalar`, from the header and the content after the header's line. `indent` is the parent's, `offset`
/// where the header starts, and `length` that of the header's line. Returns the type and the value.
pub(crate) fn block(
  header: &str,
  source: &str,
  indent: usize,
  offset: usize,
  length: usize,
  at_root: bool,
) -> Result<(ScalarType, String)> {
  let kind =
    if header.starts_with('>') { ScalarType::BlockFolded } else { ScalarType::BlockLiteral };
  let mut explicit = 0;
  let mut chomp = None;
  for (i, ch) in header.char_indices().skip(1) {
    if chomp.is_none() && matches!(ch, '-' | '+') {
      chomp = Some(ch);
    } else {
      match ch.to_digit(10) {
        Some(n) if explicit == 0 && n != 0 => explicit = n as usize,
        _ => return Err((Message::UnexpectedToken, offset + i)),
      }
    }
  }
  let lines = if source.is_empty() { Vec::new() } else { split_lines(source) };
  let mut chomp_start = lines.len();
  for i in (0..lines.len()).rev() {
    if matches!(lines[i].1, "" | "\r") {
      chomp_start = i;
    } else {
      break;
    }
  }
  if chomp_start == 0 {
    let value = match chomp == Some('+') && !lines.is_empty() {
      true => "\n".repeat(1.max(lines.len() - 1)),
      false => String::new(),
    };
    return Ok((kind, value));
  }
  let mut trim = indent + explicit;
  let mut at = offset + length;
  let mut content_start = 0;
  for (i, (line_indent, content)) in lines.iter().enumerate().take(chomp_start) {
    if matches!(*content, "" | "\r") {
      if explicit == 0 && line_indent.len() > trim {
        trim = line_indent.len();
      }
    } else {
      if line_indent.len() < trim {
        return Err((Message::BadIndent, at + line_indent.len()));
      }
      if explicit == 0 {
        trim = line_indent.len();
      }
      content_start = i;
      if trim == 0 && !at_root {
        return Err((Message::BadIndent, at));
      }
      break;
    }
    at += line_indent.len() + content.len() + 1;
  }
  for i in (chomp_start..lines.len()).rev() {
    if lines[i].0.len() > trim {
      chomp_start = i + 1;
    }
  }
  let mut value = String::new();
  let mut sep = "";
  let mut more_indented = false;
  for (line_indent, _) in lines.iter().take(content_start) {
    value.push_str(line_indent.get(trim..).unwrap_or_default());
    value.push('\n');
  }
  for (line_indent, content) in &lines[content_start..chomp_start] {
    at += line_indent.len() + content.len() + 1;
    let content = content.strip_suffix('\r').unwrap_or(content);
    if !content.is_empty() && line_indent.len() < trim {
      return Err((Message::BadIndent, at));
    }
    let rest = line_indent.get(trim..).unwrap_or_default();
    if kind == ScalarType::BlockLiteral {
      value.push_str(sep);
      value.push_str(rest);
      value.push_str(content);
      sep = "\n";
    } else if line_indent.len() > trim || content.starts_with('\t') {
      if sep == " " {
        sep = "\n";
      } else if !more_indented && sep == "\n" {
        sep = "\n\n";
      }
      value.push_str(sep);
      value.push_str(rest);
      value.push_str(content);
      sep = "\n";
      more_indented = true;
    } else if content.is_empty() {
      if sep == "\n" {
        value.push('\n');
      } else {
        sep = "\n";
      }
    } else {
      value.push_str(sep);
      value.push_str(content);
      sep = " ";
      more_indented = false;
    }
  }
  match chomp {
    Some('-') => {}
    Some(_) => {
      for (line_indent, _) in &lines[chomp_start..] {
        value.push('\n');
        value.push_str(line_indent.get(trim..).unwrap_or_default());
      }
      if !value.ends_with('\n') {
        value.push('\n');
      }
    }
    None => value.push('\n'),
  }
  Ok((kind, value))
}

/// `splitLines`: each line's leading spaces, and the rest of it.
fn split_lines(source: &str) -> Vec<(&str, &str)> {
  let mut lines = Vec::new();
  for line in source.split('\n') {
    let spaces = line.len() - line.trim_start_matches(' ').len();
    lines.push(line.split_at(spaces));
  }
  lines
}

/// `timestamp.test` of `yaml`'s YAML 1.1 schema, which `!!timestamp` must match.
pub(crate) fn is_timestamp(value: &str) -> bool {
  let bytes = value.as_bytes();
  let mut at = 0;
  let digits = |at: &mut usize, min: usize, max: usize| {
    let count = bytes[*at..].iter().take(max).take_while(|it| it.is_ascii_digit()).count();
    *at += count;
    count >= min
  };
  let char_at = |at: usize| bytes.get(at).copied();
  if !(digits(&mut at, 4, 4) && char_at(at) == Some(b'-')) {
    return false;
  }
  at += 1;
  if !(digits(&mut at, 1, 2) && char_at(at) == Some(b'-')) {
    return false;
  }
  at += 1;
  if !digits(&mut at, 1, 2) {
    return false;
  }
  if at == bytes.len() {
    return true;
  }
  match char_at(at) {
    Some(b't' | b'T') => at += 1,
    Some(b' ' | b'\t') => {
      at += bytes[at..].iter().take_while(|it| matches!(it, b' ' | b'\t')).count()
    }
    _ => return false,
  }
  if !(digits(&mut at, 1, 2) && char_at(at) == Some(b':')) {
    return false;
  }
  at += 1;
  if !(digits(&mut at, 1, 2) && char_at(at) == Some(b':')) {
    return false;
  }
  at += 1;
  if !digits(&mut at, 1, 2) {
    return false;
  }
  if char_at(at) == Some(b'.') {
    at += 1;
    if !digits(&mut at, 1, usize::MAX) {
      return false;
    }
  }
  at += bytes[at..].iter().take_while(|it| matches!(it, b' ' | b'\t')).count();
  match char_at(at) {
    None => true,
    Some(b'Z') => at + 1 == bytes.len(),
    Some(b'+' | b'-') => {
      at += 1;
      let before = at;
      if char_at(at).is_some_and(|it| matches!(it, b'0'..=b'2'))
        && char_at(at + 1).is_some_and(|it| it.is_ascii_digit())
      {
        at += 2;
      } else if char_at(at).is_some_and(|it| it.is_ascii_digit()) {
        at += 1;
      }
      if at == before {
        return false;
      }
      if char_at(at) == Some(b':') {
        at += 1;
        if !digits(&mut at, 2, 2) {
          return false;
        }
      }
      at == bytes.len()
    }
    _ => false,
  }
}
