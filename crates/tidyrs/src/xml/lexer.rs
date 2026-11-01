//! The lexer of `@xml-tools/parser`, which chevrotain runs: in each mode, the first pattern that matches wins, not the
//! longest. Its name patterns have bugs that decide which files parse, so they are kept: a name is ASCII, and the
//! ranges of other characters it means to allow are three-character texts, such as `⁰-↏`. A hexadecimal character
//! reference is one token here, where the plugin's lexer takes only its first digit: `&#x4` and then the text `1;`.

use crate::syntax::{Message, Rejected, Rejection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
  Comment,
  CData,
  DocType,
  EntityRef,
  CharRef,
  /// `SEA_WS`
  Space,
  /// `XMLDeclOpen`: `<?xml` and the whitespace after it.
  Declaration,
  SlashOpen,
  /// `PROCESSING_INSTRUCTION`
  Instruction,
  Open,
  Text,
  Close,
  /// `?>`
  SpecialClose,
  SlashClose,
  Slash,
  Equals,
  String,
  Name,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Token {
  pub(crate) kind: Kind,
  pub(crate) start: u32,
  pub(crate) end: u32,
}

/// Splits `text` into tokens, leaving out the whitespace in tags and the declarations that are skipped, such as
/// `<!ELEMENT a>`.
pub(crate) fn lex(
  text: &[u8],
  tokens: &mut Vec<Token>,
  rejection: &Rejection,
) -> Result<(), Rejected> {
  let mut depth = 0usize;
  let mut at = 0;
  while at < text.len() {
    let next = match depth {
      0 => outside(text, at),
      _ => inside(text, at),
    };
    let Some((kind, end, mode)) = next else {
      return Err(rejection.note(Message::UnexpectedCharacter, at as u32));
    };
    match mode {
      Mode::Push => depth += 1,
      Mode::Pop => depth -= 1,
      Mode::Stay => {}
    }
    if let Some(kind) = kind {
      tokens.push(Token { kind, start: at as u32, end: end as u32 });
    }
    at = end;
  }
  Ok(())
}

enum Mode {
  Stay,
  Push,
  Pop,
}

/// The token at `at` outside of tags, its end, and how it changes the mode. `None` as the kind skips the text.
fn outside(text: &[u8], at: usize) -> Option<(Option<Kind>, usize, Mode)> {
  let rest = &text[at..];
  let token = |kind, len| Some((Some(kind), at + len, Mode::Stay));
  let push = |kind, len| Some((Some(kind), at + len, Mode::Push));
  match rest[0] {
    b'<' => {
      if let Some(end) = comment(text, at) {
        return Some((Some(Kind::Comment), end, Mode::Stay));
      }
      if rest.starts_with(b"<![CDATA[")
        && let Some(end) = lazy(text, at + 9, b"]]>")
      {
        return Some((Some(Kind::CData), end, Mode::Stay));
      }
      if rest.starts_with(b"<!DOCTYPE") {
        return push(Kind::DocType, 9);
      }
      if rest.starts_with(b"<!")
        && let Some(end) = skipped(text, at + 2)
      {
        return Some((None, end, Mode::Stay));
      }
      if rest.starts_with(b"<?xml") && matches!(rest.get(5), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        return push(Kind::Declaration, 6);
      }
      if rest.starts_with(b"</") {
        return push(Kind::SlashOpen, 2);
      }
      if rest.starts_with(b"<?")
        && let Some(end) = instruction(text, at)
      {
        return Some((Some(Kind::Instruction), end, Mode::Stay));
      }
      push(Kind::Open, 1)
    }
    b'&' => {
      if let Some(len) = name(&rest[1..])
        && rest.get(1 + len) == Some(&b';')
      {
        return token(Kind::EntityRef, len + 2);
      }
      if let Some(digits) = rest.strip_prefix(b"&#") {
        let count = digits.iter().take_while(|byte| byte.is_ascii_digit()).count();
        if count > 0 && digits.get(count) == Some(&b';') {
          return token(Kind::CharRef, count + 3);
        }
        let hex = digits.strip_prefix(b"x").unwrap_or_default();
        let count = hex.iter().take_while(|byte| byte.is_ascii_hexdigit()).count();
        if count > 0 && hex.get(count) == Some(&b';') {
          return token(Kind::CharRef, count + 4);
        }
        if count > 0 {
          return token(Kind::CharRef, 4);
        }
      }
      None
    }
    b' ' | b'\t' | b'\n' => {
      let len = rest.iter().take_while(|&&byte| matches!(byte, b' ' | b'\t' | b'\n')).count();
      token(Kind::Space, len)
    }
    _ => {
      let len = memchr::memchr2(b'<', b'&', rest).unwrap_or(rest.len());
      token(Kind::Text, len)
    }
  }
}

/// The token at `at` inside a tag, its end, and how it changes the mode.
fn inside(text: &[u8], at: usize) -> Option<(Option<Kind>, usize, Mode)> {
  let rest = &text[at..];
  let token = |kind, len| Some((Some(kind), at + len, Mode::Stay));
  let pop = |kind, len| Some((Some(kind), at + len, Mode::Pop));
  match rest[0] {
    b'<' => match comment(text, at) {
      Some(end) => Some((Some(Kind::Comment), end, Mode::Stay)),
      None if rest.starts_with(b"</") => token(Kind::SlashOpen, 2),
      None => token(Kind::Open, 1),
    },
    b'>' => pop(Kind::Close, 1),
    b'?' if rest.starts_with(b"?>") => pop(Kind::SpecialClose, 2),
    b'/' if rest.starts_with(b"/>") => pop(Kind::SlashClose, 2),
    b'/' => token(Kind::Slash, 1),
    b'=' => token(Kind::Equals, 1),
    quote @ (b'"' | b'\'') => {
      let len = rest[1..].iter().position(|&byte| byte == quote || byte == b'<')?;
      (rest[1 + len] == quote).then_some((Some(Kind::String), at + len + 2, Mode::Stay))
    }
    b' ' | b'\t' | b'\r' | b'\n' => Some((None, at + 1, Mode::Stay)),
    _ => name(rest).map(|len| (Some(Kind::Name), at + len, Mode::Stay)),
  }
}

/// The end of the comment at `at`, if there is one.
fn comment(text: &[u8], at: usize) -> Option<usize> {
  match text[at..].starts_with(b"<!--") {
    true => lazy(text, at + 4, b"-->"),
    false => None,
  }
}

/// `(.|\r?\n)*?` and then `close`: the end of the first `close` from `from` on, unless a U+2028 or U+2029 comes before
/// it.
fn lazy(text: &[u8], from: usize, close: &[u8]) -> Option<usize> {
  let found = memchr::memmem::find(&text[from..], close)? + from;
  let between = &text[from..found];
  let blocked = ["\u{2028}", "\u{2029}"]
    .iter()
    .any(|separator| memchr::memmem::find(between, separator.as_bytes()).is_some());
  (!blocked).then_some(found + close.len())
}

/// `<!.*?>`, which is skipped: the end of the first `>` from `from` on, before any line break.
fn skipped(text: &[u8], from: usize) -> Option<usize> {
  let line = line_end(text, from);
  memchr::memchr(b'>', &text[from..line]).map(|at| from + at + 1)
}

/// `<\?Name.*\?>`: the end of the last `?>` on the line, after a name.
fn instruction(text: &[u8], at: usize) -> Option<usize> {
  let first = start_char(&text[at + 2..])?;
  let from = at + 2 + first;
  let line = line_end(text, from);
  memchr::memmem::rfind(&text[from..line], b"?>").map(|found| from + found + 2)
}

/// Where the line that `from` is on ends: at `\n`, `\r`, U+2028 or U+2029, which `.` doesn't match.
fn line_end(text: &[u8], from: usize) -> usize {
  let mut at = from;
  while let Some(found) = memchr::memchr3(b'\n', b'\r', 0xE2, &text[at..]) {
    let end = at + found;
    if text[end] != 0xE2
      || (text.get(end + 1) == Some(&0x80) && matches!(text.get(end + 2), Some(0xA8 | 0xA9)))
    {
      return end;
    }
    at = end + 1;
  }
  text.len()
}

/// The length of the name that `text` starts with.
pub(crate) fn name(text: &[u8]) -> Option<usize> {
  let mut len = start_char(text)?;
  while let Some(more) = start_char(&text[len..]).or_else(|| name_char(&text[len..])) {
    len += more;
  }
  Some(len)
}

/// The three-character texts that the name patterns mean as ranges.
const TRIPLES: [(char, char); 5] = [
  ('\u{2070}', '\u{218F}'),
  ('\u{2C00}', '\u{2FEF}'),
  ('\u{3001}', '\u{D7FF}'),
  ('\u{F900}', '\u{FDCF}'),
  ('\u{FDF0}', '\u{FFFD}'),
];

/// `NameStartChar`: the length of the one it starts with.
fn start_char(text: &[u8]) -> Option<usize> {
  match *text.first()? {
    b':' | b'_' | b'a'..=b'z' | b'A'..=b'Z' => Some(1),
    byte if byte < 0x80 => None,
    _ => {
      let mut chars = std::str::from_utf8(&text[..text.len().min(12)])
        .or_else(|error| std::str::from_utf8(&text[..error.valid_up_to()]))
        .ok()?
        .chars();
      let (first, dash, last) = (chars.next()?, chars.next()?, chars.next()?);
      TRIPLES
        .iter()
        .any(|&triple| (first, last) == triple && dash == '-')
        .then(|| first.len_utf8() + 1 + last.len_utf8())
    }
  }
}

/// The characters that `NameChar` adds to `NameStartChar`: the length of the one it starts with.
fn name_char(text: &[u8]) -> Option<usize> {
  match *text.first()? {
    b'-' | b'.' | b'0'..=b'9' => Some(1),
    byte if byte < 0x80 => None,
    _ => {
      let valid = match std::str::from_utf8(&text[..text.len().min(4)]) {
        Ok(valid) => valid,
        Err(error) => std::str::from_utf8(&text[..error.valid_up_to()]).ok()?,
      };
      let char = valid.chars().next()?;
      matches!(char, '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}')
        .then(|| char.len_utf8())
    }
  }
}
