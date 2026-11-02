//! dprint-plugin-dockerfile's parser, which follows the grammar of the `dockerfile-parser` crate. A line that doesn't
//! parse as its instruction is kept as it is, so parsing never fails. Spans come from where each slice of the text
//! starts.

use std::borrow::Cow;

use super::ast::*;

/// The parse failed here, and the caller tries something else.
struct Fail;

type Parsed<'a, T> = Result<(&'a str, T), Fail>;

pub(crate) fn parse(text: &str) -> Dockerfile<'_> {
  let escape = escape(text);
  let parser = Parser { base: text, escape };
  Dockerfile::new(text, parser.dockerfile(text), escape)
}

/// The escape character of the `# escape=` parser directive at the top, or `\`. The directives are the `# name=value`
/// lines that start the file.
fn escape(text: &str) -> char {
  for line in text.lines() {
    let Some(directive) = line.trim().strip_prefix('#') else { break };
    let Some((name, value)) = directive.split_once('=') else { break };
    match name.trim().to_ascii_lowercase().as_str() {
      "escape" => return if value.trim() == "`" { '`' } else { '\\' },
      "syntax" => continue,
      _ => break,
    }
  }
  '\\'
}

/// The instructions with a grammar of their own; any other is a [`MiscInstruction`].
const KEYWORDS: [&str; 11] = [
  "from",
  "run",
  "arg",
  "label",
  "copy",
  "entrypoint",
  "cmd",
  "env",
  "shell",
  "onbuild",
  "healthcheck",
];

struct Parser<'a> {
  base: &'a str,
  escape: char,
}

#[derive(Clone, Copy)]
enum ExprKind {
  Run,
  Cmd,
  Entrypoint,
  Shell,
}

impl<'a> Parser<'a> {
  fn dockerfile(&self, mut input: &'a str) -> Vec<Instruction<'a>> {
    let mut instructions = Vec::new();
    loop {
      let after_ws = skip_ws(input);
      if after_ws.is_empty() {
        break;
      }
      if let Some(rest) = strip_newline(after_ws) {
        input = rest;
        continue;
      }
      // The printer finds comment lines again in the gaps between instructions.
      if after_ws.starts_with('#') {
        input = skip_to_next_line(after_ws);
        continue;
      }
      let parsed = self.instruction(after_ws).and_then(|(rest, instruction)| {
        let (rest, instruction) = self.heredocs(instruction, rest);
        Ok((self.finish_line(rest)?, instruction))
      });
      match parsed {
        Ok((next, instruction)) => {
          instructions.push(instruction);
          input = next;
        }
        Err(Fail) => {
          let (rest, line) = self.unknown_line(after_ws);
          instructions.push(Instruction::Unknown(line));
          input = rest;
        }
      }
    }
    instructions
  }

  fn instruction(&self, input: &'a str) -> Parsed<'a, Instruction<'a>> {
    let (after_keyword, keyword) = alpha(input);
    let lower = keyword.to_ascii_lowercase();
    // A keyword only counts with whitespace after it; else the line is a misc instruction.
    if KEYWORDS.contains(&lower.as_str())
      && let Some(after_ws) = self.arg_ws(after_keyword)
    {
      let start = self.off(input);
      // The shell forms keep a comment after the keyword's line continuation.
      let shell = self.arg_ws_keeping_comments(after_keyword).unwrap_or(after_ws);
      return match lower.as_str() {
        "from" => self.from(after_ws, start),
        "run" => self.shell_or_exec(shell, start, ExprKind::Run),
        "cmd" => self.shell_or_exec(shell, start, ExprKind::Cmd),
        "entrypoint" => self.shell_or_exec(shell, start, ExprKind::Entrypoint),
        "arg" => self.arg(after_ws, start),
        "label" => self.label(after_keyword, start),
        "copy" => self.copy(after_keyword, start),
        "env" => self.env(after_keyword, start),
        "shell" => self.shell_or_exec(shell, start, ExprKind::Shell),
        "onbuild" => self.onbuild(after_ws, start).or_else(|_| self.misc(input)),
        _ => self.healthcheck(after_ws, start).or_else(|_| self.misc(input)),
      };
    }
    self.misc(input)
  }

  fn from(&self, mut input: &'a str, start: usize) -> Parsed<'a, Instruction<'a>> {
    let mut flags = Vec::new();
    while let Some((rest, flag)) = self.flag(input, |c| c.is_ascii_alphabetic()) {
      flags.push(flag);
      match self.arg_ws(rest) {
        Some(next) => input = next,
        None => {
          input = rest;
          break;
        }
      }
    }
    let image = take_while(input, |c| {
      c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '/' | '$' | '{' | '}' | '@')
    });
    let (after_image, image) = match image {
      (_, "") => return Err(Fail),
      (rest, text) => (rest, self.spanned(input, rest, text)),
    };
    let (end, alias) = match self.alias(after_image) {
      Some((rest, alias)) => (rest, Some(alias)),
      None => (after_image, None),
    };
    let span = Span::new(start, self.off(end));
    Ok((end, Instruction::From(FromInstruction { span, flags, image, alias })))
  }

