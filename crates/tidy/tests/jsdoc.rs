#![cfg(feature = "js")]

//! JSDoc comments, formatted by default as prettier-plugin-jsdoc does. `fixtures/jsdoc` is a part of oxc's
//! `crates/oxc_formatter/tests/jsdoc/fixtures` (MIT): each input next to oxfmt's output with `jsdoc: true`.

use std::path::Path;
use tidy::js::{self, Dialect, Options};

fn format(text: &str, path: &str, options: &Options) -> String {
  let dialect = Dialect::of_path(path).unwrap();
  js::format(text, path, &Options { dialect, ..options.clone() }).unwrap()
}

#[test]
fn matches_oxfmt() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jsdoc");
  let mut failures = Vec::new();
  let mut count = 0;
  for entry in std::fs::read_dir(&dir).unwrap() {
    let path = entry.unwrap().path();
    let name = path.file_name().unwrap().to_str().unwrap().to_owned();
    if name.contains(".out.") {
      continue;
    }
    let (stem, extension) = name.rsplit_once('.').unwrap();
    let expected = std::fs::read_to_string(dir.join(format!("{stem}.out.{extension}"))).unwrap();
    let input = std::fs::read_to_string(&path).unwrap();
    let printed = format(&input, &name, &Options::default());
    if printed != expected {
      failures.push(format!("{name}\n--- expected\n{expected}--- printed\n{printed}"));
    }
    count += 1;
  }
  assert!(count > 0);
  assert!(failures.is_empty(), "{} of {count}:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn keeps_comments_that_follow_each_other_directly_apart() {
  let input = "class A {\n    /**\n     * @constructor\n     * @overload\n     * @param {string}   a\n     *//**\n     * @constructor\n     * @param {number | string}   a   the thing\n     */\n    constructor(a, b) {}\n}\n";
  let expected = "class A {\n  /**\n   * @overload\n   * @class\n   * @param {string} a\n   *//**\n   * @class\n   * @param {number | string} a The thing\n   */\n  constructor(a, b) {}\n}\n";
  assert_eq!(format(input, "a.js", &Options::default()), expected);
}

#[test]
fn formats_code_in_other_languages() {
  let input = "/**\n * Styles:\n *\n * ```css\n * a{color:red}\n * ```\n *\n * ```json\n * {\"a\":1,\n * \"b\":[1,2]}\n * ```\n *\n * ```yaml\n * a:   1\n * ```\n */\nlet a;\n";
  let expected = "/**\n * Styles:\n *\n * ```css\n * a {\n *   color: red;\n * }\n * ```\n *\n * ```json\n * { \"a\": 1, \"b\": [1, 2] }\n * ```\n *\n * ```yaml\n * a: 1\n * ```\n */\nlet a;\n";
  assert_eq!(format(input, "a.js", &Options::default()), expected);
}

#[test]
fn keeps_the_alt_text_of_images() {
  let input =
    "/**\n * Shows ![an image](a.png) and ![Bar `b` *c*][d].\n *\n * [d]: e.png\n */\nlet a;\n";
  let expected =
    "/**\n * Shows ![an image](a.png) and ![Bar b c][d].\n *\n * [d]: e.png\n */\nlet a;\n";
  assert_eq!(format(input, "a.js", &Options::default()), expected);
}

#[test]
fn makes_only_strong_underscores_asterisks() {
  let input = "/**\n * @param a call __init__, snake__case__name or __x\n */\nlet a;\n";
  let expected = "/** @param a Call **init**, snake__case__name or __x */\nlet a;\n";
  assert_eq!(format(input, "a.js", &Options::default()), expected);
}

#[test]
fn can_be_turned_off() {
  let input = "/**\n * @param {string}   a   the thing\n */\nfunction f(a) {}\n";
  let options = Options { jsdoc: false, ..Options::default() };
  assert_eq!(format(input, "a.js", &options), input);
  let expected = "/** @param {string} a The thing */\nfunction f(a) {}\n";
  assert_eq!(format(input, "a.js", &Options::default()), expected);
}

#[cfg(feature = "config")]
#[test]
fn reads_the_config_key() {
  let config = tidy::config::Config::json(r#"{ "js": { "jsdoc": false } }"#).unwrap();
  let mut options = tidy::Options::default();
  config.apply(&mut options);
  assert!(!options.js.jsdoc);
  assert!(tidy::Options::default().js.jsdoc);
  assert!(tidy::config::Config::json(r#"{ "js": { "jsdoc": "no" } }"#).is_err());
}
