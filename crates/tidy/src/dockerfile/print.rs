//! dprint-plugin-dockerfile's generator (`src/generation`), with the parts of dprint-core's printer that it uses:
//! indentation that is written with the first text of a line, `ExpectNewLine`, and `gen_separated_values`, whose values
//! are all on one line when they fit and each on its own line when they don't.

use std::borrow::Cow;

use rustc_hash::FxHashSet;
use unicode_width::UnicodeWidthStr;

use super::Options;
use super::ast::*;

/// What the printer writes: dprint-core's signals that the plugin uses.
enum Item<'a> {
  /// Text without line breaks or tabs.
  Text(Cow<'a, str>),
  Tab,
  NewLine,
  /// A line break before the next text on this line, as after a comment.
  ExpectNewLine,
  StartIgnoringIndent,
  FinishIgnoringIndent,
  StartIndent(usize),
  FinishIndent(usize),
  Group(Group<'a>),
}

/// `gen_separated_values`: values on one line with a space between them, or each on its own line, with a line
/// continuation at the end of each line.
struct Group<'a> {
  values: Vec<Value<'a>>,
  /// Each on its own line however wide they are.
  force: bool,
  /// How a value after the first is indented on its own line.
  indent: Indent,
  continuation: &'static str,
}

struct Value<'a> {
  items: Vec<Item<'a>>,
  /// Ends with the line continuation when the values are each on their own line.
  ends: bool,
}

#[derive(Clone, Copy)]
enum Indent {
  /// Indent levels, which also indent the lines inside the value.
  Levels(usize),
  /// Spaces before the value only.
  Spaces(usize),
}

/// Prints the file. The text is the whole file, after a byte order mark.
pub(crate) fn print(file: &Dockerfile, options: &Options, out: &mut String) {
  let mut generator = Generator::new(file, options);
  let items = generator.file();
  let mut printer = Printer {
    out,
    eol: options.eol.text(),
    width: options.width,
    column: 0,
    indent: 0,
    ignoring: 0,
    expect_newline: false,
  };
  printer.items(&items);
}

/// What can be on a line of its own at the top, or among the values of an instruction: a node, or a comment.
enum Entry<'a, T> {
  Node(T),
  Comment(SpannedComment<'a>),
}

impl<T: Spanned> Entry<'_, T> {
  fn span(&self) -> Span {
    match self {
      Entry::Node(node) => node.span(),
      Entry::Comment(comment) => comment.span,
    }
  }
}

trait Spanned {
  fn span(&self) -> Span;
}

impl<T: Spanned> Spanned for &T {
  fn span(&self) -> Span {
    (*self).span()
  }
}

impl Spanned for Instruction<'_> {
  fn span(&self) -> Span {
    Instruction::span(self)
  }
}

impl Spanned for EnvVar<'_> {
  fn span(&self) -> Span {
    self.span
  }
}

impl Spanned for Label<'_> {
  fn span(&self) -> Span {
    self.span
  }
}

/// A value of `COPY`: a flag, or a path.
enum CopyValue<'a> {
  Flag(&'a Flag<'a>),
  Path(&'a SpannedString<'a>),
}

impl Spanned for CopyValue<'_> {
  fn span(&self) -> Span {
    match self {
      CopyValue::Flag(flag) => flag.span,
      CopyValue::Path(path) => path.span,
    }
  }
}

/// What a breakable string is part of, which decides how it prints.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
  /// The value of `ENV`, which is quoted when it has a space.
  EnvValue,
  /// The shell form of `RUN`, `CMD` and `ENTRYPOINT`, whose runs of whitespace collapse.
  Command,
  Other,
}

struct Generator<'a> {
  file: &'a Dockerfile<'a>,
  text: &'a str,
  options: &'a Options,
  handled: FxHashSet<usize>,
  /// Print the content of strings, rather than their text: in the quoted value of `ENV`.
  content: bool,
  /// Collapse runs of whitespace outside quotes: in shell commands.
  collapse: bool,
  /// The quote that is open while collapsing, from one line of a string to the next.
  quote: Option<char>,
  /// The whitespace to remove from the start of an instruction's continuation lines, as stages are indented.
  dedent: usize,
}

