//! taplo's lexer (`syntax.rs`): the longest match of its patterns, the one with the highest priority on a tie, as logos
//! picks it. This file depends on nothing else, so the oracle can compare it with logos itself.

/// A token of taplo's lexer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Token {
  Whitespace,
  Newline,
  Comment,
  Ident,
  IdentWithGlob,
  Period,
  Comma,
  Eq,
  String,
  MultiLineString,
  StringLiteral,
  MultiLineStringLiteral,
  Integer,
  IntegerHex,
  IntegerOct,
  IntegerBin,
  Float,
  Bool,
  DateTimeOffset,
  DateTimeLocal,
  Date,
  Time,
  BracketStart,
  BracketEnd,
  BraceStart,
  BraceEnd,
  Error,
}

/// The token at the start of `text`, and its length. `text` is not empty.
pub(crate) fn next(text: &[u8]) -> (Token, usize) {
  match text[0] {
    b' ' | b'\t' => return (Token::Whitespace, run(text, |byte| matches!(byte, b' ' | b'\t'))),
    b'\n' | b'\r' => {
      let length = newlines(text);
      return match length {
        0 => (Token::Error, 1),
        _ => (Token::Newline, length),
      };
    }
    b'#' => return (Token::Comment, 1 + run(&text[1..], |byte| !matches!(byte, b'\n' | b'\r'))),
    b'.' => return (Token::Period, 1),
    b',' => return (Token::Comma, 1),
    b'=' => return (Token::Eq, 1),
    b'[' => return (Token::BracketStart, 1),
    b']' => return (Token::BracketEnd, 1),
    b'{' => return (Token::BraceStart, 1),
    b'}' => return (Token::BraceEnd, 1),
    b'"' => {
      return match text.starts_with(b"\"\"\"") {
        true => callback(Token::MultiLineString, 3, multi_line_string(&text[3..])),
        false => callback(Token::String, 1, string(&text[1..])),
      };
    }
    b'\'' => {
      return match text.starts_with(b"'''") {
        true => callback(Token::MultiLineStringLiteral, 3, multi_line_literal(&text[3..])),
        false => callback(Token::StringLiteral, 1, literal(&text[1..])),
      };
    }
    _ => {}
  }
  let glob = |byte: u8| is_ident(byte) || matches!(byte, b'*' | b'?');
  // A word that starts with a letter can only be these, and logos reads no further than their longest match.
  if text[0].is_ascii_alphabetic() {
    let candidates = [
      (Token::Bool, boolean(text)),
      (Token::Float, float(text)),
      (Token::Ident, run(text, is_ident)),
      (Token::IdentWithGlob, run(text, glob)),
    ];
    return longest(candidates);
  }
  // The patterns of words, by priority: the longest wins, and on a tie, the one listed first.
  let candidates = [
    (Token::DateTimeOffset, date_time(text, true)),
    (Token::DateTimeLocal, date_time(text, false)),
    (Token::Date, date(text)),
    (Token::Time, time(text)),
    (Token::Bool, boolean(text)),
    (Token::IntegerHex, radix(text, b'x', |byte| byte.is_ascii_hexdigit())),
    (Token::IntegerOct, radix(text, b'o', |byte| matches!(byte, b'0'..=b'7'))),
    (Token::IntegerBin, radix(text, b'b', |byte| matches!(byte, b'0' | b'1'))),
    (Token::Integer, integer(text)),
    (Token::Float, float(text)),
    (Token::Ident, run(text, is_ident)),
    (Token::IdentWithGlob, run(text, glob)),
  ];
  match longest(candidates) {
    (_, 0) => (Token::Error, char_length(text)),
    (token, length) if reach(text) > length => (backtracked(token, length), length),
    best => best,
  }
}

/// The longest match, and on a tie, the one listed first.
fn longest<const N: usize>(candidates: [(Token, usize); N]) -> (Token, usize) {
  let mut best = (Token::Error, 0);
  for (token, length) in candidates {
    if length > best.1 {
      best = (token, length);
    }
  }
  best
}

