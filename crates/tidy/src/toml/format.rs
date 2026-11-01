//! taplo's formatter (`formatter/mod.rs`), which writes text as it walks the tree, without a document IR.

use super::Options;
use super::lexer::Token;
use super::tree::{Id, Kind, Tree};

#[derive(Clone, Copy, Default)]
struct Context {
  indent_level: usize,
  force_multiline: bool,
}

pub(crate) struct Formatter<'f> {
  pub(crate) tree: &'f Tree,
  pub(crate) text: &'f str,
  pub(crate) options: &'f Options,
  pub(crate) indent: String,
  pub(crate) newline: &'static str,
}

/// An entry, written and waiting to be aligned with the others of its group.
struct Entry {
  syntax: Id,
  key: String,
  value: String,
  comment: Option<String>,
}

impl Entry {
  /// `cleaned_key`: the key without quotes, split at its dots, which `reorder_keys` sorts by.
  fn cleaned_key(&self) -> Vec<String> {
    self.key.replace(['\'', '"'], "").split('.').map(str::to_owned).collect()
  }

  fn write_to(&self, formatted: &mut String, options: &Options) {
    formatted.push_str(&self.key);
    formatted.push_str(if options.compact_entries { "=" } else { " = " });
    formatted.push_str(&self.value);
  }
}