  /// ` AS name` after the image.
  fn alias(&self, after_image: &'a str) -> Option<(&'a str, SpannedString<'a>)> {
    let after_ws = self.arg_ws(after_image)?;
    let after_as = strip_prefix_ci(after_ws, "as")?;
    let start = self.arg_ws(after_as)?;
    match take_while(start, |c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')) {
      (_, "") => None,
      (rest, text) => Some((rest, self.spanned(start, rest, text))),
    }
  }

  fn shell_or_exec(
    &self,
    input: &'a str,
    start: usize,
    kind: ExprKind,
  ) -> Parsed<'a, Instruction<'a>> {
    let exec = match input.starts_with('[') {
      true => self.string_array(input).ok(),
      false => None,
    };
    let (rest, expr) = match exec {
      Some((rest, array)) => (rest, Expr::Exec(array)),
      None => {
        let (rest, breakable) = self.breakable(input)?;
        (rest, Expr::Shell(breakable))
      }
    };
    let end = match &expr {
      Expr::Exec(array) => array.span.end,
      Expr::Shell(breakable) => breakable.span.end,
    };
    let instruction = ExprInstruction { span: Span::new(start, end), expr };
    Ok((
      rest,
      match kind {
        ExprKind::Run => Instruction::Run(instruction),
        ExprKind::Cmd => Instruction::Cmd(instruction),
        ExprKind::Entrypoint => Instruction::Entrypoint(instruction),
        ExprKind::Shell => Instruction::Shell(instruction),
      },
    ))
  }

  fn arg(&self, input: &'a str, start: usize) -> Parsed<'a, Instruction<'a>> {
    let first = input.chars().next().filter(char::is_ascii_alphabetic).ok_or(Fail)?;
    let (after_name, _) =
      take_while(&input[first.len_utf8()..], |c| c.is_ascii_alphanumeric() || c == '_');
    let name = self.spanned(input, after_name, &input[..input.len() - after_name.len()]);
    let (end, value) = match after_name.strip_prefix('=') {
      Some(after_eq) => {
        let (rest, value) = match starts_with_quote(after_eq) {
          true => self.quoted(after_eq)?,
          false => {
            let (rest, text) = self.any_whitespace(after_eq)?;
            (rest, self.spanned(after_eq, rest, text))
          }
        };
        (rest, Some(value))
      }
      None => (after_name, None),
    };
    let span = Span::new(start, self.off(end));
    Ok((end, Instruction::Arg(ArgInstruction { span, name, value })))
  }

  fn copy(&self, mut input: &'a str, start: usize) -> Parsed<'a, Instruction<'a>> {
    let mut flags = Vec::new();
    while let Some(after_ws) = self.arg_ws(input) {
      match self.flag(after_ws, |c| c.is_ascii_alphabetic()) {
        Some((rest, flag)) => {
          flags.push(flag);
          input = rest;
        }
        None => break,
      }
    }
    if let Some(after_ws) = self.arg_ws(input)
      && after_ws.starts_with('[')
      && let Ok((rest, array)) = self.string_array(after_ws)
    {
      let span = Span::new(start, array.span.end);
      return Ok((
        rest,
        Instruction::Copy(CopyInstruction { span, flags, args: CopyArgs::Exec(array) }),
      ));
    }
    let mut paths = Vec::new();
    while let Some(after_ws) = self.arg_ws(input) {
      match self.any_whitespace(after_ws) {
        Ok((rest, text)) => {
          paths.push(self.spanned(after_ws, rest, text));
          input = rest;
        }
        Err(Fail) => break,
      }
    }
    if paths.len() < 2 {
      return Err(Fail);
    }
    let destination = paths.pop().expect("there are two paths");
    let span = Span::new(start, destination.span.end);
    let args = CopyArgs::Paths { sources: paths, destination };
    Ok((input, Instruction::Copy(CopyInstruction { span, flags, args })))
  }

