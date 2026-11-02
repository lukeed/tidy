//! Angular's microsyntax, the value of `*directive="..."`: the bindings that `angular-estree-parser` reads into an
//! `NGMicrosyntax`, with the code of their expressions left for the host to format.

/// A binding of the microsyntax.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Binding<'v> {
  /// `NGMicrosyntaxExpression`: the directive's own expression, and its alias after `as`.
  Expression { code: &'v str, alias: Option<&'v str> },
  /// `NGMicrosyntaxKeyedExpression`
  Keyed { key: &'v str, code: &'v str, alias: Option<&'v str> },
  /// `NGMicrosyntaxLet`
  Let { key: &'v str, value: Option<&'v str> },
  /// `NGMicrosyntaxAs`
  As { key: &'v str, alias: &'v str },
}

impl Binding<'_> {
  fn key(&self) -> Option<&str> {
    match *self {
      Binding::Expression { .. } => None,
      Binding::Keyed { key, .. } | Binding::Let { key, .. } | Binding::As { key, .. } => Some(key),
    }
  }
}

/// The bindings of `value`, as Angular's `parseTemplateBindings` reads them, or `None` if it can't tell where they
/// are.
pub(crate) fn parse(value: &str) -> Option<Vec<Binding<'_>>> {
  let mut reader = Reader { text: value, at: 0 };
  let mut bindings = Vec::new();
  loop {
    reader.skip_terminators();
    if reader.at == value.len() {
      return (!bindings.is_empty()).then_some(bindings);
    }
    if reader.keyword("let") {
      let key = reader.key()?;
      reader.skip_spaces();
      let value = match reader.eat(b'=') {
        true => Some(reader.key()?),
        false => None,
      };
      bindings.push(Binding::Let { key, value });
      continue;
    }
    if bindings.is_empty() {
      let code = reader.expression()?;
      let alias = reader.alias()?;
      bindings.push(Binding::Expression { code, alias });
      continue;
    }
    let key = reader.key()?;
    reader.skip_spaces();
    if reader.keyword("as") {
      let alias = reader.key()?;
      bindings.push(Binding::As { key, alias });
      continue;
    }
    reader.eat(b':');
    let code = reader.expression()?;
    let alias = reader.alias()?;
    bindings.push(Binding::Keyed { key, code, alias });
  }
}

/// How the binding at `index` is printed after the one before it: `NGMicrosyntax` and
/// `NGMicrosyntaxKeyedExpression` of Prettier's `printAngular`. Returns whether it follows a space instead of `;` and a
/// line, and whether a keyed expression has no `:` after its key.
pub(crate) fn layout(bindings: &[Binding<'_>], index: usize) -> (bool, bool) {
  let Binding::Keyed { key, .. } = bindings[index] else { return (false, false) };
  let is_for_of = key == "of" && index == 1;
  let is_for_of_track = bindings.get(1).and_then(Binding::key) == Some("of") && key == "track";
  let follows_expression = matches!(bindings[0], Binding::Expression { .. });
  let after_then = || matches!(bindings[index - 1], Binding::Keyed { key: "then", .. });
  let is_branch = (index == 1 && matches!(key, "then" | "else" | "as"))
    || (index == 2 && ((key == "else" && after_then()) || key == "track"));
  (is_for_of, is_for_of || is_for_of_track || (is_branch && follows_expression))
}

/// `NGMicrosyntaxKey`: the name as it is if it looks like one, or else as a JSON string.
pub(crate) fn key(name: &str) -> String {
  match is_plain_key(name.as_bytes()) {
    true => name.to_owned(),
    false => {
      let mut quoted = String::with_capacity(name.len() + 2);
      quoted.push('"');
      for char in name.chars() {
        match char {
          '"' => quoted.push_str("\\\""),
          '\\' => quoted.push_str("\\\\"),
          _ => quoted.push(char),
        }
      }
      quoted.push('"');
      quoted
    }
  }
}

/// `/^[$_a-z][\w$]*(?:-[$_a-z][\w$])*$/i`
fn is_plain_key(name: &[u8]) -> bool {
  let is_start = |byte: u8| byte.is_ascii_alphabetic() || matches!(byte, b'$' | b'_');
  let is_part = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'$' | b'_');
  let Some((&first, rest)) = name.split_first() else { return false };
  let words = rest.iter().take_while(|&&byte| is_part(byte)).count();
  is_start(first)
    && rest[words..]
      .chunks(3)
      .all(|chunk| matches!(chunk, [b'-', start, part] if is_start(*start) && is_part(*part)))
}

struct Reader<'v> {
  text: &'v str,
  at: usize,
}

