//! The lexer of `yaml` 2.9.0 (`src/parse/lexer.ts`), for a whole input. It splits the text into lexemes: slices of the
//! text, and three markers that aren't in it. Offsets are in bytes.

/// A lexeme: a slice of the text, or a marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lexeme {
  /// The text from `start` to `end`. It can be empty, after [`Lexeme::Scalar`].
  Text(u32, u32),
  /// `\x02`: a document starts with the next lexeme.
  Document,
  /// `\x18`: a flow collection ends where it shouldn't.
  FlowEnd,
  /// `\x1f`: the next lexeme is a scalar.
  Scalar,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
  Stream,
  BlockStart,
  Doc,
  Flow,
}

const BOM: &str = "\u{feff}";

/// `isEmpty`
fn is_empty(byte: Option<u8>) -> bool {
  matches!(byte, None | Some(b' ' | b'\n' | b'\r' | b'\t'))
}

fn is_flow_indicator(byte: Option<u8>) -> bool {
  matches!(byte, Some(b',' | b'[' | b']' | b'{' | b'}'))
}

/// `isNotAnchorChar`
fn is_not_anchor_char(byte: Option<u8>) -> bool {
  matches!(byte, None | Some(b' ' | b',' | b'[' | b']' | b'{' | b'}' | b'\n' | b'\r' | b'\t'))
}

fn is_tag_char(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || b"-#;/?:@&=+$_.!~*'()".contains(&byte)
}

pub(crate) struct Lexer<'l> {
  text: &'l [u8],
  out: &'l mut Vec<Lexeme>,
  block_scalar_indent: isize,
  block_scalar_keep: bool,
  flow_key: bool,
  flow_level: usize,
  indent_next: usize,
  indent_value: usize,
  pos: usize,
}

impl<'l> Lexer<'l> {
  pub(crate) fn lex(text: &'l [u8], out: &'l mut Vec<Lexeme>) {
    let mut lexer = Lexer {
      text,
      out,
      block_scalar_indent: -1,
      block_scalar_keep: false,
      flow_key: false,
      flow_level: 0,
      indent_next: 0,
      indent_value: 0,
      pos: 0,
    };
    let mut next = State::Stream;
    while lexer.pos < text.len() {
      next = match next {
        State::Stream => lexer.parse_stream(),
        State::BlockStart => lexer.parse_block_start(),
        State::Doc => lexer.parse_document(),
        State::Flow => lexer.parse_flow_collection(),
      };
    }
  }

  fn at(&self, index: usize) -> Option<u8> {
    self.text.get(index).copied()
  }

  fn char_at(&self, n: usize) -> Option<u8> {
    self.at(self.pos + n)
  }

  fn starts_with(&self, at: usize, part: &[u8]) -> bool {
    self.text.get(at..).is_some_and(|rest| rest.starts_with(part))
  }

  fn at_line_end(&self) -> bool {
    let mut i = self.pos;
    while matches!(self.at(i), Some(b' ' | b'\t')) {
      i += 1;
    }
    match self.at(i) {
      None | Some(b'#' | b'\n') => true,
      Some(b'\r') => self.at(i + 1) == Some(b'\n'),
      _ => false,
    }
  }

  /// `continueScalar`: where a scalar continues on the line at `offset`, or `None` if it ends before it.
  fn continue_scalar(&self, offset: usize) -> Option<usize> {
    let ch = self.at(offset);
    if self.indent_next > 0 {
      let mut indent = 0;
      while self.at(offset + indent) == Some(b' ') {
        indent += 1;
      }
      let ch = self.at(offset + indent);
      if ch == Some(b'\r') && self.at(offset + indent + 1) == Some(b'\n') {
        return Some(offset + indent + 1);
      }
      return (ch == Some(b'\n') || indent >= self.indent_next || ch.is_none())
        .then_some(offset + indent);
    }
    if matches!(ch, Some(b'-' | b'.'))
      && (self.starts_with(offset, b"---") || self.starts_with(offset, b"..."))
      && is_empty(self.at(offset + 3))
    {
      return None;
    }
    Some(offset)
  }