impl<'a> Generator<'a> {
  fn new(file: &'a Dockerfile<'a>, options: &'a Options) -> Self {
    Generator {
      file,
      text: file.content,
      options,
      handled: FxHashSet::default(),
      content: false,
      collapse: false,
      quote: None,
      dedent: 0,
    }
  }

  fn file(&mut self) -> Vec<Item<'a>> {
    let entries = with_comments(self.text, 0, self.text.len(), true, self.file.instructions.iter());
    let stage_body = self.stage_bodies(&entries);
    let stage_indent = self.options.indent_width.min(200);
    let mut items = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
      self.dedent = match self.options.indent_stages {
        true => self.line_indent(entry.span().start),
        false => 0,
      };
      let node = match entry {
        Entry::Node(instruction) => self.instruction(instruction),
        Entry::Comment(comment) => self.comment(comment),
      };
      // Comments that an instruction's grammar skips go before it, so that none is lost.
      let mut entry_items = self.dropped_comments(entry);
      entry_items.extend(node);
      if stage_body[index] {
        items.push(Item::StartIndent(stage_indent));
        items.extend(entry_items);
        items.push(Item::FinishIndent(stage_indent));
      } else {
        items.extend(entry_items);
      }
      if let Some(next) = entries.get(index + 1) {
        items.push(Item::NewLine);
        if self.has_blank_line(entry.span(), next.span()) {
          items.push(Item::NewLine);
        }
      }
    }
    if !entries.is_empty() {
      items.push(Item::NewLine);
    }
    items
  }

  /// Which entries are in the body of a stage, which `indentStages` indents: everything after a `FROM`, but the
  /// comments right above the next `FROM`.
  fn stage_bodies(&self, entries: &[Entry<&Instruction>]) -> Vec<bool> {
    let mut body = vec![false; entries.len()];
    if !self.options.indent_stages {
      return body;
    }
    let is_from = |entry: &Entry<&Instruction>| matches!(entry, Entry::Node(Instruction::From(_)));
    let mut in_stage = false;
    for (index, entry) in entries.iter().enumerate() {
      match is_from(entry) {
        true => in_stage = true,
        false => body[index] = in_stage,
      }
    }
    for index in 0..entries.len() {
      if !is_from(&entries[index]) {
        continue;
      }
      for before in (0..index).rev() {
        let comment = matches!(entries[before], Entry::Comment(_));
        if !comment || self.has_blank_line(entries[before].span(), entries[before + 1].span()) {
          break;
        }
        body[before] = false;
      }
    }
    body
  }

  fn has_blank_line(&self, before: Span, after: Span) -> bool {
    memchr::memchr_iter(b'\n', &self.text.as_bytes()[before.end..after.start]).nth(1).is_some()
  }

  /// The width of the indent of the line that `at` is on.
  fn line_indent(&self, at: usize) -> usize {
    let start = self.file.line_start(self.file.line_index(at));
    self.text.as_bytes()[start..at].iter().take_while(|byte| matches!(byte, b' ' | b'\t')).count()
  }

  /// `recover_dropped_comments`: the comments in an instruction that it didn't print. Not those in heredocs.
  fn dropped_comments(&mut self, entry: &Entry<&Instruction>) -> Vec<Item<'a>> {
    let mut items = Vec::new();
    let Entry::Node(instruction) = entry else { return items };
    let span = match instruction {
      Instruction::Heredoc(heredoc) => heredoc.instruction.span(),
      _ => instruction.span(),
    };
    for comment in comments(&self.text[span.start..span.end], span.start) {
      if !self.handled.contains(&comment.span.start) {
        items.extend(self.comment(&comment));
      }
    }
    items
  }

  fn instruction(&mut self, instruction: &'a Instruction<'a>) -> Vec<Item<'a>> {
    let mut items = Vec::new();
    match instruction {
      Instruction::Arg(arg) => {
        items.push(text("ARG "));
        items.extend(self.string(&arg.name, true));
        if let Some(value) = &arg.value {
          items.push(text("="));
          items.extend(self.string(value, true));
        }
      }
      Instruction::Cmd(cmd) => items.extend(self.expr("CMD ", &cmd.expr, Role::Command)),
      Instruction::Run(run) => items.extend(self.expr("RUN ", &run.expr, Role::Command)),
      Instruction::Entrypoint(entrypoint) => {
        items.extend(self.expr("ENTRYPOINT ", &entrypoint.expr, Role::Command))
      }
      Instruction::Shell(shell) => items.extend(self.expr("SHELL ", &shell.expr, Role::Other)),
      Instruction::Copy(copy) => items.extend(self.copy(copy)),
      Instruction::Env(env) => {
        items.push(text("ENV "));
        let entries =
          with_comments(self.text, env.span.start, env.span.end, false, env.vars.iter());
        items.extend(self.values(entries, 4, |generator, var: &&'a EnvVar<'a>| {
          let mut items = generator.string(&var.key, true);
          items.push(text("="));
          items.extend(generator.breakable(&var.value, Role::EnvValue));
          items
        }));
      }
      Instruction::Label(label) => {
        items.push(text("LABEL "));
        let entries =
          with_comments(self.text, label.span.start, label.span.end, false, label.labels.iter());
        items.extend(self.values(entries, 6, |generator, label: &&'a Label<'a>| {
          let mut items = generator.string(&label.name, true);
          items.push(text("="));
          items.extend(generator.string(&label.value, true));
          items
        }));
      }
      Instruction::From(from) => {
        items.push(text("FROM "));
        for flag in &from.flags {
          items.extend(self.flag(flag));
          items.push(text(" "));
        }
        items.extend(self.string(&from.image, true));
        if let Some(alias) = &from.alias {
          items.push(text(" AS "));
          items.extend(self.string(alias, true));
        }
      }
      Instruction::Misc(misc) => {
        items.extend(self.string(&misc.instruction, true));
        items.push(text(" "));
        items.extend(self.breakable(&misc.arguments, Role::Other));
      }
      Instruction::Onbuild(onbuild) => {
        items.push(text("ONBUILD "));
        items.extend(self.instruction(&onbuild.instruction));
      }
      Instruction::Healthcheck(healthcheck) => items.extend(self.healthcheck(healthcheck)),
      Instruction::Heredoc(heredoc) => {
        items.extend(self.instruction(&heredoc.instruction));
        // The bodies and their closing delimiters stay as they are: a delimiter only closes when it matches exactly.
        items.push(Item::NewLine);
        items.push(Item::StartIgnoringIndent);
        raw(heredoc.body, &mut items);
        items.push(Item::FinishIgnoringIndent);
      }
      Instruction::Unknown(line) => {
        // It is kept as it is written, with the comment lines that it continues over.
        for comment in comments(&line.content, line.span.start) {
          self.handled.insert(comment.span.start);
        }
        // Its first line takes the indent of an instruction, and the lines it continues to stay as they are.
        let text = self.span_text(line.span).trim();
        match text.split_once('\n') {
          None => raw(text, &mut items),
          Some((first, rest)) => {
            raw(first.strip_suffix('\r').unwrap_or(first), &mut items);
            items.push(Item::NewLine);
            items.push(Item::StartIgnoringIndent);
            raw(rest, &mut items);
            items.push(Item::FinishIgnoringIndent);
          }
        }
      }
    }
    items
  }

  fn expr(&mut self, keyword: &'static str, expr: &'a Expr<'a>, role: Role) -> Vec<Item<'a>> {
    let mut items = vec![text(keyword)];
    match expr {
      Expr::Exec(array) => items.extend(self.array(array)),
      Expr::Shell(breakable) => items.extend(self.breakable(breakable, role)),
    }
    items
  }

  fn copy(&mut self, copy: &'a CopyInstruction<'a>) -> Vec<Item<'a>> {
    let mut items = vec![text("COPY ")];
    match &copy.args {
      CopyArgs::Exec(array) => {
        for flag in &copy.flags {
          items.extend(self.flag(flag));
          items.push(text(" "));
        }
        items.extend(self.array(array));
      }
      CopyArgs::Paths { sources, destination } => {
        let values = (copy.flags.iter().map(CopyValue::Flag))
          .chain(sources.iter().map(CopyValue::Path))
          .chain([CopyValue::Path(destination)]);
        let entries = with_comments(self.text, copy.span.start, copy.span.end, false, values);
        let print = |generator: &mut Self, value: &CopyValue<'a>| match value {
          CopyValue::Flag(flag) => generator.flag(flag),
          CopyValue::Path(path) => generator.string(path, true),
        };
        match entries.iter().any(|entry| matches!(entry, Entry::Comment(_))) {
          // Comments keep the values on their own lines, aligned.
          true => items.extend(self.values(entries, 5, print)),
          false => {
            for (index, entry) in entries.iter().enumerate() {
              if index > 0 {
                items.push(text(" "));
              }
              if let Entry::Node(value) = entry {
                items.extend(print(self, value));
              }
            }
          }
        }
      }
    }
    items
  }

  fn healthcheck(&mut self, healthcheck: &'a HealthcheckInstruction<'a>) -> Vec<Item<'a>> {
    let mut items = vec![text("HEALTHCHECK")];
    let mut flags = Vec::new();
    for flag in &healthcheck.flags {
      flags.push(text(" --"));
      flags.extend(self.string(&flag.name, true));
      flags.push(text("="));
      flags.extend(self.string(&flag.value, true));
    }
    let command = match &healthcheck.cmd {
      Some(cmd) => self.instruction(cmd),
      None => vec![text("NONE")],
    };
    // A line break that the author wrote keeps the command on its own line, as `healthcheckCmdNewLine` does.
    let first = self.file.line_index(healthcheck.span.start);
    let span = healthcheck.cmd.as_ref().map_or(healthcheck.span, |cmd| cmd.span());
    let proactive = self.options.healthcheck_cmd_new_line
      && healthcheck.cmd.is_some()
      && !healthcheck.flags.is_empty();
    let force = self.file.line_index(span.start) > first || proactive;
    items.push(Item::Group(Group {
      values: vec![Value { items: flags, ends: true }, Value { items: command, ends: false }],
      force,
      indent: Indent::Spaces(2),
      continuation: self.space_continuation(),
    }));
    items
  }

  /// `gen_multi_line_items`: the values of `ENV`, `LABEL` and `COPY`, aligned after the keyword when they are on lines
  /// of their own.
  fn values<T: Spanned>(
    &mut self,
    entries: Vec<Entry<'a, T>>,
    indent: usize,
    print: impl Fn(&mut Self, &T) -> Vec<Item<'a>>,
  ) -> Vec<Item<'a>> {
    let force = entries.len() > 1
      && (self.file.line_index(entries[0].span().start)
        < self.file.line_index(entries[1].span().start)
        || entries.iter().any(|entry| matches!(entry, Entry::Comment(_))));
    let count = entries.len();
    let values = entries
      .into_iter()
      .enumerate()
      .map(|(index, entry)| match entry {
        Entry::Node(node) => Value { items: print(self, &node), ends: index + 1 < count },
        Entry::Comment(comment) => Value { items: self.comment(&comment), ends: false },
      })
      .collect();
    let continuation = self.space_continuation();
    vec![Item::Group(Group { values, force, indent: Indent::Levels(indent), continuation })]
  }

  fn flag(&mut self, flag: &'a Flag<'a>) -> Vec<Item<'a>> {
    let mut items = vec![text("--")];
    items.extend(self.string(&flag.name, true));
    items.push(text("="));
    items.extend(self.string(&flag.value, true));
    items
  }

  fn array(&mut self, array: &'a StringArray<'a>) -> Vec<Item<'a>> {
    let mut items = vec![text("[")];
    for (index, element) in array.elements.iter().enumerate() {
      if index > 0 {
        items.push(text(", "));
      }
      items.extend(self.string(element, true));
    }
    items.push(text("]"));
    items
  }

  /// `gen_breakable_string`
  fn breakable(&mut self, breakable: &'a BreakableString<'a>, role: Role) -> Vec<Item<'a>> {
    let mut items = Vec::new();
    let source = self.span_text(breakable.span);
    // A quoted value stays as it is written: quoting its content again would lose its escapes, and a value over lines
    // that tidy quoted reads back the same.
    if role == Role::EnvValue && starts_quoted(source) {
      for comment in comments(source, breakable.span.start) {
        self.handled.insert(comment.span.start);
      }
      raw(source, &mut items);
      return items;
    }
    let quoted = role == Role::EnvValue && source.contains([' ', '\t']);
    let (content, collapse, quote) = (self.content, self.collapse, self.quote);
    self.content = quoted;
    self.collapse = role == Role::Command && !quoted;
    self.quote = None;
    let continuation = self.continuation();
    if quoted {
      items.push(text("\""));
    }
    // A string that starts with a comment, as after `RUN \`, keeps the comment with the instruction.
    if matches!(breakable.components.first(), Some(Component::Comment(_))) {
      items.push(text(continuation));
      items.push(Item::NewLine);
    }
    let first_is_string = matches!(breakable.components.first(), Some(Component::String(_)));
    let mut ignoring = false;
    let last = breakable.components.len() - 1;
    for (index, component) in breakable.components.iter().enumerate() {
      // As `string` reads it: the first line is trimmed, and the others lose the indent that the stage moved, but in a
      // quote.
      let lead = index == 0 && first_is_string;
      let dedents = !lead && self.quote.is_none();
      match component {
        Component::Comment(comment) => {
          // A comment loses its indent when parsed: it takes that of the strings around it. Inside a quote, the lines
          // keep their indent, so the comment keeps it too.
          let indent = indent_around(&breakable.components, index, first_is_string);
          let indent = match self.quote {
            Some(_) => indent,
            None => dedent(indent, self.dedent),
          };
          raw(indent, &mut items);
          items.extend(self.comment(comment));
        }
        Component::String(string) => items.extend(self.string(string, lead)),
      }
      let in_quote = quoted || self.collapse && self.quote.is_some();
      if ignoring && !in_quote {
        items.push(Item::FinishIgnoringIndent);
        ignoring = false;
      }
      if index < last {
        if let Component::String(string) = component {
          let content =
            if dedents { dedent(&string.content, self.dedent) } else { &string.content };
          // A first line of only whitespace prints as nothing: a bare line continuation reads back the same.
          let blank_lead = lead && content.trim().is_empty();
          match !in_quote && content.ends_with(' ') && !blank_lead {
            true => items.push(text(self.space_continuation())),
            false => items.push(text(continuation)),
          }
        }
        // A line that a quote continues is content: an indent would change the value.
        if in_quote && !ignoring {
          items.push(Item::StartIgnoringIndent);
          ignoring = true;
        }
        items.push(Item::NewLine);
      }
    }
    if ignoring {
      items.push(Item::FinishIgnoringIndent);
    }
    if quoted {
      items.push(text("\""));
    }
    (self.content, self.collapse, self.quote) = (content, collapse, quote);
    items
  }

  /// `gen_string`. Only the first line of a breakable string is trimmed at its start.
  fn string(&mut self, string: &'a SpannedString<'a>, trim: bool) -> Vec<Item<'a>> {
    let mut items = Vec::new();
    if self.content {
      raw(&string.content, &mut items);
      return items;
    }
    let source = self.span_text(string.span);
    // A continuation line loses the indent of its instruction, which the printer adds back where it now is.
    let source = match trim || self.quote.is_some() {
      true => source,
      false => dedent(source, self.dedent),
    };
    if self.collapse {
      let collapsed = collapse(source, trim, &mut self.quote);
      // Whitespace at the end separates, outside a quote; inside one it is content.
      let text = match self.quote {
        None => collapsed.trim_end(),
        Some(_) => &collapsed,
      };
      raw_owned(text, &mut items);
    } else {
      let text = if trim { source.trim() } else { source.trim_end() };
      raw(text, &mut items);
    }
    items
  }

  /// `gen_comment`: once, then a line break.
  fn comment(&mut self, comment: &SpannedComment<'a>) -> Vec<Item<'a>> {
    let mut items = Vec::new();
    if !self.handled.insert(comment.span.start) {
      return items;
    }
    let content = comment.content;
    let start = content.find(|c| c != '#').unwrap_or(content.len());
    let body = content[start..].trim();
    let text = match body.is_empty() {
      true => content[..start].to_owned(),
      false => format!("{} {body}", &content[..start]),
    };
    raw_owned(&text, &mut items);
    items.push(Item::ExpectNewLine);
    items
  }

  fn span_text(&self, span: Span) -> &'a str {
    &self.text[span.start..span.end]
  }

  fn continuation(&self) -> &'static str {
    if self.file.escape == '`' { "`" } else { "\\" }
  }

  fn space_continuation(&self) -> &'static str {
    if self.file.escape == '`' { " `" } else { " \\" }
  }
}

