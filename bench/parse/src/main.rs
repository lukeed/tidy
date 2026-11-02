//! Times tidy's parsers against the Rust crates that parse the same languages, on the corpora of `bench/run.ts`. Each
//! tool parses every file that all tools of its language accept, on one thread, from memory, and drops what it builds.
//! A pass is all those files once; the result is the median pass. The tools build different things, which each row
//! names: a tree for a formatter keeps comments and positions, and a value only keeps the data.
//!
//!   mise run bench:parse -- [--runs <n>] [--only <language,...>] [<corpus>]
//!
//! The corpus is `bench/run.ts`'s, in the temporary folder, by default: run `mise run bench` once to make it.

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

struct Tool {
  name: &'static str,
  builds: &'static str,
  parse: fn(&str) -> bool,
}

fn languages() -> [(&'static str, Vec<Tool>); 3] {
  let tool = |name, builds, parse| Tool { name, builds, parse };
  [
    (
      "json",
      vec![
        tool("tidy parse", "tree, comments", |text| keep(tidy::json::parse(text))),
        tool("tidy value", "serde_json value", |text| keep(tidy::json::value(text))),
        tool("jsonc-parser ast", "tree, comments", jsonc_ast),
        tool("jsonc-parser value", "own value", |text| {
          keep(jsonc_parser::parse_to_value(text, &Default::default()))
        }),
        tool("jsonc-parser serde", "serde_json value", |text| {
          keep(jsonc_parser::parse_to_serde_value::<serde_json::Value>(text, &Default::default()))
        }),
        tool("serde_json", "value, strict", |text| {
          keep(serde_json::from_str::<serde_json::Value>(text))
        }),
        tool("sonic-rs", "value, strict", |text| keep(sonic_rs::from_str::<sonic_rs::Value>(text))),
      ],
    ),
    (
      "toml",
      vec![
        tool("tidy parse", "tree, trivia", |text| keep(tidy::toml::parse(text))),
        tool("tidy value", "serde_json value", |text| keep(tidy::toml::value(text))),
        tool("taplo", "tree, trivia", |text| {
          let parse = taplo::parser::parse(text);
          parse.errors.is_empty() && keep::<_, ()>(Ok(parse.into_syntax()))
        }),
        tool("toml_edit", "document, trivia", |text| keep(text.parse::<toml_edit::DocumentMut>())),
        tool("toml", "value", |text| keep(toml::from_str::<toml::Table>(text))),
      ],
    ),
    (
      "yaml",
      vec![
        tool("tidy parse", "tree, comments", |text| keep(tidy::yaml::parse(text))),
        tool("yaml_parser", "tree, trivia", |text| keep(yaml_parser::parse(text))),
        tool("saphyr", "value", |text| {
          keep(<saphyr::Yaml as saphyr::LoadableYamlNode>::load_from_str(text))
        }),
        tool("yaml-rust2", "value", |text| keep(yaml_rust2::YamlLoader::load_from_str(text))),
        tool("serde_yaml", "value, libyaml", serde_yaml),
      ],
    ),
  ]
}

fn keep<T, E>(result: Result<T, E>) -> bool {
  black_box(result).is_ok()
}

fn jsonc_ast(text: &str) -> bool {
  let collect = jsonc_parser::CollectOptions {
    comments: jsonc_parser::CommentCollectionStrategy::Separate,
    tokens: false,
  };
  keep(jsonc_parser::parse_to_ast(text, &collect, &Default::default()))
}

fn serde_yaml(text: &str) -> bool {
  serde_yaml::Deserializer::from_str(text).all(|document| {
    use serde::Deserialize;
    keep(serde_yaml::Value::deserialize(document))
  })
}

fn main() {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let option = |name: &str| args.iter().position(|it| it == name).map(|at| args[at + 1].as_str());
  let runs = option("--runs").map_or(20, |runs| runs.parse().expect("--runs takes a number"));
  let only = option("--only").map(|only| only.split(',').collect::<Vec<_>>());
  let corpus = match args.len() % 2 {
    1 => PathBuf::from(&args[args.len() - 1]),
    _ => std::env::temp_dir().join("tidy-bench"),
  };
  assert!(
    corpus.is_dir(),
    "{} isn't a folder: run `mise run bench` to make the corpus",
    corpus.display()
  );
  for (language, tools) in languages() {
    if only.as_ref().is_some_and(|only| !only.contains(&language)) {
      continue;
    }
    let mut paths = Vec::new();
    walk(&corpus.join(language), &mut paths);
    let texts: Vec<String> =
      paths.iter().filter_map(|it| std::fs::read_to_string(it).ok()).collect();
    let mut rejected = vec![0; tools.len()];
    let texts: Vec<&str> = texts
      .iter()
      .map(String::as_str)
      .filter(|text| {
        let accepted: Vec<bool> = tools.iter().map(|tool| (tool.parse)(text)).collect();
        accepted
          .iter()
          .zip(&mut rejected)
          .for_each(|(accepted, count)| *count += !accepted as usize);
        accepted.iter().all(|it| *it)
      })
      .collect();
    let bytes: usize = texts.iter().map(|it| it.len()).sum();
    let mut passes = vec![Vec::with_capacity(runs); tools.len()];
    for _ in 0..runs {
      for (tool, times) in tools.iter().zip(&mut passes) {
        let start = Instant::now();
        for text in &texts {
          (tool.parse)(text);
        }
        times.push(start.elapsed().as_secs_f64());
      }
    }
    let medians: Vec<f64> = passes.iter_mut().map(|times| median(times)).collect();
    let fastest = medians.iter().copied().fold(f64::INFINITY, f64::min);
    println!(
      "\n{language}: {} of {} files, {:.2} MB",
      texts.len(),
      paths.len(),
      bytes as f64 / 1e6
    );
    for ((tool, time), rejected) in tools.iter().zip(&medians).zip(&rejected) {
      println!(
        "  {:20} {:18} {:8.2} ms {:7.0} MB/s {:6.2}x  rejects {}",
        tool.name,
        tool.builds,
        time * 1000.0,
        bytes as f64 / 1e6 / time,
        time / fastest,
        rejected
      );
    }
  }
}

fn median(times: &mut [f64]) -> f64 {
  times.sort_by(f64::total_cmp);
  times[times.len() / 2]
}

fn walk(path: &Path, out: &mut Vec<PathBuf>) {
  let Ok(entries) = std::fs::read_dir(path) else { return };
  for entry in entries.flatten() {
    match entry.file_type() {
      Ok(kind) if kind.is_dir() => walk(&entry.path(), out),
      Ok(kind) if kind.is_file() => out.push(entry.path()),
      _ => {}
    }
  }
}
