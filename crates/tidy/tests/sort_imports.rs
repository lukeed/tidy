#![cfg(feature = "js")]

//! Import sorting, which is off by default. `fixtures/sort_imports` holds a part of bun's tests
//! (`test/cli/format/sort-imports`, MIT): the inputs of `@trivago/prettier-plugin-sort-imports`,
//! `@ianvs/prettier-plugin-sort-imports` (both Apache-2.0, see `LICENSE`) and oxc's formatter, and generated ones,
//! each with its options and with the output of Prettier 3.9.9 with the plugin (with TypeScript 5.9 for
//! `prettier-plugin-organize-imports`) or of oxfmt 0.72. `native.json` is bun's `sortImports` from bunfig.toml.

use serde_json::Value;
use std::path::Path;
use tidy::js::{self, Dialect, Options, SortSettings};

/// Formats each case of `fixtures/sort_imports/<tool>.json` with the plugin `plugin` listed.
fn matches(tool: &str, plugin: Option<&str>) {
  let path =
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/sort_imports/{tool}.json"));
  let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
  let mut failures = Vec::new();
  for case in &cases {
    let filename = case["filename"].as_str().unwrap();
    let mut options =
      Options { dialect: Dialect::of_path(filename).unwrap(), jsdoc: false, ..Options::default() };
    let mut settings = SortSettings::default();
    if let Some(plugin) = plugin {
      settings.set("plugins", Value::from(vec![plugin]));
    }
    for (key, value) in case["options"].as_object().unwrap() {
      match key.as_str() {
        "printWidth" => options.width = value.as_u64().unwrap() as usize,
        "tabWidth" => options.tab_width = value.as_u64().unwrap() as usize,
        "semi" => options.semi = value.as_bool().unwrap(),
        "singleQuote" => options.single_quote = value.as_bool().unwrap(),
        _ => assert!(settings.set(key, value.clone()), "unknown option {key}"),
      }
    }
    options.sort_imports = settings.compile().unwrap();
    let (input, expected) = (case["input"].as_str().unwrap(), case["output"].as_str().unwrap());
    let printed = js::format(input, filename, &options).unwrap();
    if printed != expected {
      failures.push(format!("{}\n--- expected\n{expected}--- printed\n{printed}", case["name"]));
    }
  }
  assert!(!cases.is_empty());
  assert!(failures.is_empty(), "{} of {}:\n{}", failures.len(), cases.len(), failures.join("\n"));
}

#[test]
fn matches_trivago() {
  matches("trivago", Some("@trivago/prettier-plugin-sort-imports"));
}

#[test]
fn matches_ianvs() {
  matches("ianvs", Some("@ianvs/prettier-plugin-sort-imports"));
}

#[test]
fn matches_organize_imports() {
  matches("organize", Some("prettier-plugin-organize-imports"));
}

#[test]
fn matches_oxfmt() {
  matches("oxfmt", None);
}

#[test]
fn matches_bun() {
  matches("native", None);
}

fn sorted(input: &str, settings: &[(&str, Value)]) -> String {
  let mut sort = SortSettings::default();
  for (name, value) in settings {
    assert!(sort.set(name, value.clone()));
  }
  let options = Options { sort_imports: sort.compile().unwrap(), ..Options::default() };
  js::format(input, "a.ts", &options).unwrap()
}

const UNSORTED: &str = "import b from \"b\";\nimport a from \"a\";\n";
const SORTED: &str = "import a from \"a\";\nimport b from \"b\";\n";

#[test]
fn is_off_by_default() {
  assert_eq!(js::format(UNSORTED, "a.ts", &Options::default()).unwrap(), UNSORTED);
  assert_eq!(sorted(UNSORTED, &[("sortImports", Value::Bool(false))]), UNSORTED);
}

#[test]
fn is_off_with_the_options_of_a_plugin_that_isnt_listed() {
  let settings =
    [("importOrder", Value::from(vec!["^[./]"])), ("importOrderSeparation", Value::Bool(true))];
  assert_eq!(sorted(UNSORTED, &settings), UNSORTED);
}

