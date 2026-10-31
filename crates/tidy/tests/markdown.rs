use mdtidy::{Bullet, Emphasis, Fence, Heading, Numbering, Options, Preset, Rule, Strong, Wrap};

fn prettier(source: &str) -> String {
  mdtidy::format(source, &Options::default())
}

fn preserve(source: &str) -> String {
  mdtidy::format(source, &Options::from(Preset::Preserve))
}

#[test]
fn separates_blocks_with_one_blank_line() {
  assert_eq!(prettier("# a\ntext\n\n\n\n> quote\n"), "# a\n\ntext\n\n> quote\n");
}

#[test]
fn turns_setext_headings_into_atx() {
  assert_eq!(prettier("Title\n=====\n\nSub\n---\n"), "# Title\n\n## Sub\n");
  assert_eq!(preserve("Title\n=====\n"), "Title\n=====\n");
  assert_eq!(prettier("# Closed #\n"), "# Closed\n");
}

#[test]
fn normalizes_bullets_and_alternates_adjacent_lists() {
  assert_eq!(prettier("* a\n* b\n"), "- a\n- b\n");
  assert_eq!(prettier("- a\n\n* b\n"), "- a\n\n* b\n");
  assert_eq!(prettier("+ a\n\n- b\n\n* c\n"), "- a\n\n* b\n\n- c\n");
  assert_eq!(preserve("+ a\n+ b\n"), "+ a\n+ b\n");
  let options = Options { bullet: Bullet::Plus, ..Options::default() };
  assert_eq!(mdtidy::format("- a\n", &options), "+ a\n");
}

#[test]
fn numbers_ordered_lists_like_prettier() {
  assert_eq!(prettier("1. a\n2. b\n3. c\n"), "1. a\n2. b\n3. c\n");
  assert_eq!(prettier("1. a\n1. b\n1. c\n"), "1. a\n1. b\n1. c\n");
  assert_eq!(prettier("3. a\n9. b\n"), "3. a\n4. b\n");
  assert_eq!(prettier("0. a\n1. b\n1. c\n"), "0. a\n1. b\n1. c\n");
  assert_eq!(prettier("1) a\n2) b\n"), "1. a\n2. b\n");
  assert_eq!(prettier("1. a\n\n1) b\n"), "1. a\n\n1) b\n");
  assert_eq!(preserve("3) a\n9) b\n"), "3) a\n9) b\n");
  let options = Options { numbering: Numbering::Keep, ..Options::default() };
  assert_eq!(mdtidy::format("3. a\n9. b\n", &options), "3. a\n9. b\n");
}

#[test]
fn indents_list_content_to_the_marker() {
  assert_eq!(prettier("*   a\n\n    b\n"), "- a\n\n  b\n");
  assert_eq!(prettier("10. a\n\n    b\n"), "10. a\n\n    b\n");
  assert_eq!(prettier("- a\n  - b\n    - c\n"), "- a\n  - b\n    - c\n");
}

#[test]
fn keeps_lists_tight_or_loose() {
  assert_eq!(prettier("- a\n- b\n"), "- a\n- b\n");
  assert_eq!(prettier("- a\n\n- b\n"), "- a\n\n- b\n");
  assert_eq!(prettier("- ```\n  x\n  ```\n\n- b\n"), "- ```\n  x\n  ```\n\n- b\n");
}

#[test]
fn chooses_emphasis_delimiters() {
  assert_eq!(prettier("*a* and __b__\n"), "_a_ and **b**\n");
  assert_eq!(prettier("a*b*c\n"), "a*b*c\n");
  assert_eq!(prettier("*a *b* c*\n"), "_a *b* c_\n");
  assert_eq!(preserve("*a* and __b__\n"), "*a* and __b__\n");
  let options =
    Options { emphasis: Emphasis::Asterisk, strong: Strong::Underscore, ..Options::default() };
  assert_eq!(mdtidy::format("_a_ **b** c**d**\n", &options), "*a* __b__ c**d**\n");
  assert_eq!(prettier("~a~\n"), "~~a~~\n");
}

#[test]
fn writes_thematic_breaks() {
  assert_eq!(prettier("a\n\n***\n"), "a\n\n---\n");
  assert_eq!(prettier("- a\n\n  ___\n"), "- a\n\n  ***\n");
  assert_eq!(preserve("_ _ _\n"), "_ _ _\n");
  let options = Options { rule: Rule::Underscore, ..Options::default() };
  assert_eq!(mdtidy::format("---\n", &options), "___\n");
}

#[test]
fn fences_code_longer_than_its_content() {
  assert_eq!(prettier("~~~js\nx\n~~~\n"), "```js\nx\n```\n");
  assert_eq!(prettier("~~~\n```\n~~~\n"), "````\n```\n````\n");
  assert_eq!(prettier("```js   title=\"a\"\nx\n```\n"), "```js title=\"a\"\nx\n```\n");
  assert_eq!(preserve("~~~~js\nx\n~~~~\n"), "~~~~js\nx\n~~~~\n");
  let options = Options { fence: Fence::Tilde, ..Options::default() };
  assert_eq!(mdtidy::format("```\nx\n```\n", &options), "~~~\nx\n~~~\n");
  assert_eq!(prettier("    indented\n"), "    indented\n");
}