/// `nodes_with_comments`: the nodes, with the comments in the text between them. Without `leading`, the comments
/// before the first node are left out, so that they don't go on the instruction's first line.
fn with_comments<'a, T: Spanned>(
  text: &'a str,
  start: usize,
  end: usize,
  leading: bool,
  nodes: impl Iterator<Item = T>,
) -> Vec<Entry<'a, T>> {
  let mut entries = Vec::new();
  let mut last = start;
  for (index, node) in nodes.enumerate() {
    let span = node.span();
    if index > 0 || leading {
      entries.extend(comments(&text[last..span.start], last).into_iter().map(Entry::Comment));
    }
    entries.push(Entry::Node(node));
    last = span.end;
  }
  entries.extend(comments(&text[last..end], last).into_iter().map(Entry::Comment));
  entries
}

/// `parse_comments`: the comments that start lines of `text`, which is at `offset`, after whitespace.
fn comments(text: &str, offset: usize) -> Vec<SpannedComment<'_>> {
  let mut comments = Vec::new();
  let mut chars = text.char_indices();
  let mut line_start = true;
  while let Some((at, c)) = chars.next() {
    if line_start && c.is_whitespace() {
      continue;
    }
    if line_start && c == '#' {
      let mut end = at + 1;
      for (next, c) in chars.by_ref() {
        if c == '\n' {
          break;
        }
        end = next + c.len_utf8();
      }
      comments.push(SpannedComment {
        span: Span::new(offset + at, offset + end),
        content: &text[at..end],
      });
    } else {
      line_start = c == '\n';
    }
  }
  comments
}

