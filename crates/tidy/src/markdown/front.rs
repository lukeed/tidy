/// Front matter at the very start of a document, found the way Prettier finds it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Front<'s> {
  /// The opening line: `---` or `+++`, and an optional language.
  pub(crate) open: &'s str,
  pub(crate) lang: &'s str,
  /// The lines between the delimiters, each with its line ending.
  pub(crate) text: &'s str,
  /// `---`, `+++`, or `...`, which also closes YAML.
  pub(crate) close: &'s str,
  /// Where the rest of the document starts.
  pub(crate) end: usize,
}

/// The front matter that `source` starts with, if any.
pub(crate) fn split(source: &str) -> Option<Front<'_>> {
  let delimiter = source.get(..3).filter(|start| matches!(*start, "---" | "+++"))?;
  let line = source.find('\n')?;
  let named = source[3..line].trim();
  let lang = match named {
    "" if delimiter == "+++" => "toml",
    "" => "yaml",
    named => named,
  };
  let search = |close: &str| source[line..].find(&format!("\n{close}")).map(|at| line + at);
  let (at, close) = match search(delimiter) {
    Some(at) => (at, delimiter),
    None if delimiter == "---" && lang == "yaml" => (search("...")?, "..."),
    None => return None,
  };
  Some(Front {
    open: source[..line].trim_end(),
    lang,
    text: &source[line + 1..at + 1],
    close,
    end: at + 4,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn finds_front_matter() {
    let front = split("---\na: 1\n---\nx\n").expect("front matter");
    assert_eq!((front.open, front.lang, front.text, front.close), ("---", "yaml", "a: 1\n", "---"));
    assert_eq!(front.end, 12);
    assert_eq!(split("+++\n+++\n").map(|front| (front.lang, front.text)), Some(("toml", "")));
    assert_eq!(split("---json\n{}\n...\n").map(|front| front.close), None);
    assert_eq!(split("---\na\n...\n").map(|front| front.close), Some("..."));
    assert_eq!(split("x\n---\na\n---\n"), None);
    assert_eq!(split("---\na\n"), None);
  }
}