  /// `getLine`: the rest of the line, without its line break. Like JavaScript's `substring`, it swaps its bounds when
  /// the line break is a `\r\n` that starts before `pos`.
  fn line(&self) -> &'l [u8] {
    let text = self.text;
    let end = match text[self.pos..].iter().position(|byte| *byte == b'\n') {
      Some(at) => {
        let end = self.pos + at;
        if end > 0 && text[end - 1] == b'\r' { end - 1 } else { end }
      }
      None => text.len(),
    };
    &text[self.pos.min(end)..self.pos.max(end)]
  }

  fn parse_stream(&mut self) -> State {
    let mut line = self.line();
    if line.starts_with(BOM.as_bytes()) {
      self.push_count(BOM.len());
      line = &line[BOM.len()..];
    }
    if line.first() == Some(&b'%') {
      let mut end = line.len();
      let mut comment = line.iter().position(|byte| *byte == b'#');
      while let Some(at) = comment {
        if at > 0 && matches!(line[at - 1], b' ' | b'\t') {
          end = at - 1;
          break;
        }
        comment = line[at + 1..].iter().position(|byte| *byte == b'#').map(|next| at + 1 + next);
      }
      while end > 0 && matches!(line[end - 1], b' ' | b'\t') {
        end -= 1;
      }
      let n = self.push_count(end) + self.push_spaces(true);
      self.push_count(line.len() - n);
      // The line break is pushed by the next round: `yaml` calls `pushNewline()` here without `yield*`.
      return State::Stream;
    }
    if self.at_line_end() {
      let spaces = self.push_spaces(true);
      self.push_count(line.len() - spaces);
      self.push_newline();
      return State::Stream;
    }
    self.out.push(Lexeme::Document);
    self.parse_line_start()
  }

  fn parse_line_start(&mut self) -> State {
    let ch = self.char_at(0);
    if matches!(ch, Some(b'-' | b'.'))
      && (self.starts_with(self.pos, b"---") || self.starts_with(self.pos, b"..."))
      && is_empty(self.char_at(3))
    {
      let start = self.starts_with(self.pos, b"---");
      self.push_count(3);
      self.indent_value = 0;
      self.indent_next = 0;
      return if start { State::Doc } else { State::Stream };
    }
    self.indent_value = self.push_spaces(false);
    if self.indent_next > self.indent_value && !is_empty(self.char_at(1)) {
      self.indent_next = self.indent_value;
    }
    self.parse_block_start()
  }

  fn parse_block_start(&mut self) -> State {
    let (ch0, ch1) = (self.char_at(0), self.char_at(1));
    if matches!(ch0, Some(b'-' | b'?' | b':')) && is_empty(ch1) {
      let n = self.push_count(1) + self.push_spaces(true);
      self.indent_next = self.indent_value + 1;
      self.indent_value += n;
      return State::BlockStart;
    }
    State::Doc
  }

  fn parse_document(&mut self) -> State {
    self.push_spaces(true);
    let line = self.line();
    let mut n = self.push_indicators();
    match line.get(n) {
      Some(b'#') => {
        self.push_count(line.len() - n);
        self.push_newline();
        self.parse_line_start()
      }
      None => {
        self.push_newline();
        self.parse_line_start()
      }
      Some(b'{' | b'[') => {
        self.push_count(1);
        self.flow_key = false;
        self.flow_level = 1;
        State::Flow
      }
      Some(b'}' | b']') => {
        self.push_count(1);
        State::Doc
      }
      Some(b'*') => {
        self.push_until(is_not_anchor_char);
        State::Doc
      }
      Some(b'"' | b'\'') => self.parse_quoted_scalar(),
      Some(b'|' | b'>') => {
        n += self.parse_block_scalar_header();
        n += self.push_spaces(true);
        self.push_count(line.len() - n);
        self.push_newline();
        self.parse_block_scalar()
      }
      _ => self.parse_plain_scalar(),
    }
  }

  fn parse_flow_collection(&mut self) -> State {
    let mut indent: isize = -1;
    loop {
      let nl = self.push_newline();
      let mut sp = 0;
      if nl > 0 {
        sp = self.push_spaces(false);
        self.indent_value = sp;
        indent = sp as isize;
      }
      sp += self.push_spaces(true);
      if nl + sp == 0 {
        break;
      }
    }
    let line = self.line();
    let marker = indent == 0
      && (line.starts_with(b"---") || line.starts_with(b"..."))
      && is_empty(line.get(3).copied());
    if (indent != -1 && (indent as usize) < self.indent_next && line.first() != Some(&b'#'))
      || marker
    {
      let at_flow_end_marker = indent == self.indent_next as isize - 1
        && self.flow_level == 1
        && matches!(line.first(), Some(b']' | b'}'));
      if !at_flow_end_marker {
        self.flow_level = 0;
        self.out.push(Lexeme::FlowEnd);
        return self.parse_line_start();
      }
    }
    let mut n = 0;
    while line.get(n) == Some(&b',') {
      n += self.push_count(1);
      n += self.push_spaces(true);
      self.flow_key = false;
    }
    n += self.push_indicators();
    match line.get(n) {
      None => State::Flow,
      Some(b'#') => {
        self.push_count(line.len() - n);
        State::Flow
      }
      Some(b'{' | b'[') => {
        self.push_count(1);
        self.flow_key = false;
        self.flow_level += 1;
        State::Flow
      }
      Some(b'}' | b']') => {
        self.push_count(1);
        self.flow_key = true;
        self.flow_level -= 1;
        if self.flow_level > 0 { State::Flow } else { State::Doc }
      }
      Some(b'*') => {
        self.push_until(is_not_anchor_char);
        State::Flow
      }
      Some(b'"' | b'\'') => {
        self.flow_key = true;
        self.parse_quoted_scalar()
      }
      Some(b':')
        if {
          let next = self.char_at(1);
          self.flow_key || is_empty(next) || next == Some(b',')
        } =>
      {
        self.flow_key = false;
        self.push_count(1);
        self.push_spaces(true);
        State::Flow
      }
      _ => {
        self.flow_key = false;
        self.parse_plain_scalar()
      }
    }
  }

  fn parse_quoted_scalar(&mut self) -> State {
    let text = self.text;
    let quote = text[self.pos];
    let find = |from: usize| {
      text
        .get(from..)
        .and_then(|rest| rest.iter().position(|byte| *byte == quote))
        .map(|at| from + at)
    };
    let mut end = find(self.pos + 1);
    if quote == b'\'' {
      while let Some(at) = end
        && text.get(at + 1) == Some(&b'\'')
      {
        end = find(at + 2);
      }
    } else {
      while let Some(at) = end {
        let backslashes = text[..at].iter().rev().take_while(|byte| **byte == b'\\').count();
        if backslashes % 2 == 0 {
          break;
        }
        end = find(at + 1);
      }
    }
    // Only looking for newlines within the quotes, of which there are none if the quote doesn't end.
    let quoted = &text[..end.unwrap_or(0)];
    let newline = |from: usize| {
      quoted
        .get(from..)
        .and_then(|rest| rest.iter().position(|byte| *byte == b'\n'))
        .map(|at| from + at)
    };
    let mut nl = newline(self.pos);
    if nl.is_some() {
      while let Some(at) = nl {
        let Some(cs) = self.continue_scalar(at + 1) else { break };
        nl = newline(cs);
      }
      if let Some(at) = nl {
        // An error, from an unexpected unindent.
        end = Some(at - if at > 0 && quoted[at - 1] == b'\r' { 2 } else { 1 });
      }
    }
    let end = end.unwrap_or(text.len());
    self.push_to_index(end + 1, false);
    if self.flow_level > 0 { State::Flow } else { State::Doc }
  }

  fn parse_block_scalar_header(&mut self) -> usize {
    self.block_scalar_indent = -1;
    self.block_scalar_keep = false;
    let mut i = self.pos;
    loop {
      i += 1;
      match self.at(i) {
        Some(b'+') => self.block_scalar_keep = true,
        Some(digit @ b'1'..=b'9') => self.block_scalar_indent = (digit - b'0') as isize - 1,
        Some(b'-') => {}
        _ => break,
      }
    }
    self.push_until(|ch| is_empty(ch) || ch == Some(b'#'))
  }

  fn parse_block_scalar(&mut self) -> State {
    let text = self.text;
    let mut nl: isize = self.pos as isize - 1;
    let mut indent = 0;
    let mut i = self.pos;
    while let Some(ch) = self.at(i) {
      match ch {
        b' ' => indent += 1,
        b'\n' => {
          nl = i as isize;
          indent = 0;
        }
        b'\r' if self.at(i + 1) == Some(b'\n') => {}
        _ => break,
      }
      i += 1;
    }
    if indent >= self.indent_next {
      if self.block_scalar_indent == -1 {
        self.indent_next = indent;
      } else {
        let base = if self.indent_next == 0 { 1 } else { self.indent_next };
        self.indent_next = self.block_scalar_indent as usize + base;
      }
      while let Some(cs) = self.continue_scalar((nl + 1) as usize) {
        match text[cs.min(text.len())..].iter().position(|byte| *byte == b'\n') {
          Some(at) => nl = (cs + at) as isize,
          None => {
            nl = text.len() as isize;
            break;
          }
        }
      }
    }
    // Trailing insufficiently indented tabs are invalid. To catch that during parsing, they are in the value.
    let mut i = (nl + 1) as usize;
    while self.at(i) == Some(b' ') {
      i += 1;
    }
    if self.at(i) == Some(b'\t') {
      while matches!(self.at(i), Some(b'\t' | b' ' | b'\r' | b'\n')) {
        i += 1;
      }
      nl = i as isize - 1;
    } else if !self.block_scalar_keep {
      loop {
        let mut i = nl - 1;
        let mut ch = self.at_signed(i);
        if ch == Some(b'\r') {
          i -= 1;
          ch = self.at_signed(i);
        }
        let last = i;
        while ch == Some(b' ') {
          i -= 1;
          ch = self.at_signed(i);
        }
        if ch == Some(b'\n') && i >= self.pos as isize && i + 1 + indent as isize > last {
          nl = i;
        } else {
          break;
        }
      }
    }
    self.out.push(Lexeme::Scalar);
    self.push_to_index((nl + 1) as usize, true);
    self.parse_line_start()
  }

  fn at_signed(&self, index: isize) -> Option<u8> {
    usize::try_from(index).ok().and_then(|index| self.at(index))
  }

  fn parse_plain_scalar(&mut self) -> State {
    let in_flow = self.flow_level > 0;
    let mut end = self.pos as isize - 1;
    let mut i = self.pos as isize - 1;
    loop {
      i += 1;
      let Some(mut ch) = self.at_signed(i) else { break };
      if ch == b':' {
        let next = self.at_signed(i + 1);
        if is_empty(next) || (in_flow && is_flow_indicator(next)) {
          break;
        }
        end = i;
      } else if is_empty(Some(ch)) {
        let mut next = self.at_signed(i + 1);
        if ch == b'\r' {
          if next == Some(b'\n') {
            i += 1;
            ch = b'\n';
            next = self.at_signed(i + 1);
          } else {
            end = i;
          }
        }
        if next == Some(b'#') || (in_flow && is_flow_indicator(next)) {
          break;
        }
        if ch == b'\n' {
          let Some(cs) = self.continue_scalar((i + 1) as usize) else { break };
          i = i.max(cs as isize - 2);
        }
      } else {
        if in_flow && is_flow_indicator(Some(ch)) {
          break;
        }
        end = i;
      }
    }
    self.out.push(Lexeme::Scalar);
    self.push_to_index((end + 1) as usize, true);
    if in_flow { State::Flow } else { State::Doc }
  }

  fn emit(&mut self, start: usize, end: usize) {
    self.out.push(Lexeme::Text(start as u32, end as u32));
  }

  fn push_count(&mut self, n: usize) -> usize {
    if n == 0 {
      return 0;
    }
    let end = (self.pos + n).min(self.text.len());
    self.emit(self.pos, end);
    let n = end - self.pos;
    self.pos = end;
    n
  }

  fn push_to_index(&mut self, index: usize, allow_empty: bool) -> usize {
    let index = index.min(self.text.len());
    if index > self.pos {
      let n = index - self.pos;
      self.emit(self.pos, index);
      self.pos = index;
      return n;
    }
    if allow_empty {
      self.emit(self.pos, self.pos);
    }
    0
  }

  fn push_indicators(&mut self) -> usize {
    let mut n = 0;
    loop {
      match self.char_at(0) {
        Some(b'!') => {
          n += self.push_tag();
          n += self.push_spaces(true);
        }
        Some(b'&') => {
          n += self.push_until(is_not_anchor_char);
          n += self.push_spaces(true);
        }
        Some(b'-' | b'?' | b':') => {
          let in_flow = self.flow_level > 0;
          let ch1 = self.char_at(1);
          if !(is_empty(ch1) || (in_flow && is_flow_indicator(ch1))) {
            break;
          }
          if !in_flow {
            self.indent_next = self.indent_value + 1;
          } else if self.flow_key {
            self.flow_key = false;
          }
          n += self.push_count(1);
          n += self.push_spaces(true);
        }
        _ => break,
      }
    }
    n
  }

  fn push_tag(&mut self) -> usize {
    if self.char_at(1) == Some(b'<') {
      let mut i = self.pos + 2;
      while !is_empty(self.at(i)) && self.at(i) != Some(b'>') {
        i += 1;
      }
      let end = if self.at(i) == Some(b'>') { i + 1 } else { i };
      return self.push_to_index(end, false);
    }
    let mut i = self.pos + 1;
    while let Some(ch) = self.at(i) {
      if is_tag_char(ch) {
        i += 1;
      } else if ch == b'%'
        && self.at(i + 1).is_some_and(|it| it.is_ascii_hexdigit())
        && self.at(i + 2).is_some_and(|it| it.is_ascii_hexdigit())
      {
        i += 3;
      } else {
        break;
      }
    }
    self.push_to_index(i, false)
  }

  fn push_newline(&mut self) -> usize {
    match (self.char_at(0), self.char_at(1)) {
      (Some(b'\n'), _) => self.push_count(1),
      (Some(b'\r'), Some(b'\n')) => self.push_count(2),
      _ => 0,
    }
  }

  fn push_spaces(&mut self, tabs: bool) -> usize {
    let mut i = self.pos;
    while matches!(self.at(i), Some(b' ')) || (tabs && self.at(i) == Some(b'\t')) {
      i += 1;
    }
    let n = i - self.pos;
    if n > 0 {
      self.emit(self.pos, i);
      self.pos = i;
    }
    n
  }

  fn push_until(&mut self, stop: impl Fn(Option<u8>) -> bool) -> usize {
    let mut i = self.pos;
    while !stop(self.at(i)) {
      i += 1;
    }
    self.push_to_index(i, false)
  }
}