/// The leading whitespace of the nearest string after the comment at `index`, or else before it. A string of only
/// whitespace, which prints as a bare line continuation, doesn't count, and the first line has none: its whitespace is
/// the gap after the keyword, which is trimmed. So the indent reads back the same.
fn indent_around<'c>(components: &'c [Component], index: usize, first_is_string: bool) -> &'c str {
  let indent = |(at, component): (usize, &'c Component)| match component {
    Component::String(_) if at == 0 && first_is_string => Some(""),
    Component::String(string) if !string.content.trim().is_empty() => {
      Some(&string.content[..string.content.len() - string.content.trim_start().len()])
    }
    _ => None,
  };
  let mut around = components.iter().enumerate();
  (around.clone().skip(index + 1).find_map(indent))
    .or_else(|| around.by_ref().take(index).collect::<Vec<_>>().into_iter().rev().find_map(indent))
    .unwrap_or("")
}

fn starts_quoted(text: &str) -> bool {
  matches!(text.as_bytes().first(), Some(b'"' | b'\'' | b'`'))
}

/// Up to `width` leading spaces or tabs removed.
fn dedent(text: &str, width: usize) -> &str {
  let count = text.bytes().take(width).take_while(|byte| matches!(byte, b' ' | b'\t')).count();
  &text[count..]
}

