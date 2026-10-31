//! [Tempura](https://github.com/lukeed/tempura) templates: Handlebars-like tags whose contents are JavaScript.
//!
//! The tags are found as tempura's `gen` finds them, and become statements for `super::tokenizer`, as Handlebars'
//! do. Only `#if` and `#each` have bodies. `#expect`, `#var` and custom blocks are single tags.

use super::Error;
use super::ast::{Kind, NodeId, Range, Text, Tree};
use super::error::{Message, Rejection};
use super::parser::{Statement, StatementKind};

/// A tag: `{{ inner }}`, or `{{{ inner }}}` if it's raw.
#[derive(Copy, Clone)]
pub(crate) struct Tag {
  pub(crate) start: usize,
  pub(crate) end: usize,
  pub(crate) inner: (usize, usize),
  pub(crate) is_raw: bool,
}

/// tempura's `tag`: the next tag from `from`. It closes outside quotes, regular expressions and nested braces, except
/// in a comment. `Err` holds the start of a tag that doesn't close.
pub(crate) fn next_tag(source: &[u8], from: usize) -> Result<Option<Tag>, usize> {
  let Some(start) = memchr::memmem::find(&source[from..], b"{{").map(|at| from + at) else {
    return Ok(None);
  };
  let is_raw = source.get(start + 2) == Some(&b'{');
  let inner = start + if is_raw { 3 } else { 2 };
  let first = inner + source[inner..].iter().take_while(|byte| is_space(**byte)).count();
  let is_note = source.get(first) == Some(&b'!');
  let mut depth = 0usize;
  let mut at = inner;
  while at < source.len() {
    let byte = source[at];
    if !is_note && matches!(byte, b'"' | b'\'' | b'`') {
      if let Some(next) = quote_end(source, at) {
        at = next;
        continue;
      }
    } else if !is_note && byte == b'/' && at != first {
      if let Some(next) = regex(source, inner, at) {
        at = next;
        continue;
      }
    } else if !is_note && byte == b'{' {
      depth += 1;
    } else if byte == b'}' {
      if !is_note && depth > 0 {
        depth -= 1;
      } else if source.get(at + 1) == Some(&b'}') && (!is_raw || source.get(at + 2) == Some(&b'}'))
      {
        let end = at + if is_raw { 3 } else { 2 };
        return Ok(Some(Tag { start, end, inner: (inner, at), is_raw }));
      }
    }
    at += 1;
  }
  Err(start)
}

fn is_space(byte: u8) -> bool {
  matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// The end of the string that starts at `at`.
pub(crate) fn quote_end(source: &[u8], at: usize) -> Option<usize> {
  let quote = source[at];
  let mut at = at + 1;
  while at < source.len() {
    match source[at] {
      b'\\' => at += 1,
      byte if byte == quote => return Some(at + 1),
      _ => {}
    }
    at += 1;
  }
  None
}

/// tempura's `group`: the end of the bracketed group that starts at `at`.
pub(crate) fn group_end(source: &[u8], at: usize) -> Option<usize> {
  let (from, mut at, mut depth) = (at, at, 0usize);
  while at < source.len() {
    match source[at] {
      b'"' | b'\'' | b'`' => {
        at = quote_end(source, at)?;
        continue;
      }
      b'/' => {
        if let Some(next) = regex(source, from, at) {
          at = next;
          continue;
        }
      }
      b'(' | b'{' | b'[' => depth += 1,
      b')' | b'}' | b']' => {
        depth = depth.saturating_sub(1);
        if depth == 0 {
          return Some(at + 1);
        }
      }
      _ => {}
    }
    at += 1;
  }
  None
}

/// The end of the regular expression at `at`, or `None` if the slash there divides. `from` is where the code starts.
fn regex(source: &[u8], from: usize, at: usize) -> Option<usize> {
  if !is_operand_next(&source[from..at]) {
    return None;
  }
  let mut is_in_set = false;
  let mut end = at + 1;
  while end < source.len() {
    match source[end] {
      b'\\' => end += 1,
      b'\n' => return None,
      b'[' => is_in_set = true,
      b']' => is_in_set = false,
      b'/' if !is_in_set => {
        end += 1;
        end += source[end..]
          .iter()
          .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'_')
          .count();
        return Some(end);
      }
      _ => {}
    }
    end += 1;
  }
  None
}