  fn label(&self, after_keyword: &'a str, start: usize) -> Parsed<'a, Instruction<'a>> {
    let mut labels = Vec::new();
    let input = match self.label_single(after_keyword) {
      Some((rest, label)) => {
        labels.push(label);
        rest
      }
      None => {
        let mut current = after_keyword;
        while let Some(after_ws) = self.arg_ws(current) {
          match self.label_pair(after_ws) {
            Some((rest, label)) => {
              labels.push(label);
              current = rest;
            }
            None => {
              current = after_ws;
              break;
            }
          }
        }
        current
      }
    };
    let end = labels.last().ok_or(Fail)?.span.end;
    Ok((input, Instruction::Label(LabelInstruction { span: Span::new(start, end), labels })))
  }

  /// `LABEL name value`, whose span starts with the whitespace before the name.
  fn label_single(&self, after_keyword: &'a str) -> Option<(&'a str, Label<'a>)> {
    let input = self.arg_ws(after_keyword)?;
    let (after_name, name) = self.label_name(input)?;
    let after_ws = self.arg_ws(after_name)?;
    let (rest, value) = self.label_value(after_ws)?;
    let span = Span::new(self.off(after_keyword), value.span.end);
    Some((rest, Label { span, name, value }))
  }

  fn label_pair(&self, input: &'a str) -> Option<(&'a str, Label<'a>)> {
    let (after_name, name) = self.label_name(input)?;
    let (rest, value) = self.label_value(after_name.strip_prefix('=')?)?;
    let span = Span::new(name.span.start, value.span.end);
    Some((rest, Label { span, name, value }))
  }

  fn label_name(&self, input: &'a str) -> Option<(&'a str, SpannedString<'a>)> {
    if starts_with_quote(input) {
      return self.quoted(input).ok();
    }
    match take_while(input, |c| !is_ws(c) && !is_newline(c) && c != '=') {
      (_, "") => None,
      (rest, text) => Some((rest, self.spanned(input, rest, text))),
    }
  }

  fn label_value(&self, input: &'a str) -> Option<(&'a str, SpannedString<'a>)> {
    if starts_with_quote(input) {
      return self.quoted(input).ok();
    }
    let (rest, text) = self.any_whitespace(input).ok()?;
    Some((rest, self.spanned(input, rest, text)))
  }

  fn env(&self, after_keyword: &'a str, start: usize) -> Parsed<'a, Instruction<'a>> {
    if let Some((rest, var)) = self.env_single(after_keyword) {
      let span = Span::new(start, var.span.end);
      return Ok((rest, Instruction::Env(EnvInstruction { span, vars: vec![var] })));
    }
    let mut vars = Vec::new();
    let mut input = after_keyword;
    while let Some(after_ws) = self.arg_ws(input) {
      match self.env_pair(after_ws) {
        Some((rest, var)) => {
          vars.push(var);
          input = rest;
        }
        None => {
          input = after_ws;
          break;
        }
      }
    }
    let end = vars.last().ok_or(Fail)?.span.end;
    Ok((input, Instruction::Env(EnvInstruction { span: Span::new(start, end), vars })))
  }

  /// `ENV name value`, where the value can go over lines.
  fn env_single(&self, after_keyword: &'a str) -> Option<(&'a str, EnvVar<'a>)> {
    let input = self.arg_ws(after_keyword)?;
    let (after_name, key) = self.env_name(input)?;
    let after_ws = self.arg_ws(after_name)?;
    let (rest, value) = match starts_with_quote(after_ws) {
      true => {
        self.quoted(after_ws).map(|(rest, string)| (rest, BreakableString::of(string))).ok()?
      }
      false => self.breakable(after_ws).ok()?,
    };
    let span = Span::new(key.span.start, value.span.end);
    Some((rest, EnvVar { span, key, value }))
  }

