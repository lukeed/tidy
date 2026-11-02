//! Compares the parts of tidy's YAML parser with `yaml` 2.9.0, whose results `tests/oracle/yaml/*.ts` writes.
//!
//!   bun tests/oracle/yaml/cst.ts > target/yaml-cst.json && cargo run --release -- cst target/yaml-cst.json

#[path = "../../../../crates/tidy/src/yaml/cst.rs"]
#[allow(dead_code)]
mod cst;
#[path = "../../../../crates/tidy/src/yaml/lexer.rs"]
mod lexer;

use cst::{Item, Key, Node, Prop, ScalarKind, Source, Tree};
use serde_json::{Map, Value, json};

fn main() {
  let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(run).unwrap();
  worker.join().unwrap();
}

/// Reads a JSON file, as deep as its trees go.
fn read(path: &str) -> Vec<Value> {
  use serde::Deserialize;
  let text = std::fs::read_to_string(path).unwrap();
  let mut deserializer = serde_json::Deserializer::from_str(&text);
  deserializer.disable_recursion_limit();
  Vec::<Value>::deserialize(&mut deserializer).unwrap()
}

fn run() {
  let args: Vec<String> = std::env::args().skip(1).collect();
  match args.first().map(String::as_str) {
    Some("cst") => cst(&args[1], args.get(2).and_then(|it| it.parse().ok()).unwrap_or(5)),
    Some("format") => {
      format(&args[1], args.get(2).and_then(|it| it.parse().ok()).unwrap_or(5), args.get(3))
    }
    Some("ast") => ast(&args[1], args.get(2).and_then(|it| it.parse().ok()).unwrap_or(5)),
    Some("accepts") => accepts(&args[1], args.get(2).and_then(|it| it.parse().ok()).unwrap_or(10)),
    _ => eprintln!("usage: cst <yaml-cst.json> [show]"),
  }
}

fn cst(path: &str, show: usize) {
  let cases = read(path);
  let (mut same, mut shown) = (0, 0);
  for case in &cases {
    let name = case["name"].as_str().unwrap();
    let text = case["text"].as_str().unwrap();
    let mut lexemes = Vec::new();
    lexer::Lexer::lex(text.as_bytes(), &mut lexemes);
    let mut tree = Tree::default();
    cst::Parser::parse(text.as_bytes(), &lexemes, &mut tree);
    let dump = Dump { text, utf16: utf16_offsets(text), tree: &tree };
    let actual = Value::Array(tree.tokens.iter().map(|id| dump.node(*id)).collect());
    let expected = strip_messages(case["cst"].clone());
    if actual == expected {
      same += 1;
      continue;
    }
    if shown < show {
      shown += 1;
      println!("\n✗ {name}\n  text {:?}", text.chars().take(200).collect::<String>());
      first_difference(&expected, &actual, "");
    }
  }
  println!("\ncst: {same}/{} same", cases.len());
}

/// Compares what Prettier prints with what tidy prints, under the option sets of `format.ts`.
fn format(path: &str, show: usize, filter: Option<&String>) {
  use tidy::Wrap;
  use tidy::yaml::Options;
  let cases = read(path);
  let (mut same, mut total, mut shown) = (0, 0, 0);
  let mut per_set: std::collections::BTreeMap<String, usize> = Default::default();
  for case in &cases {
    let name = case["name"].as_str().unwrap();
    let set = case["set"].as_str().unwrap();
    if filter.is_some_and(|filter| !format!("{set}: {name}").contains(filter.as_str())) {
      continue;
    }
    total += 1;
    let text = case["text"].as_str().unwrap();
    let expected = case["expected"].as_str().unwrap();
    let default = Options::default();
    let options = match set {
      "single quotes, width 40" => Options { single_quote: true, width: 40, ..default },
      "prose always, width 40" => Options { wrap: Wrap::Always, width: 40, ..default },
      "prose never" => Options { wrap: Wrap::Never, ..default },
      "no bracket spacing, no trailing commas, indent 4" => {
        Options { bracket_spacing: false, trailing_comma: false, tab_width: 4, ..default }
      }
      _ => default,
    };
    let actual = tidy::yaml::format(text, &options).unwrap_or_else(|_| "!error".into());
    if actual == expected {
      same += 1;
      continue;
    }
    *per_set.entry(set.to_owned()).or_default() += 1;
    if shown < show {
      shown += 1;
      println!("\n✗ {set}: {name}");
      let (expected, actual): (Vec<&str>, Vec<&str>) =
        (expected.lines().collect(), actual.lines().collect());
      let first = expected
        .iter()
        .zip(&actual)
        .position(|(a, b)| a != b)
        .unwrap_or(expected.len().min(actual.len()));
      let from = first.saturating_sub(3);
      for line in expected.iter().skip(from).take(8) {
        println!("  - {line:?}");
      }
      for line in actual.iter().skip(from).take(8) {
        println!("  + {line:?}");
      }
    }
  }
  for (set, count) in per_set {
    println!("  {set}: {count} different");
  }
  println!("\nformat: {same}/{total} same");
}