impl<'v> Reader<'v> {
  fn bytes(&self) -> &'v [u8] {
    self.text.as_bytes()
  }

  fn skip_spaces(&mut self) {
    let rest = &self.bytes()[self.at..];
    self.at += rest.iter().take_while(|byte| byte.is_ascii_whitespace()).count();
  }

  /// `consumeStatementTerminator`, and the white space around.
  fn skip_terminators(&mut self) {
    self.skip_spaces();
    while self.eat(b';') || self.eat(b',') {}
  }

  /// Skips `byte` and the white space after it, if it comes next.
  fn eat(&mut self, byte: u8) -> bool {
    if self.bytes().get(self.at) != Some(&byte) {
      return false;
    }
    self.at += 1;
    self.skip_spaces();
    true
  }

  /// Skips `word` and the white space after it, if it comes next as a whole word.
  fn keyword(&mut self, word: &str) -> bool {
    let rest = &self.bytes()[self.at..];
    if !rest.starts_with(word.as_bytes()) || rest.get(word.len()).is_some_and(|&byte| is_part(byte))
    {
      return false;
    }
    self.at += word.len();
    self.skip_spaces();
    true
  }

  /// `expectTemplateBindingKey`: words joined by `-`.
  fn key(&mut self) -> Option<&'v str> {
    let start = self.at;
    loop {
      let rest = &self.bytes()[self.at..];
      let length = identifier(rest);
      if length == 0 {
        return None;
      }
      self.at += length;
      match self.bytes().get(self.at..self.at + 2) {
        Some([b'-', next]) if is_start(*next) => self.at += 1,
        _ => break,
      }
    }
    let key = &self.text[start..self.at];
    self.skip_spaces();
    Some(key)
  }

  /// `as` and a key, after an expression.
  fn alias(&mut self) -> Option<Option<&'v str>> {
    match self.keyword("as") {
      true => self.key().map(Some),
      false => Some(None),
    }
  }

  /// The code of an expression, up to what can't continue it: `;`, `,`, a bracket that closes nothing, or a word
  /// right after an operand, as the key of the next binding or `as`.
  fn expression(&mut self) -> Option<&'v str> {
    let bytes = self.bytes();
    let start = self.at;
    let mut depth = 0usize;
    let mut operand = false;
    let mut at = start;
    while at < bytes.len() {
      let byte = bytes[at];
      match byte {
        b'\'' | b'"' | b'`' => {
          at += string_length(&bytes[at..])?;
          operand = true;
          continue;
        }
        b'(' | b'[' | b'{' => depth += 1,
        b')' | b']' | b'}' if depth == 0 => break,
        b')' | b']' | b'}' => depth -= 1,
        b';' | b',' if depth == 0 => break,
        _ if byte.is_ascii_whitespace() => {
          let next = at + bytes[at..].iter().take_while(|byte| byte.is_ascii_whitespace()).count();
          let word = &bytes[next..next + identifier(&bytes[next..])];
          if depth == 0 && operand && !word.is_empty() && !matches!(word, b"in" | b"instanceof") {
            break;
          }
          at = next;
          continue;
        }
        _ => {}
      }
      operand = is_part(byte) || matches!(byte, b')' | b']' | b'}');
      at += 1;
    }
    self.at = at;
    let code = self.text[start..at].trim_end();
    self.skip_spaces();
    (!code.is_empty() && depth == 0).then_some(code)
  }
}

fn is_start(byte: u8) -> bool {
  byte.is_ascii_alphabetic() || matches!(byte, b'$' | b'_')
}

fn is_part(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || matches!(byte, b'$' | b'_')
}

/// The length of the identifier at the start of `bytes`, or 0.
fn identifier(bytes: &[u8]) -> usize {
  match bytes.first() {
    Some(&first) if is_start(first) => {
      1 + bytes[1..].iter().take_while(|&&byte| is_part(byte)).count()
    }
    _ => 0,
  }
}

/// The length of the string at the start of `bytes`, with its quotes.
fn string_length(bytes: &[u8]) -> Option<usize> {
  let quote = bytes[0];
  let mut at = 1;
  while at < bytes.len() {
    match bytes[at] {
      b'\\' => at += 2,
      byte if byte == quote => return Some(at + 1),
      _ => at += 1,
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use super::Binding::*;
  use super::*;

  #[test]
  fn reads_bindings() {
    assert_eq!(
      parse("let item  of items;index as i; let e=even").unwrap(),
      [
        Let { key: "item", value: None },
        Keyed { key: "of", code: "items", alias: None },
        As { key: "index", alias: "i" },
        Let { key: "e", value: Some("even") },
      ]
    );
    assert_eq!(
      parse("user$ | async as user; else loading").unwrap(),
      [
        Expression { code: "user$ | async", alias: Some("user") },
        Keyed { key: "else", code: "loading", alias: None },
      ]
    );
    assert_eq!(
      parse("cond, then a else b").unwrap(),
      [
        Expression { code: "cond", alias: None },
        Keyed { key: "then", code: "a", alias: None },
        Keyed { key: "else", code: "b", alias: None },
      ]
    );
    assert_eq!(
      parse("tpl; context: {a:1, b: (c ? d : e)}").unwrap(),
      [
        Expression { code: "tpl", alias: None },
        Keyed { key: "context", code: "{a:1, b: (c ? d : e)}", alias: None },
      ]
    );
    assert_eq!(parse("a ? b : c").unwrap(), [Expression { code: "a ? b : c", alias: None }]);
    assert_eq!(parse("x;").unwrap(), [Expression { code: "x", alias: None }]);
    assert_eq!(parse("a)"), None);
    assert_eq!(parse(" ; "), None);
  }

  #[test]
  fn prints_keys() {
    assert_eq!(key("ngFor"), "ngFor");
    assert_eq!(key("a-bc"), "a-bc");
    assert_eq!(key("a-b"), "\"a-b\"");
  }
}