impl Formatter<'_> {
  /// `format_with_path_scopes`, without scopes, as `taplo fmt` runs it.
  pub(crate) fn format(&self) -> String {
    let mut formatted = self.root(self.tree.root());
    if formatted.ends_with("\r\n") {
      formatted.truncate(formatted.len() - 2);
    } else if formatted.ends_with('\n') {
      formatted.truncate(formatted.len() - 1);
    }
    if self.options.trailing_newline {
      formatted.push_str(self.newline);
    }
    formatted.truncate(formatted.trim_end().len());
    if self.options.trailing_newline {
      formatted.push_str(self.newline);
    }
    formatted
  }

  fn indent(&self, context: Context) -> String {
    self.indent.repeat(context.indent_level)
  }

  /// `newlines`: `count` line breaks, but at most one more than the blank lines allowed.
  fn newlines(&self, count: usize, formatted: &mut String) {
    for _ in 0..count.min(self.options.allowed_blank_lines + 1) {
      formatted.push_str(self.newline);
    }
  }

  fn should_align_comments(&self, count: usize) -> bool {
    (count != 1 || self.options.align_single_comments) && self.options.align_comments
  }

  fn text(&self, id: Id) -> &str {
    self.tree.text(id, self.text)
  }

  fn root(&self, node: Id) -> String {
    let options = self.options;
    let mut formatted = String::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut skip_newlines = 0;
    let mut comments: Vec<String> = Vec::new();
    let mut context = Context::default();
    let mut history: Vec<(Vec<String>, usize)> = Vec::new();
    let mut dangling = 0;
    for &child in self.tree.children(node) {
      match self.tree.kind(child) {
        Kind::TableArrayHeader | Kind::TableHeader => {
          if self.add_entries(&mut entries, &mut formatted, context) {
            formatted.push_str(self.newline);
            skip_newlines = 0;
          }
          if options.indent_entries && context.indent_level == 0 {
            context.indent_level = 1;
          }
          if let Some(key) = self.tree.nodes(child).next() {
            let key = self.keys(key);
            if options.indent_tables {
              let default = options.indent_entries as usize;
              context.indent_level = table_indent_level(&history, &key, default);
            }
            history.push((key, context.indent_level));
          }
          let mut header = context;
          if options.indent_entries {
            header.indent_level = header.indent_level.saturating_sub(1);
          }
          if self.add_comments(&mut comments, &mut formatted, header) {
            formatted.push_str(self.newline);
            skip_newlines = 0;
          }
          let (text, comment) = self.table_header(child);
          if options.indent_tables {
            formatted.push_str(&self.indent(header));
          }
          formatted.push_str(&text);
          if let Some(comment) = comment {
            formatted.push(' ');
            formatted.push_str(&comment);
          }
        }
        Kind::Entry => {
          if self.add_comments(&mut comments, &mut formatted, context) {
            formatted.push_str(self.newline);
            skip_newlines = 0;
          }
          entries.push(self.entry(child, context));
          skip_newlines += 1;
        }
        Kind::Token(Token::Newline) => {
          let mut count = newline_count(self.text(child));
          match self.dangling_newlines(child) {
            Some(more) => {
              dangling += more;
              continue;
            }
            None => {
              count += dangling;
              dangling = 0;
            }
          }
          if count > 1 {
            self.add_comments(&mut comments, &mut formatted, context);
            self.add_entries(&mut entries, &mut formatted, context);
            skip_newlines = 0;
          }
          self.newlines(count.saturating_sub(skip_newlines), &mut formatted);
        }
        Kind::Token(Token::Comment) => {
          if self.add_entries(&mut entries, &mut formatted, context) {
            formatted.push_str(self.newline);
            skip_newlines = 0;
          }
          comments.push(self.text(child).to_owned());
          skip_newlines += 1;
        }
        Kind::Token(Token::Whitespace) => {}
        _ => formatted.push_str(self.text(child)),
      }
    }
    self.add_comments(&mut comments, &mut formatted, context);
    self.add_entries(&mut entries, &mut formatted, context);
    formatted
  }

  /// The parts of a key, unquoted and unescaped: taplo's `Keys`.
  fn keys(&self, key: Id) -> Vec<String> {
    let parts = self.tree.children(key).iter().filter(|it| self.tree.is(**it, Token::Ident));
    parts.map(|part| unquote(self.text(*part))).collect()
  }

  fn add_comments(
    &self,
    comments: &mut Vec<String>,
    formatted: &mut String,
    context: Context,
  ) -> bool {
    let were = !comments.is_empty();
    for (index, comment) in comments.drain(..).enumerate() {
      if index != 0 {
        formatted.push_str(self.newline);
      }
      formatted.push_str(&self.indent(context));
      formatted.push_str(&comment);
    }
    were
  }

  fn add_entries(
    &self,
    entries: &mut Vec<Entry>,
    formatted: &mut String,
    context: Context,
  ) -> bool {
    let options = self.options;
    let were = !entries.is_empty();
    if options.reorder_keys {
      entries.sort_by_cached_key(Entry::cleaned_key);
    }
    let indent_chars = context.indent_level * self.indent.chars().count();
    if options.array_auto_expand {
      for entry in entries.iter_mut() {
        let comment_chars = entry.comment.as_ref().map_or(0, |comment| comment.chars().count() + 1);
        let line_count = entry.value.split('\n').count();
        for (index, line) in entry.value.split('\n').enumerate() {
          let mut chars = line.chars().count();
          if index == 0 {
            chars += indent_chars + entry.key.chars().count();
            chars += if options.compact_entries { 1 } else { 3 };
          }
          if index == line_count - 1 {
            chars += comment_chars;
          }
          if chars > options.column_width {
            let context = Context { force_multiline: true, ..context };
            let value = self.tree.nodes(entry.syntax).find(|it| self.tree.kind(*it) == Kind::Value);
            let (value, comment) = self.value(value.expect("an entry has a value"), context);
            if comment.is_some() {
              entry.comment = comment;
            }
            entry.value = value;
            break;
          }
        }
      }
    }
    let mut comment_count = 0;
    let indent = self.indent(context);
    let rows: Vec<Vec<String>> = entries
      .drain(..)
      .map(|entry| {
        let mut row = vec![indent.clone(), entry.key, "=".to_owned(), entry.value];
        if let Some(comment) = entry.comment {
          row.push(comment);
          comment_count += 1;
        }
        row
      })
      .collect();
    let align_comments = self.should_align_comments(comment_count);
    let align = match (options.align_entries, align_comments) {
      (false, false) => 0..0,
      (false, true) => 3..usize::MAX,
      (true, false) => 0..3,
      (true, true) => 0..usize::MAX,
    };
    let separator = if options.compact_entries { 3..usize::MAX } else { 1..usize::MAX };
    formatted.push_str(&format_rows(align, separator, &rows, self.newline, " "));
    were
  }

  fn entry(&self, node: Id, context: Context) -> Entry {
    let (mut key, mut value, mut comment) = (String::new(), String::new(), None);
    for &child in self.tree.children(node) {
      match self.tree.kind(child) {
        Kind::Key => self.key(child, &mut key),
        Kind::Value => {
          let (text, trailing) = self.value(child, context);
          if trailing.is_some() {
            comment = trailing;
          }
          value.push_str(&text);
        }
        Kind::Token(Token::Comment) => comment = Some(self.text(child).to_owned()),
        _ => {}
      }
    }
    Entry { syntax: node, key, value, comment }
  }

  /// `format_key`: its parts and dots, without whitespace.
  fn key(&self, node: Id, formatted: &mut String) {
    for &child in self.tree.children(node) {
      match self.tree.kind(child) {
        Kind::Token(Token::Whitespace | Token::Newline) => {}
        Kind::Token(_) => formatted.push_str(self.text(child)),
        _ => {}
      }
    }
  }

  /// `format_value`: the value, and the comment after it.
  fn value(&self, node: Id, context: Context) -> (String, Option<String>) {
    let (mut value, mut comment) = (String::new(), None);
    for &child in self.tree.children(node) {
      match self.tree.kind(child) {
        Kind::Array => {
          let (text, trailing) = self.array(child, context);
          comment = trailing.or(comment);
          value = text;
        }
        Kind::InlineTable => {
          let (text, trailing) = self.inline_table(child, context);
          comment = trailing.or(comment);
          value = text;
        }
        Kind::Token(Token::Newline | Token::Whitespace) => {}
        Kind::Token(Token::Comment) => comment = Some(self.text(child).to_owned()),
        Kind::Token(_) => value = self.text(child).to_owned(),
        _ => {}
      }
    }
    (value, comment)
  }

  fn inline_table(&self, node: Id, context: Context) -> (String, Option<String>) {
    let options = self.options;
    let mut formatted = String::new();
    let mut comment = None;
    let context = Context {
      force_multiline: context.force_multiline && options.inline_table_expand,
      ..context
    };
    let count = self.tree.nodes(node).count();
    if count == 0 {
      formatted.push_str("{}");
    }
    let mut sorted: std::collections::VecDeque<Id> = match options.reorder_inline_tables {
      true => {
        let mut nodes: Vec<Id> = self.tree.nodes(node).collect();
        nodes.sort_unstable_by(|a, b| self.text(*a).cmp(self.text(*b)));
        nodes.into()
      }
      false => Default::default(),
    };
    let mut index = 0;
    for &child in self.tree.children(node) {
      match self.tree.kind(child) {
        Kind::Token(Token::BraceStart) if count > 0 => {
          formatted.push('{');
          if !options.compact_inline_tables {
            formatted.push(' ');
          }
        }
        Kind::Token(Token::BraceEnd) if count > 0 => {
          if !options.compact_inline_tables {
            formatted.push(' ');
          }
          formatted.push('}');
        }
        Kind::Token(Token::BraceStart | Token::BraceEnd | Token::Whitespace | Token::Comma) => {}
        Kind::Token(Token::Comment) => comment = Some(self.text(child).to_owned()),
        Kind::Token(_) => formatted.push_str(self.text(child)),
        _ => {
          if index != 0 {
            formatted.push_str(", ");
          }
          let entry = sorted.pop_front().unwrap_or(child);
          self.entry(entry, context).write_to(&mut formatted, options);
          index += 1;
        }
      }
    }
    (formatted, comment)
  }

  fn array(&self, node: Id, context: Context) -> (String, Option<String>) {
    let options = self.options;
    let tree = self.tree;
    let mut multiline = tree.any_descendant(node, &|kind| kind == Kind::Token(Token::Newline))
      || context.force_multiline;
    let collapsible = !tree.any_descendant(node, &|kind| kind == Kind::Token(Token::Comment));
    if collapsible && options.array_auto_collapse && !context.force_multiline {
      multiline = false;
    }
    let mut formatted = String::new();
    let mut skip_newlines = 0;
    let mut values: Vec<(String, Option<String>)> = Vec::new();
    let mut commas: Vec<bool> = Vec::new();
    let count = tree.nodes(node).count();
    let inner = Context { indent_level: context.indent_level + multiline as usize, ..context };
    let mut dangling = 0;
    let mut index = 0;
    for &child in tree.children(node) {
      match tree.kind(child) {
        Kind::Value => {
          if multiline && formatted.ends_with('[') {
            formatted.push_str(self.newline);
          }
          let (value, comment) = self.value(child, inner);
          commas.push(index + 1 < count || (multiline && options.array_trailing_comma));
          values.push((value, comment));
          skip_newlines += 1;
          index += 1;
        }
        Kind::Token(Token::BracketStart) => {
          formatted.push('[');
          if !options.compact_arrays && !multiline {
            formatted.push(' ');
          }
        }
        Kind::Token(Token::BracketEnd) => {
          self.add_values(&mut values, &mut commas, &mut formatted, inner, multiline);
          if multiline {
            if !formatted.ends_with('\n') {
              formatted.push_str(self.newline);
            }
            formatted.push_str(&self.indent(context));
          } else if !options.compact_arrays {
            formatted.push(' ');
          }
          formatted.push(']');
        }
        Kind::Token(Token::Newline) => {
          if !multiline {
            continue;
          }
          let mut newlines = newline_count(self.text(child));
          match self.dangling_newlines(child) {
            Some(more) => {
              dangling += more;
              continue;
            }
            None => {
              newlines += dangling;
              dangling = 0;
            }
          }
          if newlines > 1 {
            self.add_values(&mut values, &mut commas, &mut formatted, inner, multiline);
            skip_newlines = 0;
          }
          self.newlines(newlines.saturating_sub(skip_newlines), &mut formatted);
        }
        Kind::Token(Token::Comment) => {
          let before = (1..)
            .map_while(|offset| tree.sibling(child, -offset))
            .find(|sibling| !tree.is(*sibling, Token::Whitespace));
          let newline_before = before.is_some_and(|sibling| tree.is(sibling, Token::Newline));
          if !newline_before && let Some(last) = values.last_mut() {
            last.1 = Some(self.text(child).to_owned());
            continue;
          }
          if self.add_values(&mut values, &mut commas, &mut formatted, inner, multiline) {
            formatted.push_str(self.newline);
            skip_newlines = 0;
          }
          if formatted.ends_with('[') {
            formatted.push(' ');
          } else {
            formatted.push_str(&self.indent(inner));
          }
          formatted.push_str(self.text(child));
        }
        _ => {}
      }
    }
    if formatted.is_empty() {
      formatted.push_str("[]");
    }
    (formatted, None)
  }

  fn add_values(
    &self,
    values: &mut Vec<(String, Option<String>)>,
    commas: &mut Vec<bool>,
    formatted: &mut String,
    context: Context,
    multiline: bool,
  ) -> bool {
    let were = !values.is_empty();
    if self.options.reorder_arrays {
      values.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    }
    for (comma, value) in commas.drain(..).zip(values.iter_mut()) {
      if comma {
        value.0.push(',');
      }
    }
    if !multiline {
      for (index, (value, _)) in values.drain(..).enumerate() {
        if index != 0 {
          formatted.push(' ');
        }
        formatted.push_str(&value);
      }
      return were;
    }
    let mut comment_count = 0;
    let indent = self.indent(context);
    let rows: Vec<Vec<String>> = values
      .drain(..)
      .map(|(value, comment)| {
        let mut row = vec![indent.clone(), value];
        if let Some(comment) = comment {
          row.push(comment);
          comment_count += 1;
        }
        row
      })
      .collect();
    let align = if self.should_align_comments(comment_count) { 0..usize::MAX } else { 0..0 };
    formatted.push_str(&format_rows(align, 1..usize::MAX, &rows, self.newline, " "));
    were
  }

  fn table_header(&self, node: Id) -> (String, Option<String>) {
    let (mut formatted, mut comment) = (String::new(), None);
    for &child in self.tree.children(node) {
      match self.tree.kind(child) {
        Kind::Token(Token::Whitespace | Token::Newline) => {}
        Kind::Token(Token::Comment) => comment = Some(self.text(child).to_owned()),
        Kind::Token(_) => formatted.push_str(self.text(child)),
        _ => self.key(child, &mut formatted),
      }
    }
    (formatted, comment)
  }

  /// `dangling_newlines`: the line breaks of a token that whitespace and more line breaks follow, which count with them.
  fn dangling_newlines(&self, token: Id) -> Option<usize> {
    let next = self.tree.sibling(token, 1)?;
    let after = self.tree.sibling(token, 2)?;
    let dangling = self.tree.is(next, Token::Whitespace) && self.tree.is(after, Token::Newline);
    dangling.then(|| newline_count(self.text(token)))
  }
}