/// Compares the trees of Prettier's YAML parser with tidy's.
fn ast(path: &str, show: usize) {
  let cases = read(path);
  let (mut same, mut shown) = (0, 0);
  for case in &cases {
    let name = case["name"].as_str().unwrap();
    let text = case["text"].as_str().unwrap();
    let actual = match tidy::yaml::parse(text) {
      Ok(root) => {
        let dump = AstDump { root: &root, utf16: utf16_offsets(text) };
        json!({ "ast": dump.node(root.root) })
      }
      Err(error) => json!({ "error": error.to_string() }),
    };
    let expected = json!({ "ast": case["ast"] });
    if actual == expected {
      same += 1;
      continue;
    }
    if shown < show {
      shown += 1;
      println!("\n✗ {name}\n  text {:?}", text.chars().take(200).collect::<String>());
      first_difference(&expected, &actual, "");
    }
  }
  println!("\nast: {same}/{} same", cases.len());
}

struct AstDump<'d> {
  root: &'d tidy::yaml::Root,
  utf16: Vec<u32>,
}

impl AstDump<'_> {
  fn point(&self, point: tidy::yaml::Point) -> Value {
    json!([point.line, point.column, self.utf16[point.offset as usize]])
  }

  fn node(&self, id: u32) -> Value {
    use tidy::yaml::{Chomping, Kind};
    let node = self.root.node(id);
    let kind = match node.kind {
      Kind::Root => "root",
      Kind::Document => "document",
      Kind::DocumentHead => "documentHead",
      Kind::DocumentBody => "documentBody",
      Kind::Directive => "directive",
      Kind::Comment => "comment",
      Kind::Tag => "tag",
      Kind::Anchor => "anchor",
      Kind::Alias => "alias",
      Kind::BlockLiteral => "blockLiteral",
      Kind::BlockFolded => "blockFolded",
      Kind::Plain => "plain",
      Kind::QuoteSingle => "quoteSingle",
      Kind::QuoteDouble => "quoteDouble",
      Kind::Mapping => "mapping",
      Kind::MappingItem => "mappingItem",
      Kind::MappingKey => "mappingKey",
      Kind::MappingValue => "mappingValue",
      Kind::Sequence => "sequence",
      Kind::SequenceItem => "sequenceItem",
      Kind::FlowMapping => "flowMapping",
      Kind::FlowMappingItem => "flowMappingItem",
      Kind::FlowSequence => "flowSequence",
      Kind::FlowSequenceItem => "flowSequenceItem",
    };
    let mut map = Map::new();
    map.insert("type".into(), json!(kind));
    map.insert("start".into(), self.point(node.position.start));
    map.insert("end".into(), self.point(node.position.end));
    let valued = matches!(
      node.kind,
      Kind::Comment
        | Kind::Tag
        | Kind::Anchor
        | Kind::Alias
        | Kind::BlockLiteral
        | Kind::BlockFolded
        | Kind::Plain
        | Kind::QuoteSingle
        | Kind::QuoteDouble
        | Kind::Directive
    );
    if valued {
      map.insert("value".into(), json!(node.value));
    }
    if node.kind == Kind::Directive {
      map.insert("parameters".into(), json!(node.parameters));
    }
    let leaf = matches!(
      node.kind,
      Kind::Comment
        | Kind::Tag
        | Kind::Anchor
        | Kind::Alias
        | Kind::BlockLiteral
        | Kind::BlockFolded
        | Kind::Plain
        | Kind::QuoteSingle
        | Kind::QuoteDouble
        | Kind::Directive
    );
    if !leaf {
      map.insert(
        "children".into(),
        Value::Array(node.children.iter().map(|it| self.node(*it)).collect()),
      );
    }
    let list = |ids: &[u32]| Value::Array(ids.iter().map(|it| self.node(*it)).collect());
    if !node.leading_comments.is_empty() {
      map.insert("leadingComments".into(), list(&node.leading_comments));
    }
    if !node.end_comments.is_empty() {
      map.insert("endComments".into(), list(&node.end_comments));
    }
    if !node.middle_comments.is_empty() {
      map.insert("middleComments".into(), list(&node.middle_comments));
    }
    for (key, value) in [
      ("trailingComment", node.trailing_comment),
      ("indicatorComment", node.indicator_comment),
      ("tag", node.tag),
      ("anchor", node.anchor),
    ] {
      if let Some(value) = value {
        map.insert(key.into(), self.node(value));
      }
    }
    if matches!(node.kind, Kind::BlockLiteral | Kind::BlockFolded) {
      let chomping = match node.chomping {
        Chomping::Clip => "clip",
        Chomping::Keep => "keep",
        Chomping::Strip => "strip",
      };
      map.insert("chomping".into(), json!(chomping));
      map.insert("indent".into(), json!(node.indent));
    }
    if node.directives_end_marker {
      map.insert("directivesEndMarker".into(), json!(true));
    }
    if node.document_end_marker {
      map.insert("documentEndMarker".into(), json!(true));
    }
    Value::Object(map)
  }
}

