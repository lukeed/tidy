//! Compares tidy's Dockerfile formatter with dprint-plugin-dockerfile 0.7.0: on the plugin's specs, on the files under
//! folders, and on random edits of their lines. It also checks that tidy doesn't change its own output.
//!
//!   cargo run --release -- specs <tests/specs>
//!   cargo run --release -- files [--show=n] <folder>...
//!   cargo run --release -- fuzz [count] <folder>...

use dprint_plugin_dockerfile::configuration::{Configuration, ConfigurationBuilder};
use std::path::{Path, PathBuf};
use tidy::doc::Eol;
use tidy::dockerfile::Options;

fn main() {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let show = args
    .iter()
    .find_map(|it| it.strip_prefix("--show="))
    .and_then(|it| it.parse().ok())
    .unwrap_or(5);
  let rest: Vec<&String> = args.iter().skip(1).filter(|it| !it.starts_with("--")).collect();
  match args.first().map(String::as_str) {
    Some("specs") => specs(Path::new(rest[0]), show),
    Some("files") => files(&rest, show),
    Some("passes") => passes(&rest),
    Some("one") => {
      let options = sets().into_iter().find(|(name, _)| name == rest[0]).expect("an option set").1;
      let text = std::fs::read_to_string(rest[1].as_str()).unwrap();
      let once = tidy(&text, &options);
      println!("{once}\n-----\n{}", tidy(&once, &options));
    }
    Some("fuzz") => {
      let count = rest.first().and_then(|it| it.parse().ok());
      let folders = if count.is_some() { &rest[1..] } else { &rest[..] };
      fuzz(count.unwrap_or(100_000), folders, show)
    }
    _ => eprintln!("usage: specs <dir> | files [--show=n] <folder>... | fuzz [count] <folder>..."),
  }
}

/// An option set: its name, and the same options for each formatter.
fn sets() -> Vec<(&'static str, Options)> {
  let tidy = Options::default();
  vec![
    ("default", tidy.clone()),
    ("width 40", Options { width: 40, ..tidy.clone() }),
    ("indent stages", Options { indent_stages: true, indent_width: 4, ..tidy.clone() }),
    ("healthcheck on its own line", Options { healthcheck_cmd_new_line: true, ..tidy.clone() }),
    ("crlf", Options { eol: Eol::Crlf, ..tidy }),
  ]
}

fn dprint_config(options: &Options) -> Configuration {
  let mut builder = ConfigurationBuilder::new();
  builder
    .line_width(options.width as u32)
    .indent_stages(options.indent_stages)
    .indent_width(options.indent_width as u8)
    .healthcheck_cmd_new_line(options.healthcheck_cmd_new_line);
  if options.eol == Eol::Crlf {
    builder.new_line_kind(dprint_core::configuration::NewLineKind::CarriageReturnLineFeed);
  }
  builder.build()
}

/// What dprint prints on one pass.
fn dprint(text: &str, options: &Options) -> String {
  let config = dprint_config(options);
  let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
  match dprint_plugin_dockerfile::format_text(&PathBuf::from("Dockerfile"), text, &config) {
    Ok(Some(formatted)) => formatted,
    Ok(None) => text.to_owned(),
    Err(error) => format!("!error {error}"),
  }
}

fn tidy(text: &str, options: &Options) -> String {
  let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
  tidy::dockerfile::format(text, options).unwrap_or_else(|error| format!("!error {error}"))
}