#[test]
fn is_on_with_a_listed_plugin_and_no_options() {
  let plugins = |name: &str| [("plugins", Value::from(vec![name]))];
  assert_eq!(sorted(UNSORTED, &plugins("@ianvs/prettier-plugin-sort-imports")), SORTED);
  assert_eq!(sorted(UNSORTED, &[("sortImports", Value::Bool(true))]), SORTED);
}

#[test]
fn rejects_an_invalid_regular_expression() {
  let mut sort = SortSettings::default();
  sort.set("plugins", Value::from(vec!["@trivago/prettier-plugin-sort-imports"]));
  sort.set("importOrder", Value::from(vec!["("]));
  assert!(sort.compile().unwrap_err().contains("importOrder"));
}

#[cfg(feature = "config")]
#[test]
fn reads_the_config_keys() {
  let read = |json: &str| {
    let config = tidy::config::Config::json(json).unwrap();
    let mut options = tidy::Options::default();
    config.apply(&mut options);
    js::format(UNSORTED, "a.ts", &options.js).unwrap()
  };
  assert_eq!(read(r#"{ "js": { "sortImports": { "order": "asc" } } }"#), SORTED);
  assert_eq!(read(r#"{ "js": { "plugins": ["@trivago/prettier-plugin-sort-imports"] } }"#), SORTED);
  assert_eq!(read(r#"{ "js": { "semi": true } }"#), UNSORTED);
  let invalid = r#"{ "js": { "sortImports": { "order": "up" } } }"#;
  assert!(tidy::config::Config::json(invalid).is_err());
  assert!(tidy::config::Config::json(r#"{ "js": { "tsconfig.jsx": "react" } }"#).is_err());
}

/// Formats the Vue, HTML, Markdown and other files of `fixtures/sort_imports/hosts.json` (bun's tests) with the config
/// of `name`, except the files in `skips`.
#[cfg(feature = "config")]
fn matches_hosts(name: &str, skips: &[&str]) {
  let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sort_imports/hosts.json");
  let all: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
  let (config, files) = (&all[name]["config"][1], all[name]["files"].as_object().unwrap());
  let mut js = serde_json::Map::from_iter([("jsdoc".to_owned(), Value::from(false))]);
  let mut shared = serde_json::Map::new();
  for (key, value) in config.as_object().unwrap() {
    match key.as_str() {
      "printWidth" => drop(shared.insert("width".to_owned(), value.clone())),
      _ => drop(js.insert(key.clone(), value.clone())),
    }
  }
  shared.insert("js".to_owned(), Value::Object(js));
  let mut options = tidy::Options::default();
  tidy::config::Config::from_value(&Value::Object(shared)).unwrap().apply(&mut options);
  let mut failures = Vec::new();
  for (file, case) in files.iter().filter(|(file, _)| !skips.contains(&file.as_str())) {
    let (input, expected) = (case[0].as_str().unwrap(), case[1].as_str().unwrap());
    let Some(language) = tidy::Language::of_path(file) else { continue };
    let mut formatter = tidy::Formatter::default();
    let printed =
      formatter.format(input, language, &options, Some(file), &mut tidy::embed::Verbatim).unwrap();
    if printed != expected {
      failures.push(format!("{file}\n--- expected\n{expected}--- printed\n{printed}"));
    }
  }
  assert!(failures.is_empty(), "{} of {}:\n{}", failures.len(), files.len(), failures.join("\n"));
}

#[test]
#[cfg(feature = "config")]
fn matches_trivago_in_hosts() {
  matches_hosts("@trivago/prettier-plugin-sort-imports", &[]);
}

#[test]
#[cfg(feature = "config")]
fn matches_ianvs_in_hosts() {
  matches_hosts("@ianvs/prettier-plugin-sort-imports", &[]);
}

#[test]
#[cfg(feature = "config")]
fn matches_oxfmt_in_hosts() {
  matches_hosts("oxfmt, sortImports", &[]);
}

#[test]
#[cfg(feature = "config")]
fn matches_oxfmt_with_jsdoc_in_hosts() {
  // oxfmt leaves HTML to Prettier, without prettier-plugin-jsdoc. tidy formats JSDoc in the scripts of HTML as the
  // plugin does.
  matches_hosts("oxfmt, sortImports and jsdoc", &["jsdoc.html"]);
}
