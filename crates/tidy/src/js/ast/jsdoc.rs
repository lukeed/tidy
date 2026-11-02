//! A file's JSDoc comments. See [`File::jsdoc`]. Ported from bun's `bun_lint::ast::jsdoc` (MIT).
//!
//! The names, fields and shapes match the oxc_jsdoc crate, which oxfmt uses to read comments, so code ported from it
//! stays a straight port. The tags are found by oxc_jsdoc's rules, as bun's `bun_sema::check::jsdoc` finds them with
//! `Flavor::Oxc`.
//!
//! Text is a slice of [`File::text`]. Whitespace means `char::is_whitespace`.

use super::{File, Span};
use crate::js::jsdoc::strings;

impl<'a> File<'a> {
  /// The file's `/** */` comments, as oxc_jsdoc reads them. A comment is parsed when it's looked up.
  pub(crate) fn jsdoc(&'a self) -> JSDocs<'a> {
    JSDocs { file: self }
  }
}

/// Empty if `span` is out of bounds.
fn slice(text: &[u8], span: Span) -> &[u8] {
  text.get(span.range()).unwrap_or_default()
}

/// A byte range of the text. Nothing in a comment is at 0, so `0..0` means there's no range.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
struct Range {
  start: u32,
  end: u32,
}

impl Range {
  const NONE: Range = Range { start: 0, end: 0 };

  fn is_some(self) -> bool {
    self.end != 0
  }

  fn span(self) -> Span {
    Span::new(self.start, self.end)
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Tag {
  /// The `@`.
  at: u32,
  /// The name is `at + 1..name_end`. It can be empty.
  name_end: u32,
  /// `{type}`, with the braces.
  ty: Range,
  /// `name`, `a.b`, `[name]`, `[name = default]`, with the brackets.
  name: Range,
  /// Start of the description, which ends at `end`.
  text: u32,
  /// Start of the next tag, or the end of the comment's content.
  end: u32,
}

#[derive(Copy, Clone)]
pub(crate) struct JSDocs<'a> {
  file: &'a File<'a>,
}

impl<'a> JSDocs<'a> {
  /// The comment whose `/**` is at `start`.
  pub(crate) fn at(self, start: u32) -> Option<JSDoc<'a>> {
    let text = self.file.text();
    let from = start as usize;
    let close = from + 2 + strings::index_of(text.get(from + 2..)?, b"*/")?;
    let end = close + 2;
    let comment = &text[from..end];
    if comment.len() < 5 || !comment.starts_with(b"/**") || comment[3] == b'/' {
      return None;
    }
    let tags = match strings::contains_char(comment, b'@') {
      true => self.file.allocator.alloc_slice_copy(&read_tags(text, from, end)),
      false => &[][..],
    };
    Some(JSDoc { text, tags, comment_span: Span::new(start, end as u32) })
  }
}

/// oxc_jsdoc's `parse_jsdoc`: the tags of the comment at `start..end` in `text`. A tag starts at an `@` that
/// [`LineStartRule`] allows and ends where the next one starts.
fn read_tags(text: &[u8], start: usize, end: usize) -> Vec<Tag> {
  let text = &text[..end - 2];
  let mut rule = LineStartRule::new(start + 3);
  let mut tags: Vec<Tag> = Vec::new();
  let mut pos = start + 3;
  while let Some(found) = strings::index_of_char_usize(&text[pos..], b'@') {
    let at = pos + found;
    rule.advance(text, at);
    if !rule.allows_tag() {
      pos = at + 1;
      continue;
    }
    if let Some(last) = tags.last_mut() {
      last.end = at as u32;
    }
    let name_end = at + find_token_range(&text[at..]).map_or(1, |name| name.end);
    tags.push(Tag {
      at: at as u32,
      name_end: name_end as u32,
      ty: Range::NONE,
      name: Range::NONE,
      text: 0,
      end: text.len() as u32,
    });
    pos = name_end;
  }
  for tag in &mut tags {
    parts(text, tag);
  }
  tags
}

/// oxc_jsdoc's `parse_jsdoc_tag`, plus what its `type_name_comment` finds in the tag.
fn parts(text: &[u8], tag: &mut Tag) {
  let slice_of = |from: usize, to: usize| text.get(from..to).unwrap_or_default();
  let (name_end, end) = (tag.name_end as usize, tag.end as usize);
  let found_from = |from: usize, found: Option<std::ops::Range<usize>>| {
    found.map_or(Range::NONE, |found| Range {
      start: (from + found.start) as u32,
      end: (from + found.end) as u32,
    })
  };
  let ty = found_from(name_end, find_type_range(slice_of(name_end, end)));
  let after_type = if ty.is_some() { ty.end as usize } else { name_end };
  let name = found_from(after_type, find_type_name_range(slice_of(after_type, end)));
  tag.ty = ty;
  tag.name = name;
  tag.text = if name.is_some() { name.end } else { after_type as u32 };
}

/// oxc_jsdoc's state at a position in a comment. Its parenthesis and square bracket counters are left out: they're
/// reset at each `\n`, and only whitespace and `*` can come between a `\n` and a tag.
struct LineStartRule {
  /// How far it has read.
  seen: usize,
  curly_brace_depth: u32,
  /// How many backticks opened the code span this position is in.
  backtick_count: usize,
  /// `"`, `'` or 0.
  quote: u8,
  at_line_start: bool,
  line_seen_star: bool,
  spaces_after_star: u32,
}

impl LineStartRule {
  /// `at` is the position after the `/**`.
  fn new(at: usize) -> Self {
    LineStartRule {
      seen: at,
      curly_brace_depth: 0,
      backtick_count: 0,
      quote: 0,
      at_line_start: true,
      line_seen_star: false,
      spaces_after_star: 0,
    }
  }

