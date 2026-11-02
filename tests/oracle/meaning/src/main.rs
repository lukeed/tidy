//! Checks that tidy's JavaScript and TypeScript output means what its input means: both parse to the same ESTree,
//! leaving out what Prettier changes on purpose (positions, parentheses, quotes, the order of regular expression flags,
//! empty statements, white space in JSX, the layout of Jest's tables), and their comments have the same words. The Prettier oracle can't see a change of
//! meaning that Prettier makes too; this one can.
//!
//!   cargo run --release -- [--show=n] [folder...]
//!
//! Without folders, it reads Prettier's fixtures of JavaScript, JSX and TypeScript, and the bench corpus of bench/js.ts.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

fn main() {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let show =
    args.iter().find_map(|it| it.strip_prefix("--show=")).map_or(5, |it| it.parse().unwrap());
  let mut dirs: Vec<PathBuf> =
    args.iter().filter(|it| !it.starts_with("--")).map(PathBuf::from).collect();
  if dirs.is_empty() {
    let fixtures =
      Path::new(&std::env::var("HOME").unwrap()).join("repos/oss/prettier/tests/format");
    dirs = vec![fixtures.join("js"), fixtures.join("jsx"), fixtures.join("typescript")];
    dirs.push(std::env::temp_dir().join("tidy-bench-js"));
  }
  let mut paths = Vec::new();
  for dir in &dirs {
    collect(dir, &mut paths);
  }
  let (mut same, mut different, mut comments, mut unparsable, mut skipped, mut shown) =
    (0, 0, 0, 0, 0, 0);
  for path in &paths {
    let Ok(text) = std::fs::read_to_string(path) else { continue };
    let name = path.to_string_lossy();
    let Some(dialect) = tidy::js::Dialect::of_path(&name) else { continue };
    let options =
      tidy::js::Options { dialect, jsdoc: false, format_embedded: false, ..Default::default() };
    let Ok(output) = tidy::js::format(&text, &name, &options) else {
      skipped += 1;
      continue;
    };
    let Some((before, notes)) = read(&text, &name) else {
      skipped += 1;
      continue;
    };
    let Some((after, again)) = read(&output, &name) else {
      unparsable += 1;
      report(&mut shown, show, &name, "the output doesn't parse".into());
      continue;
    };
    if before != after {
      different += 1;
      report(&mut shown, show, &name, difference(&before, &after, String::new()));
    } else if notes != again {
      comments += 1;
      report(&mut shown, show, &name, format!("comments differ:\n  {notes:?}\n  {again:?}"));
    } else {
      same += 1;
    }
  }
  println!(
    "same {same}, different {different}, comments differ {comments}, output doesn't parse {unparsable}, skipped {skipped}"
  );
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
  if path.is_file() {
    let name = path.to_string_lossy();
    if tidy::js::Dialect::of_path(&name).is_some() && !name.ends_with(".js.flow") {
      out.push(path.to_path_buf());
    }
    return;
  }
  let Ok(entries) = std::fs::read_dir(path) else { return };
  let mut children: Vec<PathBuf> = entries.flatten().map(|it| it.path()).collect();
  children.sort();
  for child in children {
    let name = child.file_name().unwrap_or_default().to_string_lossy();
    let copy =
      ["-prettier", "-tidy", "-oxfmt", "-biome", "-bun"].iter().any(|end| name.ends_with(end));
    if name == "node_modules" || name == "__snapshots__" || copy {
      continue;
    }
    collect(&child, out);
  }
}

fn report(shown: &mut usize, show: usize, name: &str, what: String) {
  if *shown < show {
    println!("--- {name}\n{what}");
  }
  *shown += 1;
}

/// The normalized tree of `text` and its comments, or `None` if it doesn't parse.
fn read(text: &str, path: &str) -> Option<(Value, Vec<String>)> {
  let lower = path.to_ascii_lowercase();
  let base = match lower.rsplit_once('.').map(|(_, extension)| extension) {
    Some("ts" | "mts" | "cts") => oxc_span::SourceType::ts(),
    Some("tsx") => oxc_span::SourceType::tsx(),
    _ => oxc_span::SourceType::jsx(),
  };
  let options = oxc_parser::ParseOptions {
    preserve_parens: false,
    allow_return_outside_function: true,
    ..Default::default()
  };
  for source_type in [base.with_module(true), base.with_script(true)] {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
      oxc_parser::Parser::new(&allocator, text, source_type).with_options(options).parse();
    if parsed.fatal_error || !parsed.diagnostics.is_empty() && source_type.is_module() {
      continue;
    }
    let json: Value = serde_json::from_str(&parsed.program.to_estree_json(true, false)).ok()?;
    let comments = parsed.program.comments.iter();
    let mut notes: Vec<String> = comments
      .flat_map(|it| text[it.span.start as usize..it.span.end as usize].split_whitespace())
      .map(str::to_owned)
      .collect();
    notes.sort();
    return Some((normalize(json), notes));
  }
  None
}

