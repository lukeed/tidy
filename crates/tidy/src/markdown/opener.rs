/// What a paragraph's continuation line needs once its indent is gone, so it doesn't start a
/// block that the source didn't have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fix {
  /// Join it to the line before with a space, as Prettier does.
  Join,
  /// Escape a line of `=` or `-`, which would underline a setext heading, as Prettier does.
  Escape,
}

/// The fix for a continuation line that starts with `line`, if it could interrupt a paragraph.
pub(crate) fn interrupts(line: &str) -> Option<Fix> {
  let line = line.trim_end();
  let bytes = line.as_bytes();
  let &first = bytes.first()?;
  let run = bytes.iter().take_while(|&&byte| byte == first).count();
  let after = bytes.get(run).copied();
  let spaced = matches!(after, None | Some(b' ' | b'\t'));
  let joins = match first {
    b'=' | b'-' if run == bytes.len() => return Some(Fix::Escape),
    b'>' => true,
    b'#' => run <= 6 && spaced,
    b'-' | b'+' | b'*' if run == 1 && spaced => true,
    b'-' | b'*' | b'_' => rule(line),
    b'`' | b'~' => run >= 3,
    b'<' => html(line),
    byte if byte.is_ascii_digit() => {
      let digits = bytes.iter().take_while(|byte| byte.is_ascii_digit()).count();
      let after = bytes.get(digits + 1).copied();
      digits <= 9
        && matches!(bytes.get(digits), Some(b'.' | b')'))
        && matches!(after, None | Some(b' ' | b'\t'))
    }
    _ => false,
  };
  joins.then_some(Fix::Join)
}

/// Whether `line` is a thematic break: three or more of one marker, with only spaces between.
fn rule(line: &str) -> bool {
  let mut marks = line.bytes().filter(|byte| !matches!(byte, b' ' | b'\t'));
  let Some(first) = marks.next() else { return false };
  let rest: Vec<u8> = marks.collect();
  rest.len() >= 2 && rest.iter().all(|&byte| byte == first)
}

/// Whether `line` starts an HTML block that can interrupt a paragraph: CommonMark's kinds 1 to 6.
fn html(line: &str) -> bool {
  let rest = &line[1..];
  if rest.starts_with("!--") || rest.starts_with('?') || rest.starts_with("![CDATA[") {
    return true;
  }
  if let Some(declaration) = rest.strip_prefix('!') {
    return declaration.starts_with(|char: char| char.is_ascii_alphabetic());
  }
  let name = rest.strip_prefix('/').unwrap_or(rest);
  let length = name.bytes().take_while(u8::is_ascii_alphanumeric).count();
  let tag = name[..length].to_ascii_lowercase();
  let ends = matches!(name.as_bytes().get(length), None | Some(b' ' | b'\t' | b'>'))
    || name[length..].starts_with("/>");
  let raw = !rest.starts_with('/') && RAW.contains(&tag.as_str());
  ends && (raw || BLOCK.contains(&tag.as_str()))
}

/// Whether `line` holds nothing but a Liquid or Hugo tag, `{{ … }}` or `{% … %}`, which Prettier's
/// parser reads as a block of its own.
pub(crate) fn liquid(line: &str) -> bool {
  let line = line.trim();
  let close = match line.get(..2) {
    Some("{{") => "}}",
    Some("{%") => "%}",
    _ => return false,
  };
  line[2..].find(close).is_some_and(|at| at + 4 == line.len())
}

/// Whether `line` starts an HTML block where nothing needs interrupting: CommonMark's kinds 1 to 7.
pub(crate) fn starts_html(line: &str) -> bool {
  let line = line.trim_start();
  line.starts_with('<') && (html(line) || complete(line))
}

/// Whether `line` is a complete open or closing tag alone, CommonMark's seventh kind.
fn complete(line: &str) -> bool {
  let rest = &line[1..];
  if !rest.strip_prefix('/').unwrap_or(rest).starts_with(|char: char| char.is_ascii_alphabetic()) {
    return false;
  }
  let mut quote = None;
  for (index, char) in line.char_indices() {
    match (quote, char) {
      (Some(open), char) if char == open => quote = None,
      (Some(_), _) => {}
      (None, '"' | '\'') => quote = Some(char),
      (None, '>') => return line[index + 1..].trim().is_empty(),
      _ => {}
    }
  }
  false
}

/// Tags whose HTML block runs to its closing tag.
const RAW: [&str; 4] = ["pre", "script", "style", "textarea"];

/// Tags that start an HTML block of CommonMark's sixth kind.
const BLOCK: [&str; 62] = [
  "address",
  "article",
  "aside",
  "base",
  "basefont",
  "blockquote",
  "body",
  "caption",
  "center",
  "col",
  "colgroup",
  "dd",
  "details",
  "dialog",
  "dir",
  "div",
  "dl",
  "dt",
  "fieldset",
  "figcaption",
  "figure",
  "footer",
  "form",
  "frame",
  "frameset",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "head",
  "header",
  "hr",
  "html",
  "iframe",
  "legend",
  "li",
  "link",
  "main",
  "menu",
  "menuitem",
  "nav",
  "noframes",
  "ol",
  "optgroup",
  "option",
  "p",
  "param",
  "search",
  "section",
  "summary",
  "table",
  "tbody",
  "td",
  "tfoot",
  "th",
  "thead",
  "title",
  "tr",
  "track",
  "ul",
];

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn finds_lines_that_open_blocks() {
    for line in ["# h", "######", "- x", "+", "1. x", "123456789) x", "> q", "```js", "~~~"] {
      assert_eq!(interrupts(line), Some(Fix::Join), "{line}");
    }
    for line in ["***", "_ _ _", "* * *", "<div>", "</div>", "<DIV class=x>", "<hr/>", "<!-- c"] {
      assert_eq!(interrupts(line), Some(Fix::Join), "{line}");
    }
    for line in ["===", "---", "-", "="] {
      assert_eq!(interrupts(line), Some(Fix::Escape), "{line}");
    }
    for line in ["#h", "1234567890. x", "2.x", "-x", "``", "<span>", "<divx>", "a", "*a*"] {
      assert_eq!(interrupts(line), None, "{line}");
    }
  }

  #[test]
  fn finds_liquid_and_html_lines() {
    for line in ["{{ x }}", "  {{- end -}} ", "{% if a %}", "{{< callout >}}"] {
      assert!(liquid(line), "{line}");
    }
    for line in ["{{ x }} y", "a {{ x }}", "{{ x }} {{ y }}", "{x}", "{% x }}"] {
      assert!(!liquid(line), "{line}");
    }
    for line in ["<div>", "<button type=\"a>b\">", "</span>", "<!-- c"] {
      assert!(starts_html(line), "{line}");
    }
    for line in ["<span>x</span>", "<1>", "text"] {
      assert!(!starts_html(line), "{line}");
    }
  }
}