/// `collapse_shell_whitespace`: runs of whitespace as one space, but inside quotes and after a backslash. Leading
/// whitespace outside a quote is indent, which `drop_leading` removes.
fn collapse(text: &str, drop_leading: bool, quote: &mut Option<char>) -> String {
  let mut out = String::with_capacity(text.len());
  let mut chars = text.chars().peekable();
  if quote.is_none() {
    while let Some(c) = chars.next_if(|c| matches!(c, ' ' | '\t')) {
      if !drop_leading {
        out.push(c);
      }
    }
  }
  while let Some(c) = chars.next() {
    match *quote {
      Some('\'') => {
        out.push(c);
        if c == '\'' {
          *quote = None;
        }
      }
      Some(_) => {
        out.push(c);
        if c == '\\' {
          out.extend(chars.next());
        } else if c == '"' {
          *quote = None;
        }
      }
      None => match c {
        '\'' | '"' => {
          out.push(c);
          *quote = Some(c);
        }
        '\\' => {
          out.push(c);
          out.extend(chars.next());
        }
        ' ' | '\t' => {
          let mut count = 1;
          while chars.next_if(|c| matches!(c, ' ' | '\t')).is_some() {
            count += 1;
          }
          out.push(if count > 1 { ' ' } else { c });
        }
        _ => out.push(c),
      },
    }
  }
  out
}

