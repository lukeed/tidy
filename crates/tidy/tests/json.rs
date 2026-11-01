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