/// How far logos reads before it gives up on a float, a date or a time: the longest prefix of `text` that one of them
/// could still start with.
///
/// When that is past the longest match, logos's generated code backtracks to the end of the match, but keeps the kind
/// of the last state that knew how to backtrack, which [`backtracked`] gives.
fn reach(text: &[u8]) -> usize {
  float_reach(text).max(date_reach(text)).max(time_reach(text))
}

/// The kind logos gives a match of `length` that it backtracked to. Only loops of digits know how to backtrack: those
/// of an `INTEGER`, which floats, dates and times all start with, of a `FLOAT`, and of the fraction of a
/// `DATE_TIME_LOCAL`. So `1979-05-27 x` starts with an `INTEGER`, and so does `9e+x`.
fn backtracked(token: Token, length: usize) -> Token {
  match token {
    Token::Float => Token::Float,
    Token::DateTimeLocal if length > 19 => Token::DateTimeLocal,
    _ => Token::Integer,
  }
}

/// The longest prefix of `[-+]?([0-9_]+(\.[0-9_]+)?([eE][+-]?[0-9_]+)?|nan|inf)`.
fn float_reach(text: &[u8]) -> usize {
  let signed = sign(text);
  let rest = &text[signed..];
  let mut at = digits(rest);
  if at == 0 {
    let word = |word: &[u8]| rest.iter().zip(word).take_while(|(a, b)| a == b).count();
    return signed + word(b"nan").max(word(b"inf"));
  }
  if rest.get(at) == Some(&b'.') {
    match digits(&rest[at + 1..]) {
      0 => return signed + at + 1,
      fraction => at += 1 + fraction,
    }
  }
  if matches!(rest.get(at), Some(b'e' | b'E')) {
    at += 1 + sign(&rest[at + 1..]);
    at += digits(&rest[at..]);
  }
  signed + at
}

/// The longest prefix of `DATE`, `DATE_TIME_LOCAL` or `DATE_TIME_OFFSET`.
fn date_reach(text: &[u8]) -> usize {
  let at = |index: usize| text.get(index).copied().unwrap_or(0);
  let is_digit = |index: usize| at(index).is_ascii_digit();
  let steps = [
    matches!(at(0), b'1'..=b'9'),
    is_digit(1),
    is_digit(2),
    is_digit(3),
    at(4) == b'-',
    matches!(at(5), b'0' | b'1'),
    match at(5) {
      b'0' => matches!(at(6), b'1'..=b'9'),
      _ => matches!(at(6), b'0'..=b'2'),
    },
    at(7) == b'-',
  ];
  let reach = steps.iter().take_while(|it| **it).count();
  if reach < 8 {
    return reach;
  }
  let month = (at(5) - b'0') * 10 + at(6) - b'0';
  let year = (0..4).fold(0u32, |year, index| year * 10 + (at(index) - b'0') as u32);
  let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
  let tens = match at(8) {
    b'0'..=b'2' => at(8) - b'0',
    b'3' if month != 2 => 3,
    _ => return 8,
  };
  let ones = match (tens, at(9)) {
    (0, b'1'..=b'9') | (1, b'0'..=b'9') | (2, b'0'..=b'8') | (3, b'0') => true,
    (2, b'9') => month != 2 || leap,
    (3, b'1') => matches!(month, 1 | 3 | 5 | 7 | 8 | 10 | 12),
    _ => false,
  };
  if !ones {
    return 9;
  }
  if !matches!(at(10), b'T' | b't' | b' ') {
    return 10;
  }
  let time = time_reach(&text[11..]);
  if time < 8 || time == 9 {
    return 11 + time;
  }
  let end = 11 + time;
  match at(end) {
    b'Z' | b'z' => end + 1,
    b'+' | b'-' => {
      let offset = [
        matches!(at(end + 1), b'0' | b'1'),
        is_digit(end + 2),
        at(end + 3) == b':',
        matches!(at(end + 4), b'0'..=b'5'),
        is_digit(end + 5),
      ];
      end + 1 + offset.iter().take_while(|it| **it).count()
    }
    _ => end,
  }
}