  fn env_pair(&self, input: &'a str) -> Option<(&'a str, EnvVar<'a>)> {
    let (after_name, key) = self.env_name(input)?;
    let after_eq = after_name.strip_prefix('=')?;
    let (rest, value) = match starts_with_quote(after_eq) {
      true => self.quoted(after_eq).ok()?,
      false => self.env_value(after_eq)?,
    };
    let value = BreakableString::of(value);
    let span = Span::new(key.span.start, value.span.end);
    Some((rest, EnvVar { span, key, value }))
  }

  /// An unquoted value, where the escape character keeps the character after it, such as a space. Its content has
  /// the escapes read, so that the printer can quote a value with spaces.
  fn env_value(&self, input: &'a str) -> Option<(&'a str, SpannedString<'a>)> {
    let mut end = input.len();
    let mut chars = input.char_indices();
    while let Some((at, c)) = chars.next() {
      if c == self.escape {
        if line_continuation(&input[at..], self.escape).is_some() {
          end = at;
          break;
        }
        chars.next();
        continue;
      }
      if is_ws(c) || is_newline(c) {
        end = at;
        break;
      }
    }
    if end == 0 {
      return None;
    }
    let rest = &input[end..];
    let content = unescape(&input[..end], self.escape);
    Some((rest, SpannedString { span: self.span(input, rest), content }))
  }

  fn env_name(&self, input: &'a str) -> Option<(&'a str, SpannedString<'a>)> {
    match take_while(input, |c| c.is_ascii_alphanumeric() || c == '_') {
      (_, "") => None,
      (rest, text) => Some((rest, self.spanned(input, rest, text))),
    }
  }

  fn onbuild(&self, input: &'a str, start: usize) -> Parsed<'a, Instruction<'a>> {
    let (rest, inner) = self.instruction(input)?;
    let span = Span::new(start, inner.span().end);
    Ok((rest, Instruction::Onbuild(OnbuildInstruction { span, instruction: Box::new(inner) })))
  }

  fn healthcheck(&self, mut input: &'a str, start: usize) -> Parsed<'a, Instruction<'a>> {
    let mut flags = Vec::new();
    while let Some((rest, flag)) = self.flag(input, |c| c.is_ascii_alphanumeric() || c == '-') {
      flags.push(flag);
      match self.arg_ws(rest) {
        Some(next) => input = next,
        None => {
          input = rest;
          break;
        }
      }
    }
    if let Some(after) = strip_prefix_ci(input, "none")
      && (after.is_empty() || after.starts_with(is_ws) || starts_with_newline(after))
    {
      let span = Span::new(start, self.off(after));
      return Ok((
        after,
        Instruction::Healthcheck(HealthcheckInstruction { span, flags, cmd: None }),
      ));
    }
    let (rest, inner) = self.instruction(input)?;
    let span = Span::new(start, inner.span().end);
    let cmd = Some(Box::new(inner));
    Ok((rest, Instruction::Healthcheck(HealthcheckInstruction { span, flags, cmd })))
  }

  fn misc(&self, input: &'a str) -> Parsed<'a, Instruction<'a>> {
    let start = self.off(input);
    let (after_keyword, keyword) = alpha(input);
    // A keyword glued to what follows, as in `RUN-x`, isn't an instruction: a space after it would make it one.
    let glued = !after_keyword.is_empty()
      && !after_keyword.starts_with([' ', '\t'])
      && !starts_with_newline(after_keyword)
      && line_continuation(after_keyword, self.escape).is_none();
    if keyword.is_empty() || glued {
      return Err(Fail);
    }
    let instruction = self.spanned(input, after_keyword, keyword);
    let (rest, arguments) = self.breakable(after_keyword)?;
    let span = Span::new(start, arguments.span.end);
    Ok((rest, Instruction::Misc(MiscInstruction { span, instruction, arguments })))
  }