/// tempura's `OPERAND` and `STEP`: true if what comes before a slash makes it start a regular expression.
fn is_operand_next(before: &[u8]) -> bool {
  let trimmed = before.trim_ascii_end();
  if trimmed.ends_with(b"++") || trimmed.ends_with(b"--") {
    return false;
  }
  let Some(&last) = trimmed.last() else { return true };
  if b"-(,=:[!&|?{;~^<>*%+".contains(&last) {
    return true;
  }
  let word_len =
    trimmed.iter().rev().take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'_').count();
  let word = &trimmed[trimmed.len() - word_len..];
  let rest = &trimmed[..trimmed.len() - word_len];
  const KEYWORDS: [&[u8]; 13] = [
    b"await",
    b"case",
    b"delete",
    b"do",
    b"else",
    b"in",
    b"instanceof",
    b"new",
    b"of",
    b"return",
    b"typeof",
    b"void",
    b"yield",
  ];
  let is_keyword = !word.is_empty()
    && KEYWORDS.contains(&word)
    && !rest
      .last()
      .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.'));
  // `#if /re/`: a block's name, then whitespace.
  let name_len = trimmed
    .iter()
    .rev()
    .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'/'))
    .count();
  let is_after_name = name_len > 0
    && trimmed.len() < before.len()
    && trimmed[..trimmed.len() - name_len].trim_ascii_end().ends_with(b"#");
  is_keyword || is_after_name
}

/// What a tag is, by its first character.
pub(crate) enum Form<'s> {
  Comment,
  /// `#name rest`
  Open {
    name: &'s [u8],
    rest: (usize, usize),
  },
  /// `/name`
  Close {
    name: &'s [u8],
  },
  Value,
}

/// Splits the tag into its form. `name` and `rest` are trimmed.
pub(crate) fn form<'s>(source: &'s [u8], tag: Tag) -> Form<'s> {
  let (start, end) = trimmed(source, tag.inner);
  match source.get(start) {
    Some(b'!') => Form::Comment,
    Some(b'#') => {
      let name_start =
        start + 1 + source[start + 1..end].iter().take_while(|byte| is_space(**byte)).count();
      let mut name_end = name_start;
      while name_end < end && is_name(source[name_end], name_end > name_start) {
        name_end += 1;
      }
      let rest = trimmed(source, (name_end, end));
      Form::Open { name: &source[name_start..name_end], rest }
    }
    Some(b'/') => Form::Close { name: source[start + 1..end].trim_ascii() },
    _ => Form::Value,
  }
}

fn is_name(byte: u8, is_after_first: bool) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'_' || (is_after_first && matches!(byte, b'.' | b'/'))
}

fn trimmed(source: &[u8], (start, end): (usize, usize)) -> (usize, usize) {
  let start = start + source[start..end].iter().take_while(|byte| is_space(**byte)).count();
  let end = end - source[start..end].iter().rev().take_while(|byte| is_space(**byte)).count();
  (start, end)
}

/// True if the template uses syntax that only tempura has: `#expect`, `#var`, `#elif`, `#else`, or `#each list as
/// item` without Handlebars' pipes.
pub(crate) fn is_tempura(source: &[u8]) -> bool {
  let mut from = 0;
  while let Ok(Some(tag)) = next_tag(source, from) {
    from = tag.end;
    if let Form::Open { name, rest } = form(source, tag) {
      let rest = &source[rest.0..rest.1];
      let is_each_as = name == b"each"
        && !rest.contains(&b'|')
        && rest.windows(4).any(|window| {
          is_space(window[0])
            && &window[1..3] == b"as"
            && matches!(window[3], b' ' | b'\t' | b'\n' | b'(')
        });
      if matches!(name, b"expect" | b"var" | b"elif" | b"else") || is_each_as {
        return true;
      }
    }
  }
  false
}

/// An open `#if` or `#each`, and the statements of its chain: the section, then each `#elif`.
struct Frame {
  name: Vec<u8>,
  chain: Vec<usize>,
  has_else: bool,
}