/// Compares which inputs Prettier's parser accepts with which tidy's does.
fn accepts(path: &str, show: usize) {
  let cases = read(path);
  let (mut same, mut shown) = (0, 0);
  for case in &cases {
    let name = case["name"].as_str().unwrap();
    let text = case["text"].as_str().unwrap();
    let expected = case["accepted"].as_bool().unwrap();
    let actual = tidy::yaml::parse(text).map(|_| ());
    if expected == actual.is_ok() {
      same += 1;
      continue;
    }
    if shown < show {
      shown += 1;
      let message = case["message"].as_str().unwrap_or_default();
      println!(
        "\n✗ {name}\n  text {:?}\n  prettier {}\n  tidy {:?}",
        text.chars().take(160).collect::<String>(),
        if expected { "accepts" } else { message },
        actual
      );
    }
  }
  println!("\naccepts: {same}/{} same", cases.len());
}

fn strip_messages(mut value: Value) -> Value {
  match &mut value {
    Value::Object(map) => {
      if map.get("type") == Some(&json!("error")) {
        map.remove("message");
        map.remove("source");
      }
      for value in map.values_mut() {
        *value = strip_messages(value.take());
      }
    }
    Value::Array(list) => {
      for value in list.iter_mut() {
        *value = strip_messages(value.take());
      }
    }
    _ => {}
  }
  value
}

fn first_difference(expected: &Value, actual: &Value, path: &str) {
  match (expected, actual) {
    (Value::Object(a), Value::Object(b)) => {
      for key in a.keys().chain(b.keys()) {
        if a.get(key) != b.get(key) {
          return match (a.get(key), b.get(key)) {
            (Some(x), Some(y)) => first_difference(x, y, &format!("{path}.{key}")),
            (x, y) => println!("  at {path}.{key}\n    yaml {x:?}\n    tidy {y:?}"),
          };
        }
      }
    }
    (Value::Array(a), Value::Array(b)) => {
      for index in 0..a.len().max(b.len()) {
        if a.get(index) != b.get(index) {
          return match (a.get(index), b.get(index)) {
            (Some(x), Some(y)) => first_difference(x, y, &format!("{path}[{index}]")),
            (x, y) => {
              println!("  at {path}[{index}]\n    yaml {}\n    tidy {}", short(x), short(y))
            }
          };
        }
      }
    }
    _ => {
      println!("  at {path}\n    yaml {}\n    tidy {}", short(Some(expected)), short(Some(actual)))
    }
  }
}

fn short(value: Option<&Value>) -> String {
  value.map(|it| it.to_string().chars().take(300).collect()).unwrap_or_else(|| "absent".into())
}

/// The UTF-16 offset of each byte offset, as `yaml` counts.
fn utf16_offsets(text: &str) -> Vec<u32> {
  let mut out = vec![0; text.len() + 1];
  let mut units = 0;
  for (at, character) in text.char_indices() {
    for byte in 0..character.len_utf8() {
      out[at + byte] = units;
    }
    units += character.len_utf16() as u32;
  }
  out[text.len()] = units;
  out
}

struct Dump<'d> {
  text: &'d str,
  utf16: Vec<u32>,
  tree: &'d Tree,
}