  /// `--name=value`, with the characters that the name can have.
  fn flag(&self, input: &'a str, name: impl Fn(char) -> bool) -> Option<(&'a str, Flag<'a>)> {
    let after_dashes = input.strip_prefix("--")?;
    let (after_name, name_text) = match take_while(after_dashes, name) {
      (_, "") => return None,
      found => found,
    };
    let after_eq = after_name.strip_prefix('=')?;
    let (rest, value_text) = self.any_whitespace(after_eq).ok()?;
    let name = self.spanned(after_dashes, after_name, name_text);
    let value = self.spanned(after_eq, rest, value_text);
    let span = Span::new(self.off(input), self.off(rest));
    Some((rest, Flag { span, name, value }))
  }

  /// A quoted string: its span has the quotes, and its content the escapes read, without them.
  fn quoted(&self, input: &'a str) -> Parsed<'a, SpannedString<'a>> {
    let quote = match input.chars().next() {
      Some(quote @ ('"' | '\'' | '`')) => quote,
      _ => return Err(Fail),
    };
    let mut chars = input.char_indices().skip(1);
    let mut end = None;
    while let Some((at, c)) = chars.next() {
      if c == '\\' {
        chars.next();
        continue;
      }
      if c == quote {
        end = Some(at + c.len_utf8());
        break;
      }
    }
    let end = end.ok_or(Fail)?;
    let rest = &input[end..];
    Ok((rest, self.spanned(input, rest, unquote(&input[..end]))))
  }

  /// `[ "a", "b" ]`, with whitespace anywhere and a comma after the last string.
  fn string_array(&self, input: &'a str) -> Parsed<'a, StringArray<'a>> {
    let after_open = input.strip_prefix('[').ok_or(Fail)?;
    let mut s = self.arg_ws_or(after_open);
    let mut elements = Vec::new();
    if let Some(rest) = s.strip_prefix(']') {
      return Ok((rest, StringArray { span: self.span(input, rest), elements }));
    }
    let (rest, first) = self.quoted(s)?;
    elements.push(first);
    s = rest;
    loop {
      let after_ws = self.arg_ws_or(s);
      let Some(after_comma) = after_ws.strip_prefix(',') else {
        s = after_ws;
        break;
      };
      let after_comma = self.arg_ws_or(after_comma);
      if after_comma.starts_with(']') {
        s = after_comma;
        break;
      }
      let (rest, element) = self.quoted(after_comma)?;
      elements.push(element);
      s = rest;
    }
    let rest = self.arg_ws_or(s).strip_prefix(']').ok_or(Fail)?;
    Ok((rest, StringArray { span: self.span(input, rest), elements }))
  }

  /// Content over lines that end in a line continuation, with comment lines among them.
  fn breakable(&self, input: &'a str) -> Parsed<'a, BreakableString<'a>> {
    let mut components = Vec::new();
    let mut s = input;
    loop {
      let after_ws = skip_ws(s);
      if after_ws.starts_with('#') {
        let line_end = match after_ws.find(['\n', '\r']) {
          Some(at) => &after_ws[at..],
          None => "",
        };
        let content = &after_ws[..after_ws.len() - line_end.len()];
        let span = self.span(after_ws, line_end);
        components.push(Component::Comment(SpannedComment { span, content }));
        s = strip_newline(line_end).unwrap_or(line_end);
        if s.is_empty() {
          break;
        }
        continue;
      }
      let (after_content, content) = take_content(s, self.escape);
      // A bare line continuation continues, as Docker reads it, with nothing on its line.
      if content.is_empty()
        && (components.is_empty() || line_continuation(s, self.escape).is_none())
      {
        break;
      }
      let span = self.span(s, after_content);
      components.push(Component::String(SpannedString { span, content: content.into() }));
      s = after_content;
      match line_continuation(s, self.escape) {
        Some(rest) => {
          // Docker skips the empty lines after a continuation.
          s = rest;
          while let Some(rest) = empty_line(s) {
            s = rest;
          }
          if s.is_empty() {
            break;
          }
        }
        None => break,
      }
    }
    // Lines of only whitespace at the end continue to nothing: they print as nothing, and would read back as empty
    // lines, which Docker skips.
    while components.len() > 1
      && matches!(components.last(), Some(Component::String(string)) if string.content.trim().is_empty())
    {
      components.pop();
    }
    let end = components.last().ok_or(Fail)?.end();
    Ok((s, BreakableString { span: Span::new(self.off(input), end), components }))
  }

