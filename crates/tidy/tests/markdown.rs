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
fn spreads_loose_items_like_prettier() {
  let cases = [
    ("- a\n  ```\n  x\n  ```\n\n  b\n- c\n", "- a\n\n  ```\n  x\n  ```\n\n  b\n\n- c\n"),
    ("- a\n  > q\n\n  > r\n- d\n", "- a\n\n  > q\n\n  > r\n\n- d\n"),
    ("- a\n  - b\n\n  c\n- d\n", "- a\n  - b\n\n  c\n\n- d\n"),
    ("- a\n\n  b\n  > q\n", "- a\n\n  b\n\n  > q\n"),
    ("1. a\n   ```\n   x\n   ```\n\n2. b\n", "1. a\n\n   ```\n   x\n   ```\n\n2. b\n"),
  ];
  for (source, expected) in cases {
    assert_eq!(prettier(source), expected, "{source}");
  }
  for source in [
    "- a\n  - b\n\n  - c\n- d\n",
    "1. a\n   ```\n   x\n\n   y\n   ```\n2. b\n",
    "- a\n  <div>\n\n  b\n",
  ] {
    assert_eq!(prettier(source), source, "{source}");
  }
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
  assert_eq!(prettier("x ***_a_*** y\n"), "x _**_a_**_ y\n");
  assert_eq!(prettier("_a **b *c*** d_\n"), "_a **b *c*** d_\n");
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
fn trims_code_line_ends() {
  assert_eq!(prettier("```\na  \n   \n\tb\t\n```\n"), "```\na\n\n\tb\n```\n");
  assert_eq!(prettier("> ```\n> a \n>\n> b\n> ```\n"), "> ```\n> a\n>\n> b\n> ```\n");
  assert_eq!(prettier("    a  \n\n    b\n"), "    a\n\n    b\n");
  assert_eq!(prettier("<div>  \n</div>\n"), "<div>  \n</div>\n");
}

#[test]
fn aligns_tables() {
  let source = "|a|b|\n|-|:-:|\n|long|x|\n";
  assert_eq!(prettier(source), "| a    |  b  |\n| ---- | :-: |\n| long |  x  |\n");
}

#[test]
fn keeps_each_rows_cells() {
  let source = "| a | b | c |\n| - | - | - |\n| x | y |\n| 1 | 2 | 3 | 4 |\n";
  let expected =
    "| a   | b   | c   |\n| --- | --- | --- |\n| x   | y   |\n| 1   | 2   | 3   | 4   |\n";
  assert_eq!(prettier(source), expected);
  let split = "| a | b |\n| - | - |\n| x `n|m` y | z |\n";
  assert_eq!(prettier(split), "| a    | b    |\n| ---- | ---- |\n| x `n | m` y | z   |\n");
  assert_eq!(prettier("| a | b |\n| - | - |\n| c\n"), "| a   | b   |\n| --- | --- |\n| c   |\n");
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
fn collapses_spaces_in_text() {
  assert_eq!(prettier("a  b\tc\n"), "a b c\n");
  assert_eq!(prettier("# a   b\n"), "# a b\n");
  assert_eq!(prettier("a  `x  y`  *b  c*\n"), "a `x  y` _b c_\n");
  assert_eq!(prettier("| a  b |\n| - |\n"), "| a b |\n| --- |\n");
  assert_eq!(preserve("a  b\n"), "a b\n");
}

#[test]
fn formats_link_text_like_other_text() {
  assert_eq!(prettier("[a  *b*](u \"t  t\")\n"), "[a _b_](u \"t  t\")\n");
  assert_eq!(prettier("[![a  b](i)  c](u)\n"), "[![a  b](i) c](u)\n");
  assert_eq!(prettier("[c  d][r] [r][]\n\n[r]: /r\n"), "[c d][r] [r][]\n\n[r]: /r\n");
  assert_eq!(prettier("<http://a.b>  x [](u)\n"), "<http://a.b> x [](u)\n");
  assert_eq!(prettier("- [a\n  ](u)\n"), "- [a\n  ](u)\n");
  assert_eq!(prettier("> [a\n> ](u)\n"), "> [a\n> ](u)\n");
  assert_eq!(prettier("[![a][]][b]\n\n[a]: /a\n[b]: /b\n"), "[![a][]][b]\n\n[a]: /a\n[b]: /b\n");
  let never = Options { wrap: Wrap::Always, width: 6, ..Options::default() };
  assert_eq!(mdtidy::format("x [a b\nc](u) y\n", &never), "x\n[a b c](u)\ny\n");
}

#[test]
fn keeps_definitions_together() {
  let source = "[a]: /a\n\n[b]: /b\n\ntext\n\n[c]: /c\n";
  assert_eq!(prettier(source), "[a]: /a\n[b]: /b\n\ntext\n\n[c]: /c\n");
  assert_eq!(prettier("[a]: /a\n# h\n[b]: /b\n"), "[a]: /a\n\n# h\n\n[b]: /b\n");
}

#[test]
fn hangs_task_items_after_the_box() {
  assert_eq!(prettier("- [ ] one\n  two\n- [x] three\n"), "- [ ] one\n      two\n- [x] three\n");
  assert_eq!(
    prettier("1. [x] a\n   b\n\n> - [ ] q\n>   r\n"),
    "1. [x] a\n       b\n\n> - [ ] q\n>       r\n"
  );
  assert_eq!(prettier("- [ ]   a\n"), "- [ ] a\n");
  assert_eq!(prettier("- [ ] a\n  - [ ] b\n- [ ] c\n"), "- [ ] a\n  - [ ] b\n- [ ] c\n");
  let source = "- [ ] a\n\n  <div>x\n  y</div>\n\n  p\n  q\n\n  - n\n    m\n";
  assert_eq!(prettier(source), source);
  let always = Options { wrap: Wrap::Always, width: 14, ..Options::default() };
  assert_eq!(mdtidy::format("- [ ] aaa bbb ccc ddd\n", &always), "- [ ] aaa bbb\n      ccc ddd\n");
}

#[test]
fn keeps_html_beside_a_paragraph_or_html() {
  for source in [
    "para\n<div>y</div>\n",
    "<script>\nx\n</script>\n<svg-image />\n",
    "> para\n> <div>x</div>\n",
    "para\n<div>\n\n<!-- c -->\n<!-- d -->\n",
  ] {
    assert_eq!(prettier(source), source);
    assert_eq!(preserve(source), source);
  }
  assert_eq!(prettier("<!-- a -->\ntext\n"), "<!-- a -->\n\ntext\n");
  assert_eq!(prettier("# h\n<div>x</div>\n"), "# h\n\n<div>x</div>\n");
  assert_eq!(prettier("- a\n<div>x</div>\n"), "- a\n\n<div>x</div>\n");
}

#[test]
fn keeps_indented_lines_from_opening_blocks() {
  for (line, joined) in [
    ("# h", true),
    ("- x", true),
    ("1. x", true),
    ("> q", true),
    ("```", true),
    ("***", true),
    ("<div>", true),
    ("<!-- c -->", true),
    ("<span>", false),
    ("plain", false),
  ] {
    let expected = if joined { format!("text {line}\n") } else { format!("text\n{line}\n") };
    assert_eq!(prettier(&format!("text\n    {line}\n")), expected, "{line}");
  }
  assert_eq!(prettier("text\n    ===\n\nmore\n    -\n"), "text\n\\===\n\nmore\n\\-\n");
  assert_eq!(prettier("> a\n    > b\n"), "> a > b\n");
  assert_eq!(prettier("- a\n      - b\n"), "- a - b\n");
}

#[test]
fn keeps_html_indents() {
  for source in ["  <div>\n\n  </div>\n", "- a\n\n   <details>\n", ">  <p>\n", "   <!-- c -->\n"] {
    assert_eq!(prettier(source), source);
  }
  assert_eq!(prettier("1.  a\n\n   <img>\n"), "1.  a\n\n   <img>\n");
  assert_eq!(preserve("1.  a\n\n   <img>\n"), "1. a\n\n<img>\n");
}

#[test]
fn writes_destinations_and_titles_like_prettier() {
  let cases = [
    ("[a](u 'single')", "[a](u \"single\")"),
    ("[b](u (paren))", "[b](u \"paren\")"),
    ("[c](u 'say \"hi\"')", "[c](u 'say \"hi\"')"),
    ("[d](u \"a \\\"b\\\" 'c'\")", "[d](u (a \"b\" 'c'))"),
    (
      "[e](<a b>) [f](<a)b>) [g](a\\_b) [h](a&amp;b) [i]()",
      "[e](<a b>) [f](<a)b>) [g](a_b) [h](a&b) [i](<>)",
    ),
    ("[j](<z>) [k](a\n'x')", "[j](z) [k](a \"x\")"),
    ("![alt  *x*](i 't') ![](<>)", "![alt  *x*](i \"t\") ![](<>)"),
    ("[g  *h*][G  H]\n\n[G  H]: /g", "[g _h_][G H]\n\n[G H]: /g"),
  ];
  for (source, expected) in cases {
    assert_eq!(prettier(&format!("{source}\n")), format!("{expected}\n"), "{source}");
  }
  let source = "[a]: /x 'title'\n[b  c]:\n  /y\n  \"z\"\n[d]: <a b> (t)\n";
  assert_eq!(prettier(source), "[a]: /x \"title\"\n[b c]: /y \"z\"\n[d]: <a b> \"t\"\n");
}

#[test]
fn keeps_destinations_that_would_read_differently() {
  for source in ["[a](a\\(b)", "[a](a\\\\b)", "[a](\\&amp;)", "[a](<a<b>)", "[a](u \"\\&amp;\")"] {
    assert_eq!(prettier(&format!("{source}\n")), format!("{source}\n"), "{source}");
  }
}

#[test]
fn quotes_and_nesting() {
  assert_eq!(prettier(">a\n>\n>b\n"), "> a\n>\n> b\n");
  assert_eq!(prettier("> - a\n>   > b\n"), "> - a\n>   > b\n");
}

#[test]
fn keeps_front_matter() {
  assert_eq!(prettier("---\na:   1\n---\n# x\n"), "---\na:   1\n---\n\n# x\n");
  assert_eq!(prettier("+++\na = 1\n+++\nx\n"), "+++\na = 1\n+++\n\nx\n");
  assert_eq!(prettier("---yaml\na: 1\n...\nx\n"), "---yaml\na: 1\n...\n\nx\n");
  assert_eq!(prettier("---\n---\nx\n"), "---\n---\n\nx\n");
  let mut langs = Vec::new();
  for source in ["---\na\n---\n", "+++\na\n+++\n", "---json\n{}\n---\n"] {
    mdtidy::format_with(source, &Options::default(), |code| {
      langs.push(code.lang.to_string());
      None
    });
  }
  assert_eq!(langs, ["yaml", "toml", "json"]);
}

#[test]
fn reads_front_matter_only_at_the_start() {
  assert_eq!(prettier("x\n\n---\na: 1\n---\n"), "x\n\n---\n\n## a: 1\n");
  assert_eq!(prettier("3. a\n\n   ---\n   **P**\n---\n"), "3. a\n\n   ***\n\n   **P**\n\n---\n");
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
  assert_eq!(mdtidy::format(html, &never), "- a:\n   <details>\n     <x>\n\n  - b\n");
}

#[test]
fn keeps_inline_html_and_code_when_wrapping() {
  let source =
    "a <span\n  title=\"x\n  y\">b</span> c d e f\n\n- a <span\n    title=\"x\">b</span> c\n";
  let always = Options { wrap: Wrap::Always, width: 20, ..Options::default() };
  let expected =
    "a <span\n  title=\"x\n  y\">b</span> c d e\nf\n\n- a <span\n    title=\"x\">b</span>\n  c\n";
  assert_eq!(mdtidy::format(source, &always), expected);
  let never = Options { wrap: Wrap::Never, ..Options::default() };
  assert_eq!(mdtidy::format("> a `code\n>     span` b\n", &never), "> a `code     span` b\n");
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