fn newline_count(text: &str) -> usize {
  text.bytes().filter(|byte| *byte == b'\n').count()
}

/// `table_indent_level`: one more than the last table whose key is a strict prefix of this one.
fn table_indent_level(history: &[(Vec<String>, usize)], key: &[String], default: usize) -> usize {
  history
    .iter()
    .rev()
    .find_map(|(previous, indent)| {
      let prefix = key.len() >= previous.len() && key.starts_with(previous);
      (prefix && key != previous.as_slice()).then_some(indent + 1)
    })
    .unwrap_or(default)
}

/// A part of a key without its quotes, and with the escapes of a basic string read.
fn unquote(text: &str) -> String {
  if let Some(literal) = text.strip_prefix('\'') {
    return literal.strip_suffix('\'').unwrap_or(literal).to_owned();
  }
  let Some(basic) = text.strip_prefix('"') else { return text.to_owned() };
  let basic = basic.strip_suffix('"').unwrap_or(basic);
  let mut out = String::with_capacity(basic.len());
  let mut characters = basic.chars();
  while let Some(character) = characters.next() {
    if character != '\\' {
      out.push(character);
      continue;
    }
    let escaped = match characters.next() {
      Some('b') => '\u{8}',
      Some('t') => '\t',
      Some('n') => '\n',
      Some('f') => '\u{c}',
      Some('r') => '\r',
      Some(count @ ('u' | 'U')) => {
        let digits: String = characters.by_ref().take(if count == 'u' { 4 } else { 8 }).collect();
        u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32).unwrap_or_default()
      }
      Some(other) => other,
      None => break,
    };
    out.push(escaped);
  }
  out
}

