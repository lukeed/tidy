//! Times tidy's parsers against the Rust crates that parse the same languages, on the corpora of `bench/run.ts`. Each
//! tool parses every file that all tools of its language accept, on one thread, from memory, and drops what it builds.
//! A pass is all those files once; the result is the median pass. The tools build different things, which each row
//! names: a tree for a formatter keeps comments and positions, and a value only keeps the data.
//!
//!   mise run bench:parse -- [--runs <n>] [--only <language,...>] [--rejects] [<corpus>]
//!
//! `--rejects` lists the files that each tool rejects, and `--json` prints a line of JSON for each language.
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

fn languages() -> Vec<(&'static str, Vec<Tool>)> {
  let tool = |name, builds, parse| Tool { name, builds, parse };
  vec![
    (
      "json",
      vec![
        tool("tidy parse", "tree, comments", |text| keep(tidy::json::parse(text))),
        tool("tidy value", "serde_json value", |text| keep(tidy::json::value(text))),
        tool("jsonc-parser ast", "tree, comments", jsonc_ast),
        tool("oxc_parser", "JS tree, comments", oxc_json),
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
        tool("oxc-toml", "tree, trivia", |text| {
          let parse = oxc_toml::parse(text);
          parse.errors.is_empty() && keep::<_, ()>(Ok(parse))
        }),
      ],
    ),
    (
      "yaml",
      vec![
        tool("tidy parse", "tree, comments", |text| keep(tidy::yaml::parse(text))),
        tool("yaml_parser", "tree, trivia", |text| keep(yaml_parser::parse(text))),
        tool("oxc-yaml-parser", "tree, comments", |text| {
          let allocator = oxc_yaml_parser::Allocator::default();
          keep(oxc_yaml_parser::Parser::new(&allocator, text).parse())
        }),
        tool("saphyr", "value", |text| {
          keep(<saphyr::Yaml as saphyr::LoadableYamlNode>::load_from_str(text))
        }),
        tool("yaml-rust2", "value", |text| keep(yaml_rust2::YamlLoader::load_from_str(text))),
        tool("serde_yaml", "value, libyaml", serde_yaml),
      ],
    ),
    (
      "dockerfile",
      vec![
        tool("tidy", "tree, comments", tidy::bench::dockerfile),
        tool("dprint-plugin-dockerfile", "tree", |text| {
          keep(dprint_plugin_dockerfile::ast::Dockerfile::parse(text))
        }),
        tool("dockerfile-parser", "tree", |text| keep(dockerfile_parser::Dockerfile::parse(text))),
        tool("parse-dockerfile", "tree", |text| keep(parse_dockerfile::parse(text))),
      ],
    ),
    (
      "ini",
      vec![
        tool("tidy", "lines, comments", tidy::bench::ini),
        tool("rust-ini", "map", |text| keep(ini::Ini::load_from_str(text))),
        tool("configparser", "map, lowercase names", |text| {
          keep(configparser::ini::Ini::new().read(text.to_owned()))
        }),
      ],
    ),
    (
      "css",
      vec![
        tool("tidy", "tree, values as text", |text| {
          tidy::bench::css(text, tidy::css::Dialect::Css)
        }),
        tool("raffia", "tree, comments", |text| raffia(text, raffia::Syntax::Css)),
        tool("oxc-css-parser", "tree, comments", |text| oxc_css(text, oxc_css_parser::Syntax::Css)),
        tool("lightningcss", "typed rules", |text| {
          let options = lightningcss::stylesheet::ParserOptions::default();
          keep(lightningcss::stylesheet::StyleSheet::parse(text, options))
        }),
      ],
    ),
    (
      "scss",
      vec![
        tool("tidy", "tree, values as text", |text| {
          tidy::bench::css(text, tidy::css::Dialect::Scss)
        }),
        tool("raffia", "tree, comments", |text| raffia(text, raffia::Syntax::Scss)),
        tool("oxc-css-parser", "tree, comments", |text| {
          oxc_css(text, oxc_css_parser::Syntax::Scss)
        }),
      ],
    ),
    (
      "less",
      vec![
        tool("tidy", "tree, values as text", |text| {
          tidy::bench::css(text, tidy::css::Dialect::Less)
        }),
        tool("raffia", "tree, comments", |text| raffia(text, raffia::Syntax::Less)),
        tool("oxc-css-parser", "tree, comments", |text| {
          oxc_css(text, oxc_css_parser::Syntax::Less)
        }),
      ],
    ),
    (
      "html",
      vec![
        tool("tidy", "tree, strict", tidy::bench::html),
        tool("scraper", "spec DOM", |text| keep::<_, ()>(Ok(scraper::Html::parse_document(text)))),
        tool("tl", "tree, lenient", |text| keep(tl::parse(text, tl::ParserOptions::default()))),
        tool("oxc-htmlish-parser", "tree, lenient", |text| {
          let allocator = oxc_htmlish_parser::Allocator::default();
          let parser =
            oxc_htmlish_parser::Parser::new(&allocator, text, oxc_htmlish_parser::Dialect::Html);
          keep(parser.parse())
        }),
      ],
    ),
    (
      "xml",
      vec![
        tool("tidy", "tree, comments", tidy::bench::xml),
        tool("roxmltree", "tree", |text| {
          let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
          keep(roxmltree::Document::parse_with_options(text, options))
        }),
        tool("quick-xml", "events", quick_xml),
      ],
    ),
    (
      "handlebars",
      vec![
        tool("tidy", "HTML and tags", tidy::bench::handlebars),
        tool("handlebars", "tags only", |text| keep(handlebars::Template::compile(text))),
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

/// As oxfmt reads JSON: an expression in parentheses, with the JavaScript parser.
fn oxc_json(text: &str) -> bool {
  let allocator = oxc_allocator::Allocator::default();
  let source = allocator.alloc_concat_strs_array(["(", text, "\n)"]);
  let options = oxc_parser::ParseOptions { preserve_parens: false, ..Default::default() };
  let parser = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::default());
  let parsed = black_box(parser.with_options(options).parse());
  parsed.diagnostics.is_empty() && !parsed.fatal_error
}

fn oxc_css(text: &str, syntax: oxc_css_parser::Syntax) -> bool {
  let allocator = oxc_css_parser::Allocator::default();
  let mut parser =
    oxc_css_parser::ParserBuilder::new(&allocator, text).syntax(syntax).comments().build();
  keep(parser.parse::<oxc_css_parser::ast::Stylesheet>())
}

fn raffia(text: &str, syntax: raffia::Syntax) -> bool {
  let mut comments = Vec::new();
  let mut parser = raffia::ParserBuilder::new(text).syntax(syntax).comments(&mut comments).build();
  keep(parser.parse::<raffia::ast::Stylesheet>())
}

fn quick_xml(text: &str) -> bool {
  let mut reader = quick_xml::Reader::from_str(text);
  loop {
    match black_box(reader.read_event()) {
      Ok(quick_xml::events::Event::Eof) => return true,
      Ok(_) => {}
      Err(_) => return false,
    }
  }
}

fn serde_yaml(text: &str) -> bool {
  serde_yaml::Deserializer::from_str(text).all(|document| {
    use serde::Deserialize;
    keep(serde_yaml::Value::deserialize(document))
  })
}

fn main() {
  let all: Vec<String> = std::env::args().skip(1).collect();
  let flag = |name: &str| all.iter().any(|it| it == name);
  let (rejects, json) = (flag("--rejects"), flag("--json"));
  let args: Vec<&str> =
    all.iter().map(String::as_str).filter(|it| !matches!(*it, "--rejects" | "--json")).collect();
  let option = |name: &str| args.iter().position(|it| *it == name).map(|at| args[at + 1]);
  let runs = option("--runs").map_or(20, |runs| runs.parse().expect("--runs takes a number"));
  let only = option("--only").map(|only| only.split(',').collect::<Vec<_>>());
  let corpus = match args.len() % 2 {
    1 => PathBuf::from(args[args.len() - 1]),
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
    let files: Vec<(&PathBuf, String)> =
      paths.iter().filter_map(|path| Some((path, std::fs::read_to_string(path).ok()?))).collect();
    let mut rejected = vec![0; tools.len()];
    let mut texts = Vec::new();
    for (path, text) in &files {
      let accepted: Vec<bool> = tools.iter().map(|tool| (tool.parse)(text)).collect();
      for ((tool, accepted), count) in tools.iter().zip(&accepted).zip(&mut rejected) {
        if !accepted {
          *count += 1;
          if rejects {
            eprintln!("{language}: {} rejects {}", tool.name, path.display());
          }
        }
      }
      if accepted.iter().all(|it| *it) {
        texts.push(text.as_str());
      }
    }
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
    if json {
      let rows = tools.iter().zip(&medians).zip(&rejected).map(|((tool, time), rejected)| {
        serde_json::json!({ "name": tool.name, "builds": tool.builds, "ms": time * 1000.0, "rejects": rejected })
      });
      let line = serde_json::json!({
        "language": language,
        "files": texts.len(),
        "total": paths.len(),
        "bytes": bytes,
        "runs": runs,
        "tools": rows.collect::<Vec<_>>(),
      });
      println!("{line}");
      continue;
    }
    println!(
      "\n{language}: {} of {} files, {:.2} MB",
      texts.len(),
      paths.len(),
      bytes as f64 / 1e6
    );
    for ((tool, time), rejected) in tools.iter().zip(&medians).zip(&rejected) {
      println!(
        "  {:24} {:18} {:8.2} ms {:7.0} MB/s {:6.2}x  rejects {}",
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