fn text<'a>(text: &'static str) -> Item<'a> {
  Item::Text(Cow::Borrowed(text))
}

/// `gen_from_raw_string`: lines with their indent ignored, when there is more than one, and tabs as tabs.
fn raw<'a>(text: &'a str, items: &mut Vec<Item<'a>>) {
  split(text, items, |part| Item::Text(Cow::Borrowed(part)));
}

/// [`raw`] of a text that the items keep a copy of.
fn raw_owned<'a>(text: &str, items: &mut Vec<Item<'a>>) {
  split(text, items, |part| Item::Text(Cow::Owned(part.to_owned())));
}

fn split<'t, 'a>(text: &'t str, items: &mut Vec<Item<'a>>, part: impl Fn(&'t str) -> Item<'a>) {
  let lines = text.contains('\n');
  if lines {
    items.push(Item::StartIgnoringIndent);
  }
  for (index, line) in text.lines().enumerate() {
    if index > 0 {
      items.push(Item::NewLine);
    }
    for (index, text) in line.split('\t').enumerate() {
      if index > 0 {
        items.push(Item::Tab);
      }
      if !text.is_empty() {
        items.push(part(text));
      }
    }
  }
  if text.ends_with('\n') {
    items.push(Item::NewLine);
  }
  if lines {
    items.push(Item::FinishIgnoringIndent);
  }
}

/// dprint-core's printer and writer, for the items above: indent levels of one space.
struct Printer<'p> {
  out: &'p mut String,
  eol: &'static str,
  width: usize,
  column: usize,
  indent: usize,
  ignoring: usize,
  expect_newline: bool,
}

