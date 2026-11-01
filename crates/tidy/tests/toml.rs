//! Edge cases of TOML, each with what taplo 0.14.0 prints for it, recorded by `tests/oracle/toml`.

use serde_json::Value;
use tidy::doc::Eol;
use tidy::toml::{Options, format, is_toml_path};

/// The option sets of the oracle, by name.
fn options_of(set: &str) -> Options {
  let default = Options::default();
  match set {
    "indent 4" => Options { tab_width: 4, ..default },
    "tabs" => Options { tabs: true, ..default },
    "align entries, no comment alignment" => {
      Options { align_entries: true, align_comments: false, ..default }
    }
    "compact, width 40, no trailing comma" => Options {
      compact_entries: true,
      compact_inline_tables: true,
      compact_arrays: false,
      column_width: 40,
      array_trailing_comma: false,
      ..default
    },
    "indent tables and entries" => Options { indent_tables: true, indent_entries: true, ..default },
    "reorder" => {
      Options { reorder_keys: true, reorder_arrays: true, reorder_inline_tables: true, ..default }
    }
    "no expanding or collapsing, single comments" => Options {
      array_auto_expand: false,
      array_auto_collapse: false,
      inline_table_expand: false,
      align_single_comments: false,
      ..default
    },
    "no blank lines, no trailing newline, crlf" => {
      Options { allowed_blank_lines: 0, trailing_newline: false, eol: Eol::Crlf, ..default }
    }
    _ => default,
  }
}

#[test]
fn prints_what_taplo_prints() {
  let cases: Vec<Value> = serde_json::from_str(include_str!("toml.json")).unwrap();
  let mut failures = Vec::new();
  for case in &cases {
    let input = case["input"].as_str().unwrap();
    let set = case.get("set").and_then(Value::as_str).unwrap_or("default");
    let printed = format(input, &options_of(set)).unwrap_or_else(|_| "!error".into());
    let expected = case["output"].as_str().unwrap();
    if printed != expected {
      failures.push(format!("{input:?} ({set})\n  expected {expected:?}\n  printed  {printed:?}"));
    }
  }
  assert!(failures.is_empty(), "{} cases:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn knows_toml_paths() {
  for path in ["Cargo.toml", "x/pyproject.toml", "A.TOML", "dir\\taplo.toml"] {
    assert!(is_toml_path(path), "{path}");
  }
  for path in [".toml", "Cargo.lock", "toml", "a.toml.bak"] {
    assert!(!is_toml_path(path), "{path}");
  }
}

#[test]
fn keeps_a_byte_order_mark() {
  let printed = format("\u{feff}a=1", &Options::default()).unwrap();
  assert_eq!(printed, "\u{feff}a = 1\n");
}

#[test]
fn formats_toml_in_other_languages() {
  use tidy::{Language, embed::Verbatim};
  let cases = [
    (
      Language::Markdown,
      "+++\ntitle=\"x\"\ntags=[\"a\",\"b\"]\n+++\n\n# Hi\n\n```toml\na=1\n[t]\nb  =  [1,2]\n```\n",
      "+++\ntitle = \"x\"\ntags = [\"a\", \"b\"]\n+++\n\n# Hi\n\n```toml\na = 1\n[t]\nb = [1, 2]\n```\n",
    ),
    (
      Language::Markdown,
      "- list\n\n  ```toml\n  a=1\n  ```\n",
      "- list\n\n  ```toml\n  a = 1\n  ```\n",
    ),
    (
      Language::Css(tidy::css::Dialect::Scss),
      "+++\na=1\n+++\n\na{b:c}\n",
      "+++\na = 1\n+++\n\na {\n  b: c;\n}\n",
    ),
    (Language::Html, "+++\na=1\n+++\n<div></div>\n", "+++\na = 1\n+++\n\n<div></div>\n"),
    (Language::Handlebars, "+++\na=1\n+++\n<div></div>\n", "+++\na = 1\n+++\n\n<div></div>"),
  ];
  let options = tidy::Options::default();
  for (language, text, expected) in cases {
    let printed = tidy::format(text, language, &options, None, &mut Verbatim).unwrap();
    assert_eq!(printed, expected, "{language:?}");
  }
}