fn files(folders: &[&String], show: usize) {
  let mut sources = Vec::new();
  for folder in folders {
    walk(Path::new(folder.as_str()), &mut sources);
  }
  let (mut same, mut total, mut shown, mut unstable) = (0, 0, 0, 0);
  for (name, options) in sets() {
    let mut different = 0;
    for (path, text) in &sources {
      total += 1;
      let expected = dprint(text, &options);
      let actual = tidy(text, &options);
      if tidy(&actual, &options) != actual {
        unstable += 1;
        if shown < show {
          shown += 1;
          println!("\n↻ {name}: {} changes when formatted again", path.display());
          diff(&actual, &tidy(&actual, &options));
        }
      }
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
    "\ndockerfile: {same}/{total} same ({:.1}%), {} files",
    same as f64 * 100.0 / total as f64,
    sources.len()
  );
  println!("tidy changed its own output {unstable} times");
}

/// How tidy compares with dprint's first pass, and the lines where dprint's second pass changes its first.
fn passes(folders: &[&String]) {
  let mut sources = Vec::new();
  for folder in folders {
    walk(Path::new(folder.as_str()), &mut sources);
  }
  let options = Options::default();
  let config = dprint_config(&options);
  let path = PathBuf::from("Dockerfile");
  let once = |text: &str| match dprint_plugin_dockerfile::format_text(&path, text, &config) {
    Ok(Some(next)) => next,
    _ => text.to_owned(),
  };
  let (mut first, mut stable, mut tidy_first) = (0, 0, 0);
  let mut kinds = std::collections::BTreeMap::<String, usize>::new();
  for (file, text) in &sources {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let pass1 = once(text);
    let pass2 = once(&pass1);
    first += 1;
    if pass1 == pass2 {
      stable += 1;
    } else {
      let line = pass1
        .lines()
        .zip(pass2.lines())
        .find(|(a, b)| a != b)
        .map(|(a, _)| a.trim().to_owned())
        .unwrap_or_default();
      let kind = line.split_whitespace().next().unwrap_or("").to_owned();
      *kinds.entry(kind).or_default() += 1;
      let _ = file;
    }
    if tidy(text, &options) == pass1 {
      tidy_first += 1;
    }
  }
  println!(
    "files {first}, dprint stable after one pass {stable}, tidy == dprint's first pass {tidy_first}"
  );
  for (kind, count) in kinds {
    println!("  first line pass 2 changes starts with {kind:?}: {count}");
  }
}

/// The specs of dprint-plugin-dockerfile, but range formatting: `~~ option: value ~~`, then cases of `== name ==`,
/// the input, `[expect]` and the output.
fn specs(dir: &Path, show: usize) {
  let mut paths = Vec::new();
  collect(dir, &mut paths);
  let (mut same, mut total, mut shown) = (0, 0, 0);
  for path in paths {
    if path.components().any(|it| it.as_os_str() == "Range") {
      continue;
    }
    let text = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    let (options, body) = spec_options(&text);
    for case in body.split("\n== ").map(|it| it.trim_start_matches("== ")) {
      let Some((name, rest)) = case.split_once(" ==\n") else { continue };
      let Some((input, expected)) = rest.split_once("\n[expect]\n") else { continue };
      let input = format!("{input}\n");
      let expected = expected.trim_end_matches('\n').to_owned() + "\n";
      let expected = if expected.trim().is_empty() { String::new() } else { expected };
      total += 1;
      let actual = tidy(&input, &options);
      if actual == expected || actual == expected.trim_end() {
        same += 1;
      } else if shown < show {
        shown += 1;
        println!("\n✗ {}: {name}", path.display());
        diff(&expected, &actual);
      }
    }
  }
  println!("\nspecs: {same}/{total} same");
}

fn spec_options(text: &str) -> (Options, &str) {
  let mut options = Options::default();
  let Some(rest) = text.strip_prefix("~~ ") else { return (options, text) };
  let (header, body) = rest.split_once(" ~~\n").unwrap();
  for pair in header.split(',') {
    let (key, value) = pair.split_once(':').unwrap();
    let value = value.trim();
    match key.trim() {
      "lineWidth" => options.width = value.parse().unwrap(),
      "indentStages" => options.indent_stages = value == "true",
      "indentWidth" => options.indent_width = value.parse().unwrap(),
      "healthcheckCmdNewLine" => options.healthcheck_cmd_new_line = value == "true",
      "newLineKind" if value == "crlf" => options.eol = Eol::Crlf,
      _ => {}
    }
  }
  (options, body)
}

/// Random edits of the corpus's lines: lines joined, split with continuations, spaced, quoted and commented.
fn fuzz(count: usize, folders: &[&String], show: usize) {
  let mut sources = Vec::new();
  for folder in folders {
    walk(Path::new(folder.as_str()), &mut sources);
  }
  let lines: Vec<String> = sources
    .iter()
    .flat_map(|(_, text)| text.lines().map(str::to_owned))
    .filter(|it| !it.is_empty())
    .collect();
  let mut seed = 0x2545_f491_4f6c_dd1du64;
  let mut next = |bound: usize| {
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    (seed % bound as u64) as usize
  };
  let pieces = [
    " ", "  ", "\t", " \\\n", "\\\n  ", "\n", "\n\n", "#", "# c\n", "\"", "'", "=", "<<EOF\n",
    "\nEOF", "[",
  ];
  let options = Options::default();
  let (mut same, mut unsettled, mut unstable, mut shown, mut shown_unstable) = (0, 0, 0, 0, 0);
  for _ in 0..count {
    let mut text = String::new();
    for _ in 0..1 + next(4) {
      text.push_str(&lines[next(lines.len())]);
      if next(3) == 0 {
        let at = text
          .char_indices()
          .map(|(at, _)| at)
          .nth(next(text.chars().count().max(1)))
          .unwrap_or(text.len());
        text.insert_str(at, pieces[next(pieces.len())]);
      }
      text.push('\n');
    }
    let expected = dprint(&text, &options);
    let actual = tidy(&text, &options);
    if tidy(&actual, &options) != actual {
      unstable += 1;
      if shown_unstable < show {
        shown_unstable += 1;
        println!("\n↻ {text:?}");
        diff(&actual, &tidy(&actual, &options));
      }
    }
    if expected == actual {
      same += 1;
    } else if dprint(&expected, &options) != expected {
      unsettled += 1;
    } else if shown < show {
      shown += 1;
      println!("\n✗ {text:?}");
      diff(&expected, &actual);
    }
  }
  println!("\nfuzz: {} differ where dprint changes its own output", unsettled);
  println!("fuzz: tidy changed its own output {unstable} times");
  println!("\nfuzz: {same}/{count} same");
}

fn walk(path: &Path, out: &mut Vec<(PathBuf, String)>) {
  let mut paths = Vec::new();
  collect(path, &mut paths);
  for path in paths {
    if let Ok(text) = std::fs::read_to_string(&path) {
      out.push((path, text));
    }
  }
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
  if path.is_file() {
    out.push(path.to_path_buf());
    return;
  }
  let Ok(entries) = std::fs::read_dir(path) else { return };
  let mut entries: Vec<_> = entries.flatten().map(|it| it.path()).collect();
  entries.sort();
  for entry in entries {
    if entry.file_name().is_some_and(|it| it == ".git") {
      continue;
    }
    collect(&entry, out);
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