  fn advance(&mut self, text: &[u8], upto: usize) {
    while self.seen < upto {
      let Some(&byte) = text.get(self.seen) else { return };
      let is_plain = self.backtick_count == 0 && self.quote == 0;
      let mut len = 1;
      match byte {
        b'`' if self.quote == 0 => {
          len = text[self.seen..].iter().take_while(|&&next| next == b'`').count();
          if self.backtick_count == 0 {
            self.backtick_count = len;
          } else if self.backtick_count == len {
            self.backtick_count = 0;
          }
        }
        b'"' | b'\'' if is_plain => self.quote = byte,
        b'"' | b'\'' if self.quote == byte => self.quote = 0,
        b'\n' => self.quote = 0,
        b'{' if is_plain => self.curly_brace_depth = self.curly_brace_depth.saturating_add(1),
        b'}' if is_plain => self.curly_brace_depth = self.curly_brace_depth.saturating_sub(1),
        _ => {}
      }
      if byte == b'\n' {
        self.at_line_start = true;
        self.line_seen_star = false;
        self.spaces_after_star = 0;
      } else if self.at_line_start {
        match byte {
          b'*' => {
            self.line_seen_star = true;
            self.spaces_after_star = 0;
          }
          b' ' | b'\t' | b'\r' => {
            let blanks = u32::from(self.line_seen_star);
            self.spaces_after_star = self.spaces_after_star.saturating_add(blanks);
          }
          _ => self.at_line_start = false,
        }
      }
      self.seen += len;
    }
  }

  fn allows_tag(&self) -> bool {
    self.at_line_start
      && self.curly_brace_depth == 0
      && self.backtick_count == 0
      && !(self.line_seen_star && self.spaces_after_star >= 5)
  }
}

/// The first character of `text` and its length in bytes, or U+FFFD and 1 if it isn't valid UTF-8.
fn decode(text: &[u8]) -> (char, usize) {
  let len = match text.first() {
    Some(0..0x80) => 1,
    Some(0xC0..0xE0) => 2,
    Some(0xE0..0xF0) => 3,
    Some(0xF0..) => 4,
    _ => 1,
  };
  match text.get(..len).and_then(|it| std::str::from_utf8(it).ok()) {
    Some(it) => (it.chars().next().unwrap_or('\u{FFFD}'), len),
    None => ('\u{FFFD}', 1),
  }
}

/// oxc_jsdoc's `find_token_range`: the range of the first word in `text`. A `{` also ends it: `@kind{type}`.
fn find_token_range(text: &[u8]) -> Option<std::ops::Range<usize>> {
  let mut start = None;
  let mut index = 0;
  while index < text.len() {
    let (c, len) = decode(&text[index..]);
    if c.is_whitespace() || c == '{' {
      if let Some(start) = start {
        return Some(start..index);
      }
    } else {
      start.get_or_insert(index);
    }
    index += len;
  }
  start.map(|start| start..text.len())
}

/// `find_type_range`: the range of the `{...}` that `text` starts with, after whitespace.
fn find_type_range(text: &[u8]) -> Option<std::ops::Range<usize>> {
  let trimmed = strings::trim_unicode_whitespace_start(text);
  if !trimmed.starts_with(b"{") {
    return None;
  }
  let offset = text.len() - trimmed.len();
  let mut brace_count = 0usize;
  for (index, &byte) in trimmed.iter().enumerate() {
    match byte {
      b'{' => brace_count += 1,
      b'}' if brace_count == 1 => return Some(offset..offset + index + 1),
      b'}' => brace_count -= 1,
      _ => {}
    }
  }
  None
}

/// `find_type_name_range`. Like `find_token_range`, but `[name = default]` may contain whitespace.
fn find_type_name_range(text: &[u8]) -> Option<std::ops::Range<usize>> {
  if !strings::trim_unicode_whitespace_start(text).starts_with(b"[") {
    return find_token_range(text);
  }
  let mut bracket = 0i32;
  let mut start = None;
  let mut index = 0;
  while index < text.len() {
    let (c, len) = decode(&text[index..]);
    if c.is_whitespace() {
      if bracket == 0
        && let Some(start) = start
      {
        return Some(start..index);
      }
    } else {
      match c {
        '[' => bracket = bracket.saturating_add(1),
        ']' => bracket = bracket.saturating_sub(1),
        _ => {}
      }
      start.get_or_insert(index);
    }
    index += len;
  }
  start.filter(|_| bracket == 0).map(|start| start..text.len())
}

#[derive(Copy, Clone)]
pub(crate) struct JSDoc<'a> {
  text: &'a [u8],
  tags: &'a [Tag],
  comment_span: Span,
}