/// `format_rows`: rows of items, separated, and padded so that the items in `align` line up.
fn format_rows(
  align: std::ops::Range<usize>,
  separator_range: std::ops::Range<usize>,
  rows: &[Vec<String>],
  newline: &str,
  separator: &str,
) -> String {
  let mut out = String::new();
  let can_align = rows.iter().flatten().all(|item| !item.contains('\n'));
  let width = |items: &[String]| items.iter().map(|item| item.chars().count()).sum::<usize>();
  let diff = |range: std::ops::Range<usize>, row: &[String]| {
    let max = rows
      .iter()
      .map(|other| {
        let start = range.start.min(other.len().saturating_sub(1));
        let end = range.end.min(other.len());
        width(other.get(start..end).unwrap_or_default())
      })
      .max()
      .unwrap_or(0);
    max - width(&row[range])
  };
  for (row_index, row) in rows.iter().enumerate() {
    if row_index != 0 {
      out.push_str(newline);
    }
    let mut last = 0;
    for (index, item) in row.iter().enumerate() {
      if index > separator_range.start
        && index <= separator_range.end.saturating_add(1)
        && index < row.len()
      {
        out.push_str(separator);
      }
      out.push_str(item);
      if can_align && align.start <= index && align.end > index && index < row.len() - 1 {
        let padding = diff(last..index + 1, row);
        out.extend(std::iter::repeat_n(' ', padding));
        last = index + 1;
      }
    }
  }
  out
}