impl Dump<'_> {
  fn offset(&self, offset: u32) -> Value {
    json!(self.utf16[offset as usize])
  }

  fn slice(&self, start: u32, end: u32) -> Value {
    json!(self.text[start as usize..end as usize])
  }

  fn source(&self, source: &Source) -> Value {
    json!({
      "type": kind(source.kind),
      "offset": self.offset(source.offset),
      "indent": source.indent,
      "source": self.slice(source.offset, source.end),
    })
  }

  fn sources(&self, list: &[Source]) -> Value {
    Value::Array(list.iter().map(|it| self.source(it)).collect())
  }

  fn item(&self, item: &Item) -> Value {
    let mut map = Map::new();
    map.insert("start".into(), self.sources(&item.start));
    if item.explicit_key {
      map.insert("explicitKey".into(), json!(true));
    }
    match item.key {
      Key::Absent => {}
      Key::Null => drop(map.insert("key".into(), Value::Null)),
      Key::Token(id) => drop(map.insert("key".into(), self.node(id))),
    }
    if let Some(sep) = &item.sep {
      map.insert("sep".into(), self.sources(sep));
    }
    if let Some(value) = item.value {
      map.insert("value".into(), self.node(value));
    }
    Value::Object(map)
  }

  fn node(&self, id: u32) -> Value {
    let mut map = Map::new();
    let mut put = |key: &str, value: Value| drop(map.insert(key.into(), value));
    match self.tree.node(id) {
      Node::Source(source) => return self.source(source),
      Node::Error { offset, .. } => {
        put("type", json!("error"));
        put("offset", self.offset(*offset));
      }
      Node::Directive { offset, end } => {
        put("type", json!("directive"));
        put("offset", self.offset(*offset));
        put("source", self.slice(*offset, *end));
      }
      Node::Document { offset, start, value, end } => {
        put("type", json!("document"));
        put("offset", self.offset(*offset));
        put("start", self.sources(start));
        if let Some(value) = value {
          put("value", self.node(*value));
        }
        if let Some(end) = end {
          put("end", self.sources(end));
        }
      }
      Node::DocumentEnd { offset, source_end, end } => {
        put("type", json!("doc-end"));
        put("offset", self.offset(*offset));
        put("source", self.slice(*offset, *source_end));
        if let Some(end) = end {
          put("end", self.sources(end));
        }
      }
      Node::FlowScalar { kind, offset, source_end, indent, end } => {
        let name = match kind {
          ScalarKind::Alias => "alias",
          ScalarKind::Plain => "scalar",
          ScalarKind::Single => "single-quoted-scalar",
          ScalarKind::Double => "double-quoted-scalar",
        };
        put("type", json!(name));
        put("offset", self.offset(*offset));
        put("indent", json!(indent));
        put("source", self.slice(*offset, *source_end));
        if let Some(end) = end {
          put("end", self.sources(end));
        }
      }
      Node::BlockScalar { offset, indent, props, source } => {
        put("type", json!("block-scalar"));
        put("offset", self.offset(*offset));
        put("indent", json!(indent));
        let props = props.iter().map(|prop| match prop {
          Prop::Source(source) => self.source(source),
          Prop::Token(id) => self.node(*id),
        });
        put("props", Value::Array(props.collect()));
        put("source", self.slice(source.0, source.1));
      }
      Node::BlockMap { offset, indent, items } | Node::BlockSeq { offset, indent, items } => {
        let name = if matches!(self.tree.node(id), Node::BlockMap { .. }) {
          "block-map"
        } else {
          "block-seq"
        };
        put("type", json!(name));
        put("offset", self.offset(*offset));
        put("indent", json!(indent));
        put("items", Value::Array(items.iter().map(|it| self.item(it)).collect()));
      }
      Node::FlowCollection { offset, indent, start, items, end } => {
        put("type", json!("flow-collection"));
        put("offset", self.offset(*offset));
        put("indent", json!(indent));
        put("start", self.source(start));
        put("items", Value::Array(items.iter().map(|it| self.item(it)).collect()));
        put("end", self.sources(end));
      }
    }
    Value::Object(map)
  }
}

fn kind(kind: cst::Kind) -> &'static str {
  use cst::Kind::*;
  match kind {
    ByteOrderMark => "byte-order-mark",
    DocMode => "doc-mode",
    DocStart => "doc-start",
    DocEnd => "doc-end",
    Space => "space",
    Comment => "comment",
    Newline => "newline",
    DirectiveLine => "directive-line",
    Anchor => "anchor",
    Tag => "tag",
    SeqItemInd => "seq-item-ind",
    ExplicitKeyInd => "explicit-key-ind",
    MapValueInd => "map-value-ind",
    FlowMapStart => "flow-map-start",
    FlowMapEnd => "flow-map-end",
    FlowSeqStart => "flow-seq-start",
    FlowSeqEnd => "flow-seq-end",
    FlowErrorEnd => "flow-error-end",
    Comma => "comma",
    BlockScalarHeader => "block-scalar-header",
    Alias => "alias",
    Scalar => "scalar",
    SingleQuotedScalar => "single-quoted-scalar",
    DoubleQuotedScalar => "double-quoted-scalar",
  }
}
