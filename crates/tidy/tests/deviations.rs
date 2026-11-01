//! Every known deviation from Prettier 3.9.9, and from taplo for TOML, listed in `deviations.md`. Each row holds an
//! input, what the reference prints and what tidy prints. A row fails if tidy prints something else, or if the
//! reference's output is now the same.

use serde_json::Value;
use tidy::embed::Verbatim;
use tidy::{Language, css, handlebars, html, toml, xml};

fn format(parser: &str, input: &str, option: Option<&str>) -> String {
  let whitespace = match option {
    Some("ignore") => tidy::Whitespace::Ignore,
    Some("strict") => tidy::Whitespace::Strict,
    _ => tidy::Whitespace::Auto,
  };
  let result = match parser {
    "html" | "vue" if input.starts_with("+++") => {
      let options = tidy::Options::default();
      tidy::format(input, Language::Html, &options, None, &mut Verbatim)
        .map_err(|error| error.to_string())
    }
    "html" | "vue" => {
      let parser = if parser == "vue" { html::Parser::Vue } else { html::Parser::Html };
      let options =
        html::Options { parser, whitespace, format_embedded: false, ..html::Options::default() };
      html::format(input, &options, &mut Verbatim).map_err(|error| error.to_string())
    }
    "markdown" => {
      let options = tidy::Options::default();
      tidy::format(input, Language::Markdown, &options, None, &mut Verbatim)
        .map_err(|error| error.to_string())
    }
    "toml" => toml::format(input, &toml::Options::default()).map_err(|error| error.to_string()),
    "handlebars" => {
      let dialect = handlebars::Dialect::Handlebars;
      let options = handlebars::Options { dialect, whitespace, ..handlebars::Options::default() };
      handlebars::format(input, &options, &mut Verbatim).map_err(|error| error.to_string())
    }
    "css" | "scss" | "less" => {
      let dialect = match parser {
        "scss" => css::Dialect::Scss,
        "less" => css::Dialect::Less,
        _ => css::Dialect::Css,
      };
      let options = css::Options { dialect, ..css::Options::default() };
      css::format(input, &options, &mut Verbatim).map_err(|error| error.to_string())
    }
    "xml" => {
      let whitespace = match option {
        Some("ignore") => tidy::Whitespace::Ignore,
        Some("preserve") => tidy::Whitespace::Auto,
        _ => tidy::Whitespace::Strict,
      };
      let options = xml::Options { whitespace, ..xml::Options::default() };
      xml::format(input, &options, &mut Verbatim).map_err(|error| error.to_string())
    }
    other => panic!("unknown parser {other}"),
  };
  result.unwrap_or_else(|error| format!("!error {error}"))
}

#[test]
fn every_deviation_holds() {
  let entries: Vec<Value> = serde_json::from_str(include_str!("deviations.json")).unwrap();
  let mut failures = Vec::new();
  for entry in &entries {
    let id = entry["id"].as_str().unwrap();
    for row in entry["rows"].as_array().unwrap() {
      let input = row["input"].as_str().unwrap();
      let whitespace = row.get("whitespace").and_then(Value::as_str);
      for parser in row["parsers"].as_array().unwrap() {
        let parser = parser.as_str().unwrap();
        let printed = format(parser, input, whitespace);
        if printed != row["tidy"].as_str().unwrap() {
          failures.push(format!(
            "{id} [{parser}] {input:?}\n  expected {:?}\n  printed  {printed:?}",
            row["tidy"]
          ));
        } else if printed == row["prettier"].as_str().unwrap() {
          failures.push(format!("{id} [{parser}] {input:?}: the same as Prettier now"));
        }
      }
    }
  }
  assert!(failures.is_empty(), "{} rows:\n{}", failures.len(), failures.join("\n"));
}