#[test]
fn keeps_code_whitespace() {
  assert_eq!(prettier("```\na\n   \nb\n```\n"), "```\na\n   \nb\n```\n");
  assert_eq!(prettier("> ```\n> a\n>\n> b\n> ```\n"), "> ```\n> a\n>\n> b\n> ```\n");
}

#[test]
fn aligns_tables() {
  let source = "|a|b|\n|-|:-:|\n|long|x|\n";
  assert_eq!(prettier(source), "| a    |  b  |\n| ---- | :-: |\n| long |  x  |\n");
}

#[test]
fn copies_inline_content_as_written() {
  let source = "\\# not a heading, &amp; [link][ref] `code`  \nnext <b>html</b>\n\n[ref]: /url\n";
  assert_eq!(prettier(source), source);
  assert_eq!(prettier("[a][]\n\n[a]: /b\n"), "[a][]\n\n[a]: /b\n");
}

#[test]
fn fences_code_spans_with_the_fewest_backticks() {
  assert_eq!(prettier("`` ```ts `` and ``` a``b ```\n"), "` ```ts ` and `a``b`\n");
  assert_eq!(prettier("` a ` and `` `a` `` and ``a`b``\n"), "`a` and `` `a` `` and ``a`b``\n");
  assert_eq!(prettier("``  a  ``\n"), "`  a  `\n");
  assert_eq!(prettier("| `a\\|b` |\n| - |\n"), "| `a\\|b` |\n| ------ |\n");
}

#[test]
fn quotes_and_nesting() {
  assert_eq!(prettier(">a\n>\n>b\n"), "> a\n>\n> b\n");
  assert_eq!(prettier("> - a\n>   > b\n"), "> - a\n>   > b\n");
}

#[test]
fn keeps_front_matter() {
  assert_eq!(prettier("---\na:   1\n---\n# x\n"), "---\na:   1\n---\n\n# x\n");
}

#[test]
fn wraps_paragraphs() {
  let source = "one two three\nfour five six seven\n";
  let always = Options { wrap: Wrap::Always, width: 14, ..Options::default() };
  assert_eq!(mdtidy::format(source, &always), "one two three\nfour five six\nseven\n");
  let never = Options { wrap: Wrap::Never, ..Options::default() };
  assert_eq!(mdtidy::format(source, &never), "one two three four five six seven\n");
  assert_eq!(mdtidy::format("- one\n  two\n", &never), "- one two\n");
  assert_eq!(mdtidy::format("one\\\ntwo\n", &never), "one\\\ntwo\n");
  let html = "- a:\n   <details>\n     <x>\n\n     - b\n";
  assert_eq!(mdtidy::format(html, &never), "- a:\n  <details>\n     <x>\n\n  - b\n");
}

#[test]
fn never_wraps_into_a_block() {
  let options = Options { wrap: Wrap::Always, width: 5, ..Options::default() };
  assert_eq!(mdtidy::format("aaaa - b\n", &options), "aaaa -\nb\n");
  assert_eq!(mdtidy::format("aaaa # b\n", &options), "aaaa #\nb\n");
  assert_eq!(mdtidy::format("aaaa 1. b\n", &options), "aaaa 1.\nb\n");
}

#[test]
fn ignores_the_next_block() {
  let source = "<!-- prettier-ignore -->\n|a|b|\n|-|-|\n\n* x\n";
  assert_eq!(prettier(source), "<!-- prettier-ignore -->\n|a|b|\n|-|-|\n\n- x\n");
  let mdtidy = "<!-- mdtidy-ignore -->\n* x\n\n* y\n";
  assert_eq!(preserve(mdtidy), mdtidy);
}

#[test]
fn ignores_ranges() {
  let source =
    "<!-- prettier-ignore-start -->\n* a\n\n\n* b\n<!-- prettier-ignore-end -->\n\n* c\n";
  assert_eq!(
    prettier(source),
    "<!-- prettier-ignore-start -->\n* a\n\n\n* b\n<!-- prettier-ignore-end -->\n\n- c\n"
  );
}

#[test]
fn ignores_files() {
  let source = "<!-- prettier-ignore-file -->\n* a\n";
  assert_eq!(prettier(source), source);
  let custom = Options { ignore: "starship-ignore".into(), ..Options::default() };
  assert_eq!(
    mdtidy::format("<!-- starship-ignore-file -->\n* a\n", &custom),
    "<!-- starship-ignore-file -->\n* a\n"
  );
}

#[test]
fn hands_code_to_the_caller() {
  let mut seen = Vec::new();
  let output = mdtidy::format_with("- ```js\n  a\n  ```\n", &Options::default(), |code| {
    seen.push((code.lang.to_string(), code.width));
    Some(code.text.to_uppercase())
  });
  assert_eq!(output, "- ```js\n  A\n  ```\n");
  assert_eq!(seen, [("js".to_string(), 78)]);
}

#[test]
fn checks() {
  assert!(mdtidy::check("- a\n", &Options::default()));
  assert!(!mdtidy::check("* a\n", &Options::default()));
}

#[test]
fn keeps_headings_with_keep() {
  let options = Options { heading: Heading::Keep, ..Options::default() };
  assert_eq!(mdtidy::format("A\nB\n===\n", &options), "A\nB\n===\n");
  assert_eq!(prettier("A\nB\n===\n"), "# A B\n");
  assert_eq!(mdtidy::format("> A\n> ---\n", &options), "> A\n> ---\n");
}