fn normalize(value: Value) -> Value {
  match value {
    Value::Array(items) => {
      let items = items.into_iter().filter(|it| kind(it) != Some("EmptyStatement"));
      Value::Array(items.map(normalize).collect())
    }
    Value::Object(map) => normalize_object(map),
    other => other,
  }
}

fn kind(value: &Value) -> Option<&str> {
  value.get("type").and_then(Value::as_str)
}

fn normalize_object(mut map: Map<String, Value>) -> Value {
  if kind_of(&map) == Some("LogicalExpression") {
    return logical(map);
  }
  // `(a?.b)?.c` is `a?.b?.c`: the outer access is optional, so it short-circuits as the chain does.
  if map.get("optional") == Some(&Value::Bool(true)) {
    for key in ["object", "callee"] {
      if let Some(inner) = map.get(key).filter(|it| kind(it) == Some("ChainExpression")).cloned() {
        map.insert(key.into(), inner["expression"].clone());
      }
    }
  }
  // `sourceType` is what read the text: a script where a module had errors.
  for key in ["start", "end", "range", "loc", "raw", "comments", "extra", "sourceType"] {
    map.remove(key);
  }
  // `A & B` with one member is the member.
  if matches!(kind_of(&map), Some("TSUnionType" | "TSIntersectionType"))
    && let Some(Value::Array(types)) = map.get("types")
    && types.len() == 1
  {
    return normalize(types[0].clone());
  }
  if kind_of(&map) == Some("Hashbang")
    && let Some(Value::String(value)) = map.get_mut("value")
  {
    *value = value.trim_end().to_owned();
  }
  // Prettier writes a quote that a JSX string can hold as itself.
  if matches!(kind_of(&map), Some("Literal" | "JSXText"))
    && let Some(Value::String(value)) = map.get_mut("value")
  {
    *value = value
      .replace("&quot;", "\"")
      .replace("&apos;", "'")
      .replace("&#39;", "'")
      .replace("&#34;", "\"");
  }
  // Prettier aligns the table of Jest's `describe.each` and `test.each`.
  if kind_of(&map) == Some("TaggedTemplateExpression")
    && map.get("tag").and_then(|tag| tag.get("property")).and_then(|it| it.get("name"))
      == Some(&Value::String("each".into()))
    && let Some(Value::Array(quasis)) = map.get_mut("quasi").and_then(|it| it.get_mut("quasis"))
  {
    let mut table: String =
      quasis.iter().filter_map(|it| it["value"]["cooked"].as_str()).collect::<Vec<_>>().join("${}");
    table.retain(|c| !c.is_whitespace() && c != '|');
    *quasis = vec![Value::String(table)];
  }
  if let Some(Value::Object(regex)) = map.get_mut("regex")
    && let Some(Value::String(flags)) = regex.get_mut("flags")
  {
    let mut sorted: Vec<char> = flags.chars().collect();
    sorted.sort();
    *flags = sorted.into_iter().collect();
  }
  // A key that isn't computed is its name: Prettier adds and removes quotes, as `quoteProps` says.
  if kind_of(&map) == Some("TSEnumMember")
    && let Some(id) = map.remove("id")
  {
    map.insert("key".into(), id);
    map.insert("computed".into(), Value::Bool(false));
  }
  if (map.get("computed") == Some(&Value::Bool(false)) || kind_of(&map) == Some("ImportAttribute"))
    && let Some(key) = map.get("key").cloned()
  {
    let name = match kind(&key) {
      Some("Identifier") => key.get("name").cloned(),
      Some("Literal") => key.get("value").map(|it| Value::String(text_of(it))),
      _ => None,
    };
    if let Some(name) = name {
      map.insert("key".into(), Value::String(format!("key {}", name.as_str().unwrap_or_default())));
    }
  }
  if matches!(map.get("type").and_then(Value::as_str), Some("JSXElement" | "JSXFragment"))
    && let Some(Value::Array(children)) = map.remove("children")
  {
    map.insert("children".into(), Value::Array(jsx_children(children)));
  }
  Value::Object(map.into_iter().map(|(key, value)| (key, normalize(value))).collect())
}