impl<'a> JSDoc<'a> {
  /// Includes the `/**` and `*/`.
  pub(crate) fn comment_span(self) -> Span {
    self.comment_span
  }

  /// `JSDoc::span`: everything between the `/**` and `*/`.
  pub(crate) fn span(self) -> Span {
    self.comment_span.shrink(3, 2)
  }

  /// `/***/`, `/*****/`, which oxc doesn't treat as JSDoc comments.
  pub(crate) fn is_all_asterisks(self) -> bool {
    slice(self.text, self.span()).iter().all(|&byte| byte == b'*')
  }

  /// `JSDoc::comment`: everything before the first tag.
  pub(crate) fn comment(self) -> JSDocCommentPart<'a> {
    let span = self.span();
    let end = self.tags.first().map_or(span.end, |tag| tag.at);
    JSDocCommentPart::new(self.text, span.start, end)
  }

  /// `JSDoc::tags`
  pub(crate) fn tags(self) -> impl ExactSizeIterator<Item = JSDocTag<'a>> + Clone + 'a {
    let text = self.text;
    self.tags.iter().map(move |row| {
      let raw = slice(text, Span::new(row.at, row.name_end));
      JSDocTag { kind: JSDocTagKindPart { raw }, text, row }
    })
  }
}

#[derive(Copy, Clone)]
pub(crate) struct JSDocTag<'a> {
  pub(crate) kind: JSDocTagKindPart<'a>,
  text: &'a [u8],
  row: &'a Tag,
}

impl<'a> JSDocTag<'a> {
  /// `@kind comment`
  pub(crate) fn comment(self) -> JSDocCommentPart<'a> {
    JSDocCommentPart::new(self.text, self.row.name_end, self.row.end)
  }

  /// `@kind {type}`
  fn r#type(self) -> Option<JSDocTagTypePart<'a>> {
    let ty = self.row.ty;
    ty.is_some().then_some(JSDocTagTypePart { raw: slice(self.text, ty.span()) })
  }

  /// `@kind {type} comment`
  pub(crate) fn type_comment(self) -> (Option<JSDocTagTypePart<'a>>, JSDocCommentPart<'a>) {
    let row = self.row;
    let start = if row.ty.is_some() { row.ty.end } else { row.name_end };
    (self.r#type(), JSDocCommentPart::new(self.text, start, row.end))
  }

  /// `@kind {type} name comment`
  pub(crate) fn type_name_comment(
    self,
  ) -> (Option<JSDocTagTypePart<'a>>, Option<JSDocTagTypeNamePart<'a>>, JSDocCommentPart<'a>) {
    let row = self.row;
    let raw = slice(self.text, row.name.span());
    let name = row.name.is_some().then_some(JSDocTagTypeNamePart {
      optional: raw.starts_with(b"[") && raw.ends_with(b"]"),
      raw,
    });
    (self.r#type(), name, JSDocCommentPart::new(self.text, row.text, row.end))
  }

  /// `@kind{type}`. Not in oxc_jsdoc. The formatter needs it.
  pub(crate) fn has_no_space_before_type(self) -> bool {
    self.text.get(self.row.name_end as usize) == Some(&b'{')
  }
}

/// `@kind`
#[derive(Copy, Clone)]
pub(crate) struct JSDocTagKindPart<'a> {
  raw: &'a [u8],
}

impl<'a> JSDocTagKindPart<'a> {
  /// Without the `@`.
  pub(crate) fn parsed(self) -> &'a [u8] {
    self.raw.get(1..).unwrap_or_default()
  }
}