  /// Whitespace between arguments: spaces, and line continuations with the comment and empty lines after them.
  fn arg_ws(&self, input: &'a str) -> Option<&'a str> {
    self.arg_ws_with(input, true)
  }

  /// [`Self::arg_ws`] that stops at a comment line, for the shell content it comes before.
  fn arg_ws_keeping_comments(&self, input: &'a str) -> Option<&'a str> {
    self.arg_ws_with(input, false)
  }

  fn arg_ws_with(&self, input: &'a str, comments: bool) -> Option<&'a str> {
    let mut s = input;
    loop {
      let after_ws = skip_ws(s);
      if after_ws.len() != s.len() {
        s = after_ws;
        continue;
      }
      let Some(after_continuation) = line_continuation(s, self.escape) else { break };
      s = after_continuation;
      loop {
        if comments && let Some(rest) = comment_line(s) {
          s = rest;
          continue;
        }
        match empty_line(s) {
          Some(rest) => s = rest,
          None => break,
        }
      }
    }
    (s.len() != input.len()).then_some(s)
  }

  fn arg_ws_or(&self, input: &'a str) -> &'a str {
    self.arg_ws(input).unwrap_or(input)
  }

  /// One or more characters, up to whitespace, a line break or a line continuation.
  fn any_whitespace(&self, input: &'a str) -> Parsed<'a, &'a str> {
    let mut end = input.len();
    for (at, c) in input.char_indices() {
      if is_ws(c)
        || is_newline(c)
        || (c == self.escape && line_continuation(&input[at..], self.escape).is_some())
      {
        end = at;
        break;
      }
    }
    match end {
      0 => Err(Fail),
      _ => Ok((&input[end..], &input[..end])),
    }
  }

  /// The end of an instruction's line: whitespace, then a line break, a comment or a line continuation to nothing.
  fn finish_line(&self, input: &'a str) -> Result<&'a str, Fail> {
    let rest = skip_ws(input);
    if rest.is_empty() {
      return Ok(rest);
    }
    if let Some(rest) = strip_newline(rest) {
      return Ok(rest);
    }
    if rest.starts_with('#') {
      return Ok(skip_to_next_line(rest));
    }
    line_continuation(rest, self.escape).ok_or(Fail)
  }

  /// The line as it is, with the lines that it continues to, without trailing whitespace, for a line that doesn't
  /// parse.
  fn unknown_line(&self, input: &'a str) -> (&'a str, SpannedString<'a>) {
    let mut line_end = &input[input.len()..];
    let mut rest = input;
    while let Some(at) = rest.find(['\n', '\r']) {
      line_end = &rest[at..];
      let line = rest[..at].trim_end_matches([' ', '\t']);
      let next = strip_newline(line_end).unwrap_or(line_end);
      if !line.ends_with(self.escape) || next.is_empty() {
        break;
      }
      rest = next;
      line_end = &rest[rest.len()..];
    }
    let content = input[..input.len() - line_end.len()].trim_end();
    let span = Span::new(self.off(input), self.off(input) + content.len());
    let rest = strip_newline(line_end).unwrap_or(line_end);
    (rest, SpannedString { span, content: content.into() })
  }

  /// Wraps the instruction in a [`HeredocInstruction`] when its first line declares heredocs, with their bodies. A
  /// heredoc that doesn't end leaves the instruction as it is, so that a `<<` that isn't one never takes the rest.
  fn heredocs(&self, instruction: Instruction<'a>, rest: &'a str) -> (&'a str, Instruction<'a>) {
    let first_line = &self.base[instruction.span().start..self.off(rest)];
    let delimiters: Vec<Heredoc> = first_line.split_whitespace().filter_map(heredoc).collect();
    if delimiters.is_empty() {
      return (rest, instruction);
    }
    let Some(body_start) = strip_newline(rest) else { return (rest, instruction) };
    let Some((after, body_end)) = self.heredoc_bodies(body_start, &delimiters) else {
      return (rest, instruction);
    };
    let body = &self.base[self.off(body_start)..body_end];
    let span = Span::new(instruction.span().start, body_end);
    (
      after,
      Instruction::Heredoc(HeredocInstruction { span, instruction: Box::new(instruction), body }),
    )
  }

  /// The text after the last closing delimiter, and where the bodies end.
  fn heredoc_bodies(
    &self,
    body_start: &'a str,
    delimiters: &[Heredoc],
  ) -> Option<(&'a str, usize)> {
    let mut current = body_start;
    let mut rest = body_start;
    let mut end = self.off(body_start);
    for delimiter in delimiters {
      loop {
        if current.is_empty() {
          return None;
        }
        let (line, after) = match current.find(['\n', '\r']) {
          Some(at) => (&current[..at], &current[at..]),
          None => (current, &current[current.len()..]),
        };
        let closed = delimiter.closes(line);
        end = self.off(after);
        rest = after;
        current = strip_newline(after).unwrap_or(after);
        if closed {
          break;
        }
        if after.is_empty() {
          return None;
        }
      }
    }
    Some((rest, end))
  }

  /// Where `s` starts in the text. An empty slice from elsewhere is the end of the text.
  fn off(&self, s: &str) -> usize {
    let base = self.base.as_ptr() as usize;
    let at = s.as_ptr() as usize;
    match at < base || at > base + self.base.len() {
      true => self.base.len(),
      false => at - base,
    }
  }

  fn span(&self, from: &str, to: &str) -> Span {
    Span::new(self.off(from), self.off(to))
  }

  fn spanned(&self, from: &str, to: &str, content: impl Into<Cow<'a, str>>) -> SpannedString<'a> {
    SpannedString { span: self.span(from, to), content: content.into() }
  }
}

/// A heredoc that an instruction's first line declares.
struct Heredoc<'a> {
  word: &'a str,
  /// `<<-`, whose closing delimiter can be indented with tabs.
  strip_tabs: bool,
}

impl Heredoc<'_> {
  fn closes(&self, line: &str) -> bool {
    match self.strip_tabs {
      true => line.trim_start_matches('\t') == self.word,
      false => line == self.word,
    }
  }
}

/// A word of the first line that declares a heredoc, as BuildKit reads it: a file descriptor, `<<` or `<<-`, a quote,
/// and a delimiter that starts with a letter or `_`, so that `$((1<<2))` isn't one.
fn heredoc(token: &str) -> Option<Heredoc<'_>> {
  let rest = token.trim_start_matches(|c: char| c.is_ascii_digit()).strip_prefix("<<")?;
  let (strip_tabs, rest) = match rest.strip_prefix('-') {
    Some(rest) => (true, rest),
    None => (false, rest),
  };
  let (quote, rest) = match rest.chars().next() {
    Some(quote @ ('\'' | '"')) => (Some(quote), &rest[1..]),
    _ => (None, rest),
  };
  if !rest.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
    return None;
  }
  let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
  let (word, trailing) = rest.split_at(end);
  let clean = match quote {
    Some(quote) => trailing.len() == 1 && trailing.starts_with(quote),
    None => trailing.is_empty(),
  };
  clean.then_some(Heredoc { word, strip_tabs })
}

fn is_ws(c: char) -> bool {
  c == ' ' || c == '\t'
}

fn is_newline(c: char) -> bool {
  c == '\n' || c == '\r'
}

fn starts_with_newline(s: &str) -> bool {
  matches!(s.as_bytes().first(), Some(b'\n' | b'\r'))
}

fn starts_with_quote(s: &str) -> bool {
  matches!(s.chars().next(), Some('"' | '\'' | '`'))
}

/// The rest of `s` after its leading characters that match, and those characters.
fn take_while(s: &str, matches: impl Fn(char) -> bool) -> (&str, &str) {
  let end = s.find(|c: char| !matches(c)).unwrap_or(s.len());
  (&s[end..], &s[..end])
}

fn skip_ws(s: &str) -> &str {
  s.trim_start_matches(is_ws)
}

/// Letters, which may be none.
fn alpha(s: &str) -> (&str, &str) {
  take_while(s, |c| c.is_ascii_alphabetic())
}

fn strip_newline(s: &str) -> Option<&str> {
  s.strip_prefix("\r\n").or_else(|| s.strip_prefix('\n')).or_else(|| s.strip_prefix('\r'))
}

fn skip_to_next_line(s: &str) -> &str {
  match s.find(['\n', '\r']) {
    Some(at) => strip_newline(&s[at..]).unwrap_or(&s[at..]),
    None => "",
  }
}

fn strip_prefix_ci<'s>(s: &'s str, keyword: &str) -> Option<&'s str> {
  let prefix = s.get(..keyword.len())?;
  prefix.eq_ignore_ascii_case(keyword).then(|| &s[keyword.len()..])
}