/// The longest prefix of `TIME`, with the separator of a fraction that has no digits yet.
fn time_reach(text: &[u8]) -> usize {
  let at = |index: usize| text.get(index).copied().unwrap_or(0);
  let steps = [
    matches!(at(0), b'0'..=b'2'),
    match at(0) {
      b'2' => matches!(at(1), b'0'..=b'3'),
      _ => at(1).is_ascii_digit(),
    },
    at(2) == b':',
    matches!(at(3), b'0'..=b'5'),
    at(4).is_ascii_digit(),
    at(5) == b':',
    matches!(at(6), b'0'..=b'5'),
    at(7).is_ascii_digit(),
    matches!(at(8), b'.' | b','),
  ];
  match steps.iter().take_while(|it| **it).count() {
    9 => 9 + run(&text[9..], |byte| byte.is_ascii_digit()),
    reach => reach,
  }
}

fn callback(token: Token, open: usize, length: Option<usize>) -> (Token, usize) {
  match length {
    Some(length) => (token, open + length),
    None => (Token::Error, open),
  }
}

fn run(text: &[u8], accept: impl Fn(u8) -> bool) -> usize {
  text.iter().take_while(|byte| accept(**byte)).count()
}

fn is_ident(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn char_length(text: &[u8]) -> usize {
  match text[0] {
    0xF0.. => 4,
    0xE0.. => 3,
    0xC0.. => 2,
    _ => 1,
  }
  .min(text.len())
}

/// `(\n|\r\n)+`
fn newlines(text: &[u8]) -> usize {
  let mut at = 0;
  loop {
    match &text[at..] {
      [b'\n', ..] => at += 1,
      [b'\r', b'\n', ..] => at += 2,
      _ => return at,
    }
  }
}

/// `lex_string`: up to the first `"` that no backslash escapes, across lines.
fn string(text: &[u8]) -> Option<usize> {
  let mut escaped = false;
  for (at, byte) in text.iter().enumerate() {
    if *byte == b'\\' {
      escaped = !escaped;
      continue;
    }
    if *byte == b'"' && !escaped {
      return Some(at + 1);
    }
    escaped = false;
  }
  None
}

/// `lex_multi_line_string`: up to `"""`, with the quotes right after it, and no more than five.
fn multi_line_string(text: &[u8]) -> Option<usize> {
  let (mut length, mut quotes, mut escaped, mut found) = (0, 0, false, false);
  for byte in text {
    if found {
      if *byte != b'"' {
        return (quotes < 6).then_some(length);
      }
      quotes += 1;
      length += 1;
      continue;
    }
    length += 1;
    if *byte == b'\\' {
      escaped = true;
      continue;
    }
    match *byte == b'"' && !escaped {
      true => quotes += 1,
      false => quotes = 0,
    }
    if quotes == 3 {
      found = true;
    }
    escaped = false;
  }
  (found && quotes < 6).then_some(length)
}

/// `lex_string_literal`: up to the next `'`.
fn literal(text: &[u8]) -> Option<usize> {
  text.iter().position(|byte| *byte == b'\'').map(|at| at + 1)
}

/// `lex_multi_line_string_literal`: up to `'''`, with the quotes right after it.
fn multi_line_literal(text: &[u8]) -> Option<usize> {
  let (mut length, mut quotes, mut found) = (0, 0, false);
  for byte in text {
    if found {
      if *byte != b'\'' {
        return Some(length);
      }
      if quotes > 4 {
        return None;
      }
      quotes += 1;
      length += 1;
      continue;
    }
    length += 1;
    match *byte == b'\'' {
      true => quotes += 1,
      false => quotes = 0,
    }
    if quotes == 3 {
      found = true;
    }
  }
  found.then_some(length)
}

/// `true|false`
fn boolean(text: &[u8]) -> usize {
  match () {
    _ if text.starts_with(b"true") => 4,
    _ if text.starts_with(b"false") => 5,
    _ => 0,
  }
}

/// `0x[0-9A-Fa-f_]+`, `0o[0-7_]+` or `0b(0|1|_)+`
fn radix(text: &[u8], letter: u8, digit: impl Fn(u8) -> bool) -> usize {
  if text.len() < 3 || text[0] != b'0' || text[1] != letter {
    return 0;
  }
  match run(&text[2..], |byte| digit(byte) || byte == b'_') {
    0 => 0,
    digits => 2 + digits,
  }
}

fn sign(text: &[u8]) -> usize {
  matches!(text.first(), Some(b'+' | b'-')) as usize
}

fn digits(text: &[u8]) -> usize {
  run(text, |byte| byte.is_ascii_digit() || byte == b'_')
}

/// `[+-]?[0-9_]+`
fn integer(text: &[u8]) -> usize {
  let signed = sign(text);
  match digits(&text[signed..]) {
    0 => 0,
    digits => signed + digits,
  }
}

/// `[-+]?([0-9_]+(\.[0-9_]+)?([eE][+-]?[0-9_]+)?|nan|inf)`
fn float(text: &[u8]) -> usize {
  let signed = sign(text);
  let rest = &text[signed..];
  if rest.starts_with(b"nan") || rest.starts_with(b"inf") {
    return signed + 3;
  }
  let mut at = digits(rest);
  if at == 0 {
    return 0;
  }
  if rest.get(at) == Some(&b'.') {
    let fraction = digits(&rest[at + 1..]);
    if fraction > 0 {
      at += 1 + fraction;
    }
  }
  if matches!(rest.get(at), Some(b'e' | b'E')) {
    let exponent_sign = sign(&rest[at + 1..]);
    let exponent = digits(&rest[at + 1 + exponent_sign..]);
    if exponent > 0 {
      at += 1 + exponent_sign + exponent;
    }
  }
  signed + at
}

fn digit(text: &[u8], at: usize) -> Option<u8> {
  text.get(at).filter(|byte| byte.is_ascii_digit()).map(|byte| byte - b'0')
}

/// `DATE`: a year from 1000, and a day that the month has, February 29 only in leap years.
fn date(text: &[u8]) -> usize {
  let number = |at: usize, count: usize| -> Option<u32> {
    (at..at + count).try_fold(0, |value, at| Some(value * 10 + digit(text, at)? as u32))
  };
  let valid = (|| {
    let year = number(0, 4)?;
    let month = number(5, 2)?;
    let day = number(8, 2)?;
    if year < 1000 || text[4] != b'-' || text[7] != b'-' {
      return None;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
      1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
      4 | 6 | 9 | 11 => 30,
      2 if leap => 29,
      2 => 28,
      _ => return None,
    };
    (1..=days).contains(&day).then_some(())
  })();
  if valid.is_some() { 10 } else { 0 }
}

/// `TIME`: `HH:MM:SS`, with a fraction after `.` or `,`.
fn time(text: &[u8]) -> usize {
  let pair = |at: usize, max: u8| -> Option<u8> {
    let value = digit(text, at)? * 10 + digit(text, at + 1)?;
    (value <= max).then_some(value)
  };
  let valid = pair(0, 23).is_some()
    && text.get(2) == Some(&b':')
    && pair(3, 59).is_some()
    && text.get(5) == Some(&b':')
    && pair(6, 59).is_some();
  if !valid {
    return 0;
  }
  match text.get(8) {
    Some(b'.' | b',') => match run(&text[9..], |byte| byte.is_ascii_digit()) {
      0 => 8,
      fraction => 9 + fraction,
    },
    _ => 8,
  }
}

/// `DATE_TIME_OFFSET` with an offset, and `DATE_TIME_LOCAL` without one.
fn date_time(text: &[u8], offset: bool) -> usize {
  if date(text) == 0 || !matches!(text.get(10), Some(b'T' | b't' | b' ')) {
    return 0;
  }
  let time = time(&text[11..]);
  if time == 0 {
    return 0;
  }
  let end = 11 + time;
  if !offset {
    return end;
  }
  match text.get(end) {
    Some(b'Z' | b'z') => end + 1,
    Some(b'+' | b'-') => {
      let rest = &text[end + 1..];
      let hours = digit(rest, 0).is_some_and(|tens| tens <= 1) && digit(rest, 1).is_some();
      let minutes = rest.get(2) == Some(&b':')
        && digit(rest, 3).is_some_and(|tens| tens <= 5)
        && digit(rest, 4).is_some();
      if hours && minutes { end + 6 } else { 0 }
    }
    _ => 0,
  }
}
