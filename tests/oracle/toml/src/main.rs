//! Compares tidy's TOML formatter with taplo 0.14.0, run as `taplo fmt` runs it: on the files under folders, on the edge
//! cases of `crates/tidy/tests/toml.json`, whose outputs it records, and on the tokens of random texts, with logos.
//!
//!   cargo run --release -- files [--show=n] <folder>...
//!   cargo run --release -- cases <toml.json>
//!   cargo run --release -- lex [count]
//!   cargo run --release -- values <toml-test/tests>
//!
//! `values` checks `tidy::toml::value` with the TOML 1.0 files of toml-test: each valid file has its value, and no
//! invalid file has one.

#[path = "../../../../crates/tidy/src/toml/lexer.rs"]
#[allow(dead_code)]
mod lexer;

use logos::Logos;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use taplo::formatter::{Options as Taplo, OptionsIncomplete};
use tidy::toml::Options as Tidy;

fn main() {
  let args: Vec<String> = std::env::args().skip(1).collect();
  match args.first().map(String::as_str) {
    Some("files") => files(&args[1..]),
    Some("cases") => cases(&args[1]),
    Some("values") => values(Path::new(&args[1])),
    Some("lex") => lex(args.get(1).and_then(|it| it.parse().ok()).unwrap_or(1_000_000)),
    Some("tokens") => {
      for text in &args[1..] {
        println!("{text:?}\n  logos {:?}\n  tidy  {:?}", logos_tokens(text), tidy_tokens(text));
      }
    }
    _ => eprintln!(
      "usage: files <folder>... | cases <toml.json> | lex [count] | values <toml-test/tests>"
    ),
  }
}

/// What `taplo fmt` prints for `text`, or `!error` if it has a syntax error.
fn taplo(text: &str, options: &Taplo) -> String {
  let parse = taplo::parser::parse(text);
  if !parse.errors.is_empty() {
    return "!error".into();
  }
  let scopes = Vec::<(String, OptionsIncomplete)>::new();
  taplo::formatter::format_with_path_scopes(parse.into_dom(), options.clone(), &[], scopes)
    .unwrap_or_else(|_| "!error".into())
}

fn tidy(text: &str, options: &Tidy) -> String {
  tidy::toml::format(text, options).unwrap_or_else(|_| "!error".into())
}

/// Option sets: taplo's, and tidy's for the same.
fn sets() -> Vec<(&'static str, Taplo, Tidy)> {
  let (taplo, tidy) = (Taplo::default(), Tidy::default());
  vec![
    ("default", taplo.clone(), tidy.clone()),
    (
      "indent 4",
      Taplo { indent_string: "    ".into(), ..taplo.clone() },
      Tidy { tab_width: 4, ..tidy.clone() },
    ),
    (
      "tabs",
      Taplo { indent_string: "\t".into(), ..taplo.clone() },
      Tidy { tabs: true, ..tidy.clone() },
    ),
    (
      "align entries, no comment alignment",
      Taplo { align_entries: true, align_comments: false, ..taplo.clone() },
      Tidy { align_entries: true, align_comments: false, ..tidy.clone() },
    ),
    (
      "compact, width 40, no trailing comma",
      Taplo {
        compact_entries: true,
        compact_inline_tables: true,
        compact_arrays: false,
        column_width: 40,
        array_trailing_comma: false,
        ..taplo.clone()
      },
      Tidy {
        compact_entries: true,
        compact_inline_tables: true,
        compact_arrays: false,
        column_width: 40,
        array_trailing_comma: false,
        ..tidy.clone()
      },
    ),
    (
      "indent tables and entries",
      Taplo { indent_tables: true, indent_entries: true, ..taplo.clone() },
      Tidy { indent_tables: true, indent_entries: true, ..tidy.clone() },
    ),
    (
      "reorder",
      Taplo {
        reorder_keys: true,
        reorder_arrays: true,
        reorder_inline_tables: true,
        ..taplo.clone()
      },
      Tidy {
        reorder_keys: true,
        reorder_arrays: true,
        reorder_inline_tables: true,
        ..tidy.clone()
      },
    ),
    (
      "no expanding or collapsing, single comments",
      Taplo {
        array_auto_expand: false,
        array_auto_collapse: false,
        inline_table_expand: false,
        align_single_comments: false,
        ..taplo.clone()
      },
      Tidy {
        array_auto_expand: false,
        array_auto_collapse: false,
        inline_table_expand: false,
        align_single_comments: false,
        ..tidy.clone()
      },
    ),
    (
      "no blank lines, no trailing newline, crlf",
      Taplo { allowed_blank_lines: 0, trailing_newline: false, crlf: true, ..taplo },
      Tidy { allowed_blank_lines: 0, trailing_newline: false, eol: tidy::doc::Eol::Crlf, ..tidy },
    ),
  ]
}