/// `{type}`
#[derive(Copy, Clone)]
pub(crate) struct JSDocTagTypePart<'a> {
  raw: &'a [u8],
}

impl<'a> JSDocTagTypePart<'a> {
  /// Includes the braces.
  pub(crate) fn raw(self) -> &'a [u8] {
    self.raw
  }

  /// Without the braces and the whitespace just inside them.
  pub(crate) fn parsed(self) -> &'a [u8] {
    let end = self.raw.len().saturating_sub(1);
    strings::trim_unicode_whitespace(self.raw.get(1..end).unwrap_or_default())
  }
}

/// `name`, `[name]`, `[name = default]`
#[derive(Copy, Clone)]
pub(crate) struct JSDocTagTypeNamePart<'a> {
  /// True if it's in brackets.
  pub(crate) optional: bool,
  raw: &'a [u8],
}

impl<'a> JSDocTagTypeNamePart<'a> {
  pub(crate) fn raw(self) -> &'a [u8] {
    self.raw
  }

  /// Just the name.
  pub(crate) fn parsed(self) -> &'a [u8] {
    if !self.optional {
      return self.raw;
    }
    let start = self.raw.iter().take_while(|&&byte| byte == b'[').count();
    let inner = &self.raw[start..];
    let end = inner.len() - inner.iter().rev().take_while(|&&byte| byte == b']').count();
    let inner = strings::trim_unicode_whitespace(&inner[..end]);
    strings::split_once_char(inner, b'=')
      .map_or(inner, |(name, _)| strings::trim_unicode_whitespace(name))
  }
}

/// Strips a comment line's leading `*`. `*word*` at the start of a line is emphasis, so that `*` stays.
fn without_star(trimmed: &[u8]) -> Option<&[u8]> {
  let rest = trimmed.strip_prefix(b"*")?;
  let first = (!rest.is_empty()).then(|| decode(rest).0);
  let is_emphasis = first.is_some_and(|c| c.is_alphanumeric() || c == '_');
  (!is_emphasis).then_some(rest)
}

/// A description's source text, including the `*` at the start of each line.
#[derive(Copy, Clone)]
pub(crate) struct JSDocCommentPart<'a> {
  raw: &'a [u8],
}

impl<'a> JSDocCommentPart<'a> {
  fn new(text: &'a [u8], start: u32, end: u32) -> Self {
    JSDocCommentPart { raw: slice(text, Span::new(start, end)) }
  }

  /// The lines of `parsed()`.
  fn parsed_lines(self) -> impl Iterator<Item = &'a [u8]> + 'a {
    let is_multiline = strings::contains_char(self.raw, b'\n');
    let lines = strings::split(self.raw, b"\n").map(move |line| {
      let trimmed = strings::trim_unicode_whitespace(line);
      match without_star(trimmed).filter(|_| is_multiline) {
        Some(rest) => strings::trim_unicode_whitespace_start(rest),
        None => trimmed,
      }
    });
    lines.filter(|line| !line.is_empty())
  }

  /// Strips the `*` at the start of each line, the whitespace around each line, and empty lines.
  pub(crate) fn parsed(self) -> Vec<u8> {
    let mut result = Vec::with_capacity(self.raw.len());
    for (index, line) in self.parsed_lines().enumerate() {
      if index > 0 {
        result.push(b'\n');
      }
      result.extend_from_slice(line);
    }
    result
  }

  /// Strips the `*` at the start of each line, but keeps empty lines, indentation after `* `, and two trailing spaces
  /// on a line.
  pub(crate) fn parsed_preserving_whitespace(self) -> Vec<u8> {
    if !strings::contains_char(self.raw, b'\n') {
      return strings::trim_unicode_whitespace(self.raw).to_vec();
    }
    let mut result = Vec::with_capacity(self.raw.len());
    for (index, line) in lines(self.raw).enumerate() {
      if index > 0 {
        result.push(b'\n');
      }
      let trimmed = strings::trim_unicode_whitespace(line);
      let content = match without_star(trimmed) {
        Some(rest) => rest.strip_prefix(b" ").unwrap_or(rest),
        None => trimmed,
      };
      result.extend_from_slice(content);
      // Two spaces at the end of a line are a line break in Markdown.
      if line.ends_with(b"  ") && !content.is_empty() {
        result.extend_from_slice(b"  ");
      }
    }
    result
  }
}

/// bstr's `lines`: split at `\n`, without a `\r` before it, and without an empty line after a last `\n`.
fn lines(text: &[u8]) -> impl Iterator<Item = &[u8]> {
  let empty = text.is_empty();
  let text = text.strip_suffix(b"\n").unwrap_or(text);
  strings::split(text, b"\n")
    .filter(move |_| !empty)
    .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
}
