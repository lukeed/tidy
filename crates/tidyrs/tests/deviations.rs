//! Every known deviation from Prettier 3.9.9, listed in `deviations.md`. Each row holds an input, what Prettier prints
//! and what tidyrs prints. A row fails if tidyrs prints something else, or if Prettier's output is now the same.

use serde_json::Value;
use tidyrs::embed::Verbatim;
use tidyrs::{css, handlebars, html, xml};

fn format(parser: &str, input: &str, option: Option<&str>) -> String {
  let whitespace = match option {
    Some("ignore") => html::Whitespace::Ignore,
    Some("strict") => html::Whitespace::Strict,
    _ => html::Whitespace::Css,
  };
  let result = match parser {
    "html" | "vue" => {
      let parser = if parser == "vue" { html::Parser::Vue } else { html::Parser::Html };
      let settings =
        html::Settings { parser, whitespace, format_embedded: false, ..html::Settings::default() };
      html::format(input, &settings, &mut Verbatim).map_err(|error| error.to_string())
    }
    "handlebars" => {
      let dialect = handlebars::Dialect::Handlebars;
      let settings =
        handlebars::Settings { dialect, whitespace, ..handlebars::Settings::default() };
      handlebars::format(input, &settings, &mut Verbatim).map_err(|error| error.to_string())
    }
    "css" | "scss" | "less" => {
      let dialect = match parser {
        "scss" => css::Dialect::Scss,
        "less" => css::Dialect::Less,
        _ => css::Dialect::Css,
      };
      let settings = css::Settings { dialect, ..css::Settings::default() };
      css::format(input, &settings, &mut Verbatim).map_err(|error| error.to_string())
    }
    "xml" => {
      let whitespace = match option {
        Some("ignore") => xml::Whitespace::Ignore,
        Some("preserve") => xml::Whitespace::Preserve,
        _ => xml::Whitespace::Strict,
      };
      let settings = xml::Settings { whitespace, ..xml::Settings::default() };
      xml::format(input, &settings, &mut Verbatim).map_err(|error| error.to_string())
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
        if printed != row["tidyrs"].as_str().unwrap() {
          failures.push(format!(
            "{id} [{parser}] {input:?}\n  expected {:?}\n  printed  {printed:?}",
            row["tidyrs"]
          ));
        } else if printed == row["prettier"].as_str().unwrap() {
          failures.push(format!("{id} [{parser}] {input:?}: the same as Prettier now"));
        }
      }
    }
  }
  assert!(failures.is_empty(), "{} rows:\n{}", failures.len(), failures.join("\n"));
}