impl Printer<'_> {
  fn items(&mut self, items: &[Item]) {
    for item in items {
      match item {
        Item::Text(text) => self.text(text),
        Item::Tab => {
          self.first_column();
          self.out.push('\t');
          self.column += 1;
        }
        Item::NewLine => self.newline(),
        Item::ExpectNewLine => self.expect_newline = true,
        Item::StartIgnoringIndent => self.ignoring += 1,
        Item::FinishIgnoringIndent => self.ignoring -= 1,
        Item::StartIndent(width) => self.indent += width,
        Item::FinishIndent(width) => self.indent -= width,
        Item::Group(group) => self.group(group),
      }
    }
  }

  fn text(&mut self, text: &str) {
    self.first_column();
    self.out.push_str(text);
    self.column += text.width();
  }

  /// The line break that `ExpectNewLine` asks for, and the indent, before the first text of a line.
  fn first_column(&mut self) {
    if self.expect_newline {
      self.newline();
    }
    if self.column == 0 && self.indent > 0 && self.ignoring == 0 {
      self.out.extend(std::iter::repeat_n(' ', self.indent));
      self.column = self.indent;
    }
  }

  fn newline(&mut self) {
    self.out.push_str(self.eol);
    self.column = 0;
    self.expect_newline = false;
  }

  fn group(&mut self, group: &Group) {
    let lines = group.force || self.breaks(group);
    for (index, value) in group.values.iter().enumerate() {
      if index > 0 {
        match lines {
          true => self.newline(),
          false => self.text(" "),
        }
      }
      match group.indent {
        Indent::Levels(width) if index > 0 => {
          self.indent += width;
          self.items(&value.items);
          self.indent -= width;
        }
        Indent::Spaces(width) if index > 0 && lines => {
          self.text(&" ".repeat(width));
          self.items(&value.items);
        }
        _ => self.items(&value.items),
      }
      if lines && value.ends && index + 1 < group.values.len() {
        self.text(group.continuation);
      }
    }
  }

  /// The values go on lines of their own when one of them is over more than one line, or when a value after the first
  /// would end past the width on one line.
  fn breaks(&self, group: &Group) -> bool {
    let mut column = self.column;
    for (index, value) in group.values.iter().enumerate() {
      let Some(width) = single_line_width(&value.items) else { return true };
      if index > 0 {
        column += 1 + width;
        if column > self.width {
          return true;
        }
      } else {
        column += width;
      }
    }
    false
  }
}

/// The width of items on one line, or `None` if they break the line.
fn single_line_width(items: &[Item]) -> Option<usize> {
  let mut width = 0;
  for item in items {
    match item {
      Item::Text(text) => width += text.width(),
      Item::Tab => width += 1,
      Item::NewLine | Item::ExpectNewLine | Item::Group(_) => return None,
      _ => {}
    }
  }
  Some(width)
}