/// The escape character, whitespace and a line break, or the end of the text: a continuation to nothing.
pub(crate) fn line_continuation(s: &str, escape: char) -> Option<&str> {
  let rest = skip_ws(s.strip_prefix(escape)?);
  match strip_newline(rest) {
    Some(rest) => Some(rest),
    None if rest.is_empty() => Some(rest),
    None => None,
  }
}

fn comment_line(s: &str) -> Option<&str> {
  let rest = skip_ws(s);
  rest.starts_with('#').then(|| skip_to_next_line(rest))
}

fn empty_line(s: &str) -> Option<&str> {
  strip_newline(skip_ws(s))
}

/// The content up to a line break or a line continuation.
fn take_content(s: &str, escape: char) -> (&str, &str) {
  for (at, c) in s.char_indices() {
    if is_newline(c) || (c == escape && line_continuation(&s[at..], escape).is_some()) {
      return (&s[at..], &s[..at]);
    }
  }
  ("", s)
}

/// An unquoted value with its escapes read: `Rex\ The\ Dog` is `Rex The Dog`.
fn unescape(s: &str, escape: char) -> Cow<'_, str> {
  if !s.contains(escape) {
    return Cow::Borrowed(s);
  }
  let mut result = String::with_capacity(s.len());
  let mut chars = s.chars();
  while let Some(c) = chars.next() {
    match c == escape {
      true => result.push(chars.next().unwrap_or(escape)),
      false => result.push(c),
    }
  }
  Cow::Owned(result)
}