fn files(args: &[String]) {
  let show = args
    .iter()
    .find_map(|it| it.strip_prefix("--show="))
    .and_then(|it| it.parse().ok())
    .unwrap_or(5);
  let mut paths = Vec::new();
  for folder in args.iter().filter(|it| !it.starts_with("--")) {
    walk(Path::new(folder), &mut paths);
  }
  let mut seen = HashSet::new();
  let mut sources = Vec::new();
  for path in paths {
    let Ok(text) = std::fs::read_to_string(&path) else { continue };
    if seen.insert(text.clone()) {
      sources.push((path, text));
    }
  }
  let (mut same, mut total, mut shown) = (0, 0, 0);
  for (name, taplo_options, tidy_options) in sets() {
    let mut different = 0;
    for (path, text) in &sources {
      total += 1;
      let expected = taplo(text, &taplo_options);
      let actual = tidy(text, &tidy_options);
      if expected == actual {
        same += 1;
        continue;
      }
      different += 1;
      if shown < show {
        shown += 1;
        println!("\n✗ {name}: {}", path.display());
        diff(&expected, &actual);
      }
    }
    if different > 0 {
      println!("  {name}: {different} different");
    }
  }
  println!(
    "\ntoml: {same}/{total} same ({:.1}%), {} files",
    same as f64 * 100.0 / total as f64,
    sources.len()
  );
}

/// Valid files that tidy reads otherwise: taplo's parser rejects all but the last, whose dates keep the digits they have.
const KNOWN: [&str; 8] = [
  "valid/comment/nonascii.toml",
  "valid/datetime/edge.toml",
  "valid/float/exponent-upper.toml",
  "valid/key/like-date.toml",
  "valid/key/numeric-04.toml",
  "valid/key/quoted-unicode.toml",
  "valid/string/quoted-unicode.toml",
  "valid/datetime/milliseconds.toml",
];

fn values(tests: &Path) {
  let list =
    std::fs::read_to_string(tests.join("files-toml-1.0.0")).expect("toml-test's file list");
  let (mut same, mut total) = (0, 0);
  for name in list.lines().filter(|it| it.ends_with(".toml") && !KNOWN.contains(it)) {
    let Ok(text) = std::fs::read_to_string(tests.join(name)) else { continue };
    total += 1;
    let actual = tidy::toml::value(&text);
    let wrong = match name.starts_with("valid/") {
      true => {
        let json = std::fs::read_to_string(tests.join(name.replace(".toml", ".json"))).unwrap();
        let expected = untag(&serde_json::from_str(&json).unwrap());
        match actual {
          Ok(actual) if actual == expected => None,
          Ok(actual) => Some(format!("expected {expected}\n  actual   {actual}")),
          Err(error) => Some(format!("expected {expected}\n  error    {error}")),
        }
      }
      false => actual.ok().map(|value| format!("accepted {value}")),
    };
    match wrong {
      None => same += 1,
      Some(wrong) => println!("✗ {name}\n  {wrong}"),
    }
  }
  println!("\ntoml values: {same}/{total} right, {} known", KNOWN.len());
}

/// A value of toml-test's JSON, without its types. A date is its text, with `T` and `Z` in capitals.
fn untag(value: &Value) -> Value {
  match value {
    Value::Array(items) => Value::Array(items.iter().map(untag).collect()),
    Value::Object(map) => match (map.get("type").and_then(Value::as_str), map.get("value")) {
      (Some(kind), Some(Value::String(text))) if map.len() == 2 => match kind {
        "integer" => Value::from(text.parse::<i64>().unwrap()),
        "float" => text
          .parse::<f64>()
          .ok()
          .and_then(serde_json::Number::from_f64)
          .map_or(Value::Null, Value::Number),
        "bool" => Value::Bool(text == "true"),
        _ => Value::String(text.clone()),
      },
      _ => Value::Object(map.iter().map(|(key, value)| (key.clone(), untag(value))).collect()),
    },
    _ => value.clone(),
  }
}

