/// What an ignore directive asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Directive {
  Next,
  Start,
  End,
  File,
}

/// Read `html` as `<!-- name -->`, `<!-- name-start -->`, `<!-- name-end -->`, or `<!-- name-file -->`. Each can
/// also name the tools it applies to, one of which is `format`, as `<!-- name-file format,lint -->`.
pub(crate) fn parse(html: &str, name: &str) -> Option<Directive> {
  let body = html.trim().strip_prefix("<!--")?.strip_suffix("-->")?.trim();
  let rest = body.strip_prefix(name)?;
  let (kind, tools) = rest.split_once(|it: char| it.is_ascii_whitespace()).unwrap_or((rest, ""));
  if !tools.is_empty() && !crate::directive::is_formatting(tools.as_bytes()) {
    return None;
  }
  match kind {
    "" => Some(Directive::Next),
    "-start" => Some(Directive::Start),
    "-end" => Some(Directive::End),
    "-file" => Some(Directive::File),
    _ => None,
  }
}

/// Find the `<!-- name-end -->` after `from`, as the range of its comment.
pub(crate) fn end(source: &str, from: usize, name: &str) -> Option<std::ops::Range<usize>> {
  let mut at = from;
  while let Some(offset) = source[at..].find("<!--") {
    let start = at + offset;
    let close = start + source[start..].find("-->")? + 3;
    if parse(&source[start..close], name) == Some(Directive::End) {
      return Some(start..close);
    }
    at = close;
  }
  None
}

/// Whether the file opens with `<!-- name-file -->`, after any front matter.
pub(crate) fn skips_file(source: &str, name: &str) -> bool {
  let mut text = source.trim_start();
  for fence in ["---", "+++"] {
    if let Some(rest) = text.strip_prefix(fence)
      && rest.starts_with(['\n', '\r'])
      && let Some(close) = rest.find(&format!("\n{fence}"))
    {
      text = rest[close + 1 + fence.len()..].trim_start();
      break;
    }
  }
  let Some(close) = text.find("-->") else { return false };
  parse(&text[..close + 3], name) == Some(Directive::File)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_each_form() {
    assert_eq!(parse("<!-- x -->\n", "x"), Some(Directive::Next));
    assert_eq!(parse("<!--x-start-->", "x"), Some(Directive::Start));
    assert_eq!(parse("<!--  x-end  -->", "x"), Some(Directive::End));
    assert_eq!(parse("<!-- x format -->", "x"), Some(Directive::Next));
    assert_eq!(parse("<!-- x-file lint,format -->", "x"), Some(Directive::File));
    assert_eq!(parse("<!-- x lint -->", "x"), None);
    assert_eq!(parse("<!-- x-file -->", "x"), Some(Directive::File));
    assert_eq!(parse("<!-- x-other -->", "x"), None);
    assert_eq!(parse("<!-- y -->", "x"), None);
  }

  #[test]
  fn finds_the_matching_end() {
    let source = "<!-- x-start -->\na <!-- other -->\n<!-- x-end -->\n";
    assert_eq!(end(source, 16, "x"), Some(34..48));
    assert_eq!(end(source, 16, "y"), None);
  }

  #[test]
  fn skips_files_after_front_matter() {
    assert!(skips_file("<!-- x-file -->\n# a", "x"));
    assert!(skips_file("---\na: 1\n---\n\n<!-- x-file -->\n", "x"));
    assert!(!skips_file("# a\n<!-- x-file -->", "x"));
  }
}