/// The statements of `source`, for `super::tokenizer::build`.
pub(crate) fn parse(
  source: &[u8],
  tree: &mut Tree,
  statements: &mut Vec<Statement>,
  rejection: &Rejection,
) -> Result<(), Error> {
  let reject = |message: Message, at: usize| Error::Syntax(rejection.note(message, at as u32));
  let mut frames: Vec<Frame> = Vec::new();
  let mut from = 0;
  loop {
    let tag = next_tag(source, from).map_err(|at| reject(Message::ExpectedEndOfMustache, at))?;
    let content_end = tag.map_or(source.len(), |tag| tag.start);
    if content_end > from {
      add(statements, StatementKind::Content, from, content_end);
    }
    let Some(tag) = tag else { break };
    from = tag.end;
    let text = |(start, end): (usize, usize)| Text::source(start, end);
    match form(source, tag) {
      Form::Comment => {
        // Handlebars' value: what's after the `!`, or between the dashes of `{{!-- --}}`.
        let bang = trimmed(source, tag.inner).0;
        let end = tag.inner.1;
        let body = &source[bang + 1..end];
        let value = match body.len() >= 4 && body.starts_with(b"--") && body.ends_with(b"--") {
          true => (bang + 3, end - 2),
          false => (bang + 1, end),
        };
        add(statements, StatementKind::Comment { value: text(value) }, tag.start, tag.end);
      }
      Form::Value => {
        let code = text(trimmed(source, tag.inner));
        let node = tree.add(Kind::Value { code, is_trusting: tag.is_raw }, tag.start, tag.end);
        add(
          statements,
          StatementKind::Mustache { node, is_valid: true, is_verbatim: false },
          tag.start,
          tag.end,
        );
      }
      Form::Open { name: name @ (b"if" | b"each"), rest } => {
        let node = section(tree, name_text(source, name), text(rest), false, tag);
        let index = add(statements, block(node), tag.start, tag.end);
        frames.push(Frame { name: name.to_vec(), chain: vec![index], has_else: false });
      }
      Form::Open { name: name @ (b"elif" | b"else"), rest } => {
        let Some(frame) = frames.last_mut().filter(|frame| frame.name == b"if" && !frame.has_else)
        else {
          return Err(reject(Message::MisplacedBranch, tag.start));
        };
        let current = *frame.chain.last().unwrap_or(&0);
        branch(statements, current);
        if name == b"else" {
          frame.has_else = true;
          continue;
        }
        let node = section(tree, name_text(source, name), text(rest), true, tag);
        let index = add(statements, block(node), tag.start, tag.end);
        frame.chain.push(index);
      }
      Form::Open { name: b"", .. } => return Err(reject(Message::ExpectedName, tag.start)),
      Form::Open { name, rest } => {
        let node = tree.add(
          Kind::Directive { name: name_text(source, name), args: text(rest) },
          tag.start,
          tag.end,
        );
        add(
          statements,
          StatementKind::Mustache { node, is_valid: true, is_verbatim: false },
          tag.start,
          tag.end,
        );
      }
      Form::Close { name } => {
        let Some(frame) = frames.pop() else {
          return Err(reject(Message::UnexpectedClose, tag.start));
        };
        if frame.name != name {
          return Err(reject(Message::WrongNameAtEndOfBlock, tag.start));
        }
        let next = statements.len() as u32;
        for (position, &index) in frame.chain.iter().enumerate() {
          close(statements, index);
          let statement = &mut statements[index];
          statement.next = next;
          // A chained section ends where the closing tag of the whole chain starts.
          statement.end = if position == 0 { tag.end } else { tag.start } as u32;
        }
      }
    }
  }
  match frames.last() {
    Some(frame) => Err(reject(Message::UnclosedBlock, statements[frame.chain[0]].start as usize)),
    None => Ok(()),
  }
}

fn name_text(source: &[u8], name: &[u8]) -> Text {
  let start = name.as_ptr() as usize - source.as_ptr() as usize;
  Text::source(start, start + name.len())
}

fn section(tree: &mut Tree, name: Text, code: Text, is_chained: bool, tag: Tag) -> NodeId {
  let kind = Kind::Section { name, code, program: 0, inverse: 0, is_chained };
  tree.add(kind, tag.start, tag.end)
}

/// A block statement whose first part is still open: `first_end` is 0 until the part ends.
fn block(node: NodeId) -> StatementKind {
  StatementKind::Block {
    node,
    first_end: 0,
    has_second: false,
    is_inverted: false,
    is_valid: true,
    is_verbatim: false,
    block_params: Range::default(),
  }
}

/// Ends the first part of the block statement at `index` with the statements added so far. The next ones are its
/// second part: an `#elif` or the `#else`.
fn branch(statements: &mut [Statement], index: usize) {
  let len = statements.len() as u32;
  if let StatementKind::Block { first_end, has_second, .. } = &mut statements[index].kind {
    (*first_end, *has_second) = (len, true);
  }
}

/// Ends the block statement at `index`, whose first part ends here unless a branch ended it.
fn close(statements: &mut [Statement], index: usize) {
  let len = statements.len() as u32;
  if let StatementKind::Block { first_end: first_end @ 0, .. } = &mut statements[index].kind {
    *first_end = len;
  }
}

fn add(statements: &mut Vec<Statement>, kind: StatementKind, start: usize, end: usize) -> usize {
  let next = statements.len() as u32 + 1;
  statements.push(Statement { kind, start: start as u32, end: end as u32, next });
  statements.len() - 1
}
