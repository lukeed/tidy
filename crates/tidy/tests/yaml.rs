//! YAML, with outputs that Prettier 3.9.9 prints. `tests/oracle/yaml` compares the rest with Prettier.

use tidy::embed::Verbatim;
use tidy::yaml::{Options, format, is_yaml_path};
use tidy::{Language, Wrap};

#[test]
fn knows_yaml_paths() {
  for path in [
    "a.yml",
    "x/b.yaml",
    "A.YML",
    ".prettierrc",
    "x/.clang-format",
    "CITATION.cff",
    "a.sublime-syntax",
  ] {
    assert!(is_yaml_path(path), "{path}");
  }
  for path in ["a.json", "yaml", "a.yml.bak", ".prettierrc.json"] {
    assert!(!is_yaml_path(path), "{path}");
  }
}

#[test]
fn wraps_prose() {
  let text = "key:   'it''s'\nlist:\n- a\n-   b\nmap: {a: 1, b: [1,2]}\nlong: this is a long plain scalar that should be wrapped when prose wrap is always on\n";
  let options = Options { wrap: Wrap::Always, width: 40, ..Options::default() };
  let expected = "key: \"it's\"\nlist:\n  - a\n  - b\nmap: { a: 1, b: [1, 2] }\nlong:\n  this is a long plain scalar that\n  should be wrapped when prose wrap is\n  always on\n";
  assert_eq!(format(text, &options).unwrap(), expected);
}

#[test]
fn formats_a_json_prettierrc_as_json() {
  let options = Options { filepath: Some(".prettierrc".into()), ..Options::default() };
  let json = "{\"semi\":false,\n\"singleQuote\":true}\n";
  assert_eq!(format(json, &options).unwrap(), "{ \"semi\": false, \"singleQuote\": true }\n");
  assert_eq!(format("semi:   false\n", &options).unwrap(), "semi: false\n");
}

#[test]
fn prints_nothing_for_white_space() {
  for text in ["", "  \n", "\n\n\t"] {
    assert_eq!(format(text, &Options::default()).unwrap(), "");
  }
}

#[test]
fn rejects_what_prettier_rejects() {
  for text in ["a: b: c\n", "- a\nb: c\n", "key: [a, b\n", "\"unclosed\n", "a:\n\t- b\n"] {
    assert!(format(text, &Options::default()).is_err(), "{text:?}");
  }
}

#[test]
fn formats_yaml_in_other_languages() {
  let cases = [
    (
      Language::Markdown,
      "---\ntitle:   x\nlist: [a,b]\n---\n\n# Hi\n\n```yaml\na:   1\nb:\n    - c\n```\n",
      "---\ntitle: x\nlist: [a, b]\n---\n\n# Hi\n\n```yaml\na: 1\nb:\n  - c\n```\n",
    ),
    (
      Language::Css(tidy::css::Dialect::Scss),
      "---\na:   1\n---\n\na{b:c}\n",
      "---\na: 1\n---\n\na {\n  b: c;\n}\n",
    ),
    (Language::Html, "---\na:   1\n---\n<div></div>\n", "---\na: 1\n---\n\n<div></div>\n"),
  ];
  let options = tidy::Options::default();
  for (language, text, expected) in cases {
    let printed = tidy::format(text, language, &options, None, &mut Verbatim).unwrap();
    assert_eq!(printed, expected, "{language:?}");
  }
}

#[test]
fn parses_the_tree_prettier_prints() {
  let root = tidy::yaml::parse("# head\na: 1 # one\n").unwrap();
  let document = root.node(root.root).children[0];
  let body = root.node(document).children[1];
  let mapping = root.node(body).children[0];
  assert_eq!(root.node(mapping).kind, tidy::yaml::Kind::Mapping);
  assert_eq!(root.comments.len(), 2);
}