/// A quoted string without its quotes, with its escapes read as the `enquote` crate reads them.
fn unquote(s: &str) -> Cow<'_, str> {
  if s.len() < 2 {
    return Cow::Borrowed("");
  }
  let inner = &s[1..s.len() - 1];
  if !inner.contains('\\') {
    return Cow::Borrowed(inner);
  }
  let mut result = String::with_capacity(inner.len());
  let mut chars = inner.chars();
  while let Some(c) = chars.next() {
    if c != '\\' {
      result.push(c);
      continue;
    }
    match chars.next() {
      Some('n') => result.push('\n'),
      Some('t') => result.push('\t'),
      Some('r') => result.push('\r'),
      Some('b') => result.push('\u{8}'),
      Some('f') => result.push('\u{c}'),
      Some(c @ ('\\' | '"' | '\'' | '`')) => result.push(c),
      Some('\n') => {}
      Some('u') => unicode(&mut result, &mut chars, 4),
      Some('U') => unicode(&mut result, &mut chars, 8),
      Some(other) => {
        result.push('\\');
        result.push(other);
      }
      None => result.push('\\'),
    }
  }
  Cow::Owned(result)
}

/// `\u` with `digits` hex digits. A malformed escape stays as it is written.
fn unicode(result: &mut String, chars: &mut std::str::Chars, digits: usize) {
  let marker = if digits == 4 { 'u' } else { 'U' };
  let mut hex = String::with_capacity(digits);
  for _ in 0..digits {
    match chars.next() {
      Some(c) if c.is_ascii_hexdigit() => hex.push(c),
      other => {
        result.push('\\');
        result.push(marker);
        result.push_str(&hex);
        result.extend(other);
        return;
      }
    }
  }
  match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
    Some(c) => result.push(c),
    None => {
      result.push('\\');
      result.push(marker);
      result.push_str(&hex);
    }
  }
}