fn kind_of(map: &Map<String, Value>) -> Option<&str> {
  map.get("type").and_then(Value::as_str)
}

/// `a || (b || c)` is `a || b || c`: `||`, `&&` and `??` evaluate their operands in order, however they're grouped.
fn logical(map: Map<String, Value>) -> Value {
  let operator = map["operator"].clone();
  let mut operands = Vec::new();
  let mut stack = vec![Value::Object(map)];
  while let Some(node) = stack.pop() {
    match node {
      Value::Object(inner)
        if kind_of(&inner) == Some("LogicalExpression") && inner["operator"] == operator =>
      {
        stack.push(inner["right"].clone());
        stack.push(inner["left"].clone());
      }
      other => operands.push(normalize(other)),
    }
  }
  let mut out = Map::new();
  out.insert("type".into(), Value::String("Logical".into()));
  out.insert("operator".into(), operator);
  out.insert("operands".into(), Value::Array(operands));
  Value::Object(out)
}

fn text_of(value: &Value) -> String {
  match value {
    Value::String(text) => text.clone(),
    Value::Number(number) => {
      number.as_f64().map_or_else(|| number.to_string(), |it| it.to_string())
    }
    other => other.to_string(),
  }
}

/// The children of an element as React reads them: text with its white space cleaned up as JSX does, where `{" "}`
/// and other strings in braces are text too, and without containers of comments only.
fn jsx_children(children: Vec<Value>) -> Vec<Value> {
  let mut out: Vec<Value> = Vec::new();
  let mut text = String::new();
  for child in children {
    let piece = match kind(&child) {
      Some("JSXText") => {
        Some(clean(child.get("value").and_then(Value::as_str).unwrap_or_default()))
      }
      Some("JSXExpressionContainer") => match child.get("expression") {
        Some(e) if kind(e) == Some("JSXEmptyExpression") => continue,
        Some(e) if kind(e) == Some("Literal") && e.get("value").is_some_and(Value::is_string) => {
          Some(e["value"].as_str().unwrap_or_default().to_owned())
        }
        _ => None,
      },
      _ => None,
    };
    match piece {
      Some(piece) => text.push_str(&piece),
      None => {
        if !text.is_empty() {
          out.push(Value::String(std::mem::take(&mut text)));
        }
        out.push(child);
      }
    }
  }
  if !text.is_empty() {
    out.push(Value::String(text));
  }
  // Prettier writes a run of spaces in JSX text as one, which a browser shows alike.
  for item in &mut out {
    if let Value::String(text) = item {
      while text.contains("  ") {
        *text = text.replace("  ", " ");
      }
    }
  }
  out
}

/// Babel's `cleanJSXElementLiteralChild`: tabs are spaces, each line loses the spaces at its start (but the first) and
/// at its end (but the last), empty lines go, and the others are joined with one space.
fn clean(text: &str) -> String {
  let text = text.replace("\r\n", "\n");
  let lines: Vec<&str> = text.split(['\n', '\r']).collect();
  let last_non_empty =
    lines.iter().rposition(|line| line.contains(|c| c != ' ' && c != '\t')).unwrap_or(0);
  let mut out = String::new();
  for (index, line) in lines.iter().enumerate() {
    let mut line = line.replace('\t', " ");
    if index > 0 {
      line = line.trim_start_matches(' ').to_owned();
    }
    if index < lines.len() - 1 {
      line = line.trim_end_matches(' ').to_owned();
    }
    if line.is_empty() {
      continue;
    }
    if index != last_non_empty {
      line.push(' ');
    }
    out.push_str(&line);
  }
  out
}

/// Where `before` and `after` first differ, as a path into the tree.
fn difference(before: &Value, after: &Value, at: String) -> String {
  match (before, after) {
    (Value::Object(left), Value::Object(right)) => {
      for (key, value) in left {
        match right.get(key) {
          Some(other) if other == value => {}
          Some(other) => return difference(value, other, format!("{at}.{key}")),
          None => return format!("{at}.{key} is gone"),
        }
      }
      format!(
        "{at} has more keys: {:?}",
        right.keys().filter(|it| !left.contains_key(*it)).collect::<Vec<_>>()
      )
    }
    (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
      let index = left.iter().zip(right).position(|(a, b)| a != b).unwrap_or(0);
      difference(&left[index], &right[index], format!("{at}[{index}]"))
    }
    _ => {
      let cut = |value: &Value| value.to_string().chars().take(300).collect::<String>();
      format!("{at}:\n  before {}\n  after  {}", cut(before), cut(after))
    }
  }
}