fn walk(path: &Path, out: &mut Vec<PathBuf>) {
  if path.is_file() {
    if path.extension().is_some_and(|it| it == "toml") {
      out.push(path.to_path_buf());
    }
    return;
  }
  let Ok(entries) = std::fs::read_dir(path) else { return };
  for entry in entries.flatten() {
    let name = entry.file_name();
    if matches!(name.to_str(), Some("target" | "node_modules" | ".git")) {
      continue;
    }
    walk(&entry.path(), out);
  }
}

fn diff(expected: &str, actual: &str) {
  let (expected, actual): (Vec<&str>, Vec<&str>) =
    (expected.lines().collect(), actual.lines().collect());
  let first = expected
    .iter()
    .zip(&actual)
    .position(|(a, b)| a != b)
    .unwrap_or(expected.len().min(actual.len()));
  let from = first.saturating_sub(2);
  for line in expected.iter().skip(from).take(8) {
    println!("  - {line:?}");
  }
  for line in actual.iter().skip(from).take(8) {
    println!("  + {line:?}");
  }
}

/// Records taplo's output for each case, under its options: taplo's, by name.
fn cases(path: &str) {
  let mut cases: Vec<Value> =
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
  let sets = sets();
  for case in &mut cases {
    let input = case["input"].as_str().unwrap().to_owned();
    let name = case.get("set").and_then(Value::as_str).unwrap_or("default");
    let (_, options, _) = sets.iter().find(|(it, ..)| *it == name).expect("a known option set");
    case["output"] = Value::String(taplo(&input, options));
  }
  let mut text = serde_json::to_string_pretty(&cases).unwrap();
  text.push('\n');
  std::fs::write(path, text).unwrap();
}

/// Compares the tokens of random texts with those of logos, up to the first error.
fn lex(count: usize) {
  let alphabet: Vec<char> =
    "0123456789-_:+.eEoxbaflnirstuzTZ \t\n\r\"'#[]{},=*?\\é".chars().collect();
  let templates = [
    "dddd-dd-dd",
    "dddd-dd-ddTdd:dd:dd",
    "dddd-dd-dd dd:dd:dd.ddd",
    "dd:dd:dd,dd",
    "dddd-dd-ddtdd:dd:ddZ",
    "dddd-dd-ddTdd:dd:dd+dd:dd",
    "dddd-dd-ddTdd:dd:dd.d-dd:dd",
  ];
  let mut seed = 0x2545F4914F6CDD1Du64;
  let mut random = move |below: usize| {
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    (seed % below as u64) as usize
  };
  let mut failures = 0;
  for _ in 0..count {
    let text: String = match random(2) {
      0 => (0..1 + random(24)).map(|_| alphabet[random(alphabet.len())]).collect(),
      _ => {
        let template = templates[random(templates.len())];
        let mut text: String = template
          .chars()
          .map(|it| match it {
            'd' if random(3) == 0 => char::from(b'0' + random(10) as u8),
            'd' => char::from(b'0' + [0, 1, 2, 3, 9][random(5)] as u8),
            _ if random(20) == 0 => alphabet[random(alphabet.len())],
            it => it,
          })
          .collect();
        let keep = random(text.chars().count() + 1).max(1);
        text = text.chars().take(keep).collect();
        for _ in 0..random(4) {
          text.push(alphabet[random(alphabet.len())]);
        }
        text
      }
    };
    let expected = logos_tokens(&text);
    let actual = tidy_tokens(&text);
    if expected != actual {
      failures += 1;
      if failures <= 200 {
        println!("{text:?}\n  logos {expected:?}\n  tidy  {actual:?}");
      }
    }
  }
  println!("lex: {} of {count} texts differ", failures);
}

fn logos_tokens(text: &str) -> Vec<(String, usize, usize)> {
  let mut lexer = taplo::syntax::SyntaxKind::lexer(text);
  let mut tokens = Vec::new();
  while let Some(token) = lexer.next() {
    let name = format!("{token:?}").replace('_', "").to_lowercase();
    let span = lexer.span();
    let error = name == "error";
    tokens.push((name, span.start, if error { span.start } else { span.end }));
    if error {
      break;
    }
  }
  tokens
}

fn tidy_tokens(text: &str) -> Vec<(String, usize, usize)> {
  let bytes = text.as_bytes();
  let mut tokens = Vec::new();
  let mut at = 0;
  while at < bytes.len() {
    let (token, length) = lexer::next(&bytes[at..]);
    let name = format!("{token:?}").to_lowercase();
    let error = name == "error";
    tokens.push((name, at, if error { at } else { at + length }));
    if error {
      break;
    }
    at += length;
  }
  tokens
}
