//! Prettier's `main/front-matter/parse.js`.

pub(crate) struct FrontMatter<'a> {
  pub(crate) language: &'a str,
  pub(crate) explicit: Option<&'a str>,
  pub(crate) value: &'a str,
  pub(crate) start: &'a str,
  pub(crate) end: &'a str,
  /// The length of the whole front matter, delimiters included.
  pub(crate) len: usize,
}

/// `getFrontMatter`: the front matter that `text` starts with.
pub(crate) fn parse(text: &str) -> Option<FrontMatter<'_>> {
  let start = text.get(..3).filter(|start| matches!(*start, "---" | "+++"))?;
  let first = 3 + text[3..].find('\n')?;
  let explicit = text[3..first].trim();
  let language = match (explicit, start) {
    ("", "+++") => "toml",
    ("", _) => "yaml",
    (language, _) => language,
  };
  let closing = format!("\n{start}");
  let mut end = text[first..].find(&closing).map(|at| first + at);
  if end.is_none() && start == "---" && language == "yaml" {
    end = text[first..].find("\n...").map(|at| first + at);
  }
  let end = end?;
  let len = end + 1 + 3;
  Some(FrontMatter {
    language,
    explicit: (!explicit.is_empty()).then_some(explicit),
    value: text.get(first + 1..end).unwrap_or_default(),
    start,
    end: &text[len - 3..len],
    len,
  })
}
