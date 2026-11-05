//! The schema and the config parser must know the same keys.

#![cfg(feature = "config")]

use std::collections::BTreeSet;

use tidy::config::Config;
use tidy::schema::{SECTIONS, schema, section, shared};

/// Every `"key" =>` whose nearest enclosing `match` is on a config key.
///
/// The parser also matches on values, such as `"lf"`, in blocks nested inside
/// those arms; tracking which kind of block an arm sits in keeps those out.
fn parsed_keys() -> BTreeSet<String> {
  let source = include_str!("../src/config.rs");
  let mut keys = BTreeSet::new();
  let mut depth = 0i32;
  let mut blocks: Vec<(i32, bool)> = Vec::new();

  for line in source.lines() {
    let trimmed = line.trim();

    if let Some(rest) = trimmed.strip_prefix('"')
      && let Some((key, after)) = rest.split_once('"')
      && after.trim_start().starts_with("=>")
      && blocks.last().is_some_and(|&(_, on_key)| on_key)
    {
      keys.insert(key.to_owned());
    }

    let opened = depth;
    depth += trimmed.matches('{').count() as i32;
    depth -= trimmed.matches('}').count() as i32;

    if let Some(at) = trimmed.find("match ") {
      let subject = trimmed[at + "match ".len()..].trim_start();
      blocks.push((opened, subject.starts_with("key")));
    }
    while blocks.last().is_some_and(|&(start, _)| depth <= start) {
      blocks.pop();
    }
  }
  keys
}

/// Every key the schema describes, at the top level or in a section.
fn schema_keys() -> BTreeSet<String> {
  let mut keys: BTreeSet<String> = shared().iter().map(|it| it.name.to_owned()).collect();
  for name in SECTIONS {
    keys.insert((*name).to_owned());
    keys.extend(section(name).iter().map(|it| it.name.to_owned()));
  }
  keys
}

/// The values of a choice are part of the contract too, so a renamed variant fails here.
#[test]
fn the_parser_accepts_every_key_the_schema_describes() {
  for option in shared() {
    let json = format!("{{ {:?}: {} }}", option.name, option.default);
    Config::json(&json).unwrap_or_else(|error| panic!("shared `{}`: {error}", option.name));
  }
  for name in SECTIONS {
    for option in shared().into_iter().chain(section(name)) {
      let json = format!("{{ {name:?}: {{ {:?}: {} }} }}", option.name, option.default);
      Config::json(&json).unwrap_or_else(|error| panic!("{name}.{}: {error}", option.name));
    }
  }
}

#[test]
fn the_schema_describes_every_key_the_parser_accepts() {
  let missing: Vec<String> = parsed_keys().difference(&schema_keys()).cloned().collect();
  assert!(missing.is_empty(), "keys the parser accepts but the schema omits: {missing:?}");
}

#[test]
fn every_choice_is_one_the_parser_takes() {
  for name in SECTIONS {
    for option in shared().into_iter().chain(section(name)) {
      let tidy::schema::Kind::Choice(values) = option.kind else { continue };
      for value in values {
        let json = format!("{{ {name:?}: {{ {:?}: {value:?} }} }}", option.name);
        Config::json(&json)
          .unwrap_or_else(|error| panic!("{name}.{} = {value:?}: {error}", option.name));
      }
    }
  }
}

#[test]
fn the_schema_is_json() {
  let text = schema();
  let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
  assert_eq!(value["title"], "tidy");
  assert_eq!(value["additionalProperties"], false);
  assert_eq!(value["properties"]["width"]["default"], 80);
  assert_eq!(value["properties"]["js"]["properties"]["semi"]["default"], true);
  // A section takes the shared options as well as its own.
  assert_eq!(value["properties"]["markdown"]["properties"]["width"]["default"], 80);
}
