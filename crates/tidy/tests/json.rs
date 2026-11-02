//! Edge cases of JSON, each with what Prettier prints for it, recorded by `tests/oracle/json-cases.ts`.

use serde_json::Value;
use tidy::json::{Dialect, ObjectWrap, Options, QuoteProps, format};

fn options_of(options: &Value) -> Options {
  let flag = |name: &str| options.get(name).and_then(Value::as_bool);
  let text = |name: &str| options.get(name).and_then(Value::as_str);
  let size = |name: &str| options.get(name).and_then(Value::as_u64).map(|it| it as usize);
  let defaults = Options::default();
  Options {
    dialect: match text("parser") {
      Some("jsonc") => Dialect::Jsonc,
      Some("json5") => Dialect::Json5,
      Some("json-stringify") => Dialect::Stringify,
      _ => Dialect::Json,
    },
    width: size("printWidth").unwrap_or(defaults.width),
    tab_width: size("tabWidth").unwrap_or(defaults.tab_width),
    tabs: flag("useTabs").unwrap_or_default(),
    trailing_comma: text("trailingComma") != Some("none"),
    quote_props: match text("quoteProps") {
      Some("consistent") => QuoteProps::Consistent,
      Some("preserve") => QuoteProps::Preserve,
      _ => QuoteProps::AsNeeded,
    },
    single_quote: flag("singleQuote").unwrap_or_default(),
    bracket_spacing: flag("bracketSpacing").unwrap_or(true),
    object_wrap: match text("objectWrap") {
      Some("collapse") => ObjectWrap::Collapse,
      _ => ObjectWrap::Preserve,
    },
    ..defaults
  }
}

#[test]
fn prints_what_prettier_prints() {
  let cases: Vec<Value> = serde_json::from_str(include_str!("json.json")).unwrap();
  let mut failures = Vec::new();
  for case in &cases {
    let input = case["input"].as_str().unwrap();
    let options = case.get("options").cloned().unwrap_or_default();
    let printed = format(input, &options_of(&options)).unwrap_or_else(|_| "!error".into());
    let expected = case["output"].as_str().unwrap();
    if printed != expected {
      failures
        .push(format!("{input:?} {options}\n  expected {expected:?}\n  printed  {printed:?}"));
    }
  }
  assert!(failures.is_empty(), "{} cases:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn knows_json_paths() {
  for (path, dialect) in [
    ("a.json", Dialect::Json),
    ("x/tsconfig.json", Dialect::Json),
    (".babelrc", Dialect::Json),
    ("a.jsonc", Dialect::Jsonc),
    ("a.code-workspace", Dialect::Jsonc),
    ("a.json5", Dialect::Json5),
    ("x/package.json", Dialect::Stringify),
    ("Package.JSON", Dialect::Stringify),
    ("a.importmap", Dialect::Stringify),
    ("a.JSON", Dialect::Json),
  ] {
    assert_eq!(Dialect::of_path(path), Some(dialect), "{path}");
  }
  for path in ["a.js", "json", "composer.lock", "a.JSON-tmLanguage", "a.jsonl"] {
    assert_eq!(Dialect::of_path(path), None, "{path}");
  }
}

#[test]
fn takes_another_ignore_directive() {
  let options = Options { ignore: "tidy-ignore".into(), ..Options::default() };
  let text =
    "{\n  // tidy-ignore\n  \"a\": [1,   2],\n  // prettier-ignore\n  \"b\": [1,   2]\n}\n";
  let expected =
    "{\n  // tidy-ignore\n  \"a\": [1,   2],\n  // prettier-ignore\n  \"b\": [1, 2]\n}\n";
  assert_eq!(format(text, &options).unwrap(), expected);
}

#[test]
fn parses_what_any_dialect_reads() {
  let tree = tidy::json::parse("// c\n{a: [1, , 'b'], \"c\": -Infinity}").unwrap();
  assert_eq!(tree.comments().len(), 1);
  let root = tree.node(tree.root().unwrap());
  let tidy::json::Kind::Object(items) = root.kind else { panic!("{root:?}") };
  assert_eq!(tree.items(items).len(), 2);
  assert!(tidy::json::parse("// only a comment").unwrap().root().is_none());
  assert!(tidy::json::parse("{a: b}").is_err());
}

#[test]
fn formats_json_in_other_languages() {
  use tidy::{Language, embed::Verbatim};
  let cases = [
    (
      Language::Markdown,
      "```json\n{\"a\":1,\n\"b\":[1,2,3]}\n```\n\n- item\n\n  ```jsonc\n  {\"a\":  [1,2], // c\n  }\n  ```\n",
      "```json\n{ \"a\": 1, \"b\": [1, 2, 3] }\n```\n\n- item\n\n  ```jsonc\n  {\n    \"a\": [1, 2], // c\n  }\n  ```\n",
    ),
    (
      Language::Html,
      "<div><div><script type=\"application/json\">{\"a\":1,\n\"b\":[1,2,3], \"c\": {\"d\": \"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\"}}</script></div></div>\n",
      "<div>\n  <div>\n    <script type=\"application/json\">\n      {\n        \"a\": 1,\n        \"b\": [1, 2, 3],\n        \"c\": { \"d\": \"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\" }\n      }\n    </script>\n  </div>\n</div>\n",
    ),
    (
      Language::Vue,
      "<template><div/></template>\n<i18n lang=\"json5\">{en:{'hello':'Hello'}}</i18n>\n",
      "<template><div /></template>\n<i18n lang=\"json5\">\n{ en: { hello: \"Hello\" } }\n</i18n>\n",
    ),
  ];
  let options = tidy::Options::default();
  for (language, text, expected) in cases {
    let printed = tidy::format(text, language, &options, None, &mut Verbatim).unwrap();
    assert_eq!(printed, expected, "{language:?}");
  }
}

#[cfg(feature = "config")]
#[test]
fn reads_values_as_json5_does() {
  let text = "// c\n{ a: [0x1F, .5, 5., +1, -0, 1_000, 017, 08, 1e3], 'b': \"x\\u0041\", c: `t`, 1e3: Infinity, d: -NaN, }";
  let expected = serde_json::json!({
    "a": [31, 0.5, 5.0, 1, -0.0, 1000, 15, 8, 1000.0],
    "b": "xA",
    "c": "t",
    "1000": null,
    "d": null,
  });
  assert_eq!(tidy::json::value(text).unwrap(), expected);
  assert_eq!(tidy::json::value("// only\n").unwrap(), serde_json::Value::Null);
  assert!(tidy::json::value("[1, , 2]").is_err());
}
