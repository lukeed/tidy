//! Edge cases of XML, each with what `@prettier/plugin-xml` prints for it, recorded by `tests/oracle/xml-cases.ts`.

use serde_json::Value;
use tidyrs::Whitespace;
use tidyrs::embed::Verbatim;
use tidyrs::xml::{Quotes, Settings, format, is_xml_path};

fn settings(options: &Value) -> Settings {
  let flag = |name: &str| options.get(name).and_then(Value::as_bool);
  let text = |name: &str| options.get(name).and_then(Value::as_str);
  let defaults = Settings::default();
  Settings {
    width: options
      .get("printWidth")
      .and_then(Value::as_u64)
      .map_or(defaults.width, |width| width as usize),
    tabs: flag("useTabs").unwrap_or_default(),
    whitespace: match text("xmlWhitespaceSensitivity") {
      Some("ignore") => Whitespace::Ignore,
      Some("preserve") => Whitespace::Auto,
      _ => Whitespace::Strict,
    },
    self_closing_space: flag("xmlSelfClosingSpace").unwrap_or(true),
    sort_attributes: flag("xmlSortAttributesByKey").unwrap_or_default(),
    quotes: match text("xmlQuoteAttributes") {
      Some("single") => Quotes::Single,
      Some("double") => Quotes::Double,
      _ => Quotes::Preserve,
    },
    bracket_same_line: flag("bracketSameLine").unwrap_or_default(),
    single_attribute_per_line: flag("singleAttributePerLine").unwrap_or_default(),
    format_embedded: text("embeddedLanguageFormatting") != Some("off"),
    ..defaults
  }
}

#[test]
fn prints_what_the_plugin_prints() {
  let cases: Vec<Value> = serde_json::from_str(include_str!("xml.json")).unwrap();
  let mut failures = Vec::new();
  for case in &cases {
    let input = case["input"].as_str().unwrap();
    let options = case.get("options").cloned().unwrap_or_default();
    let printed =
      format(input, &settings(&options), &mut Verbatim).unwrap_or_else(|_| "!error".into());
    let expected = case["output"].as_str().unwrap();
    if printed != expected {
      failures
        .push(format!("{input:?} {options}\n  expected {expected:?}\n  printed  {printed:?}"));
    }
  }
  assert!(failures.is_empty(), "{} cases:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn knows_xml_paths() {
  for path in
    ["a.xml", "icons/b.svg", "Info.plist", "app.csproj", "x/pom.xml", "Web.config", "a.xml.dist"]
  {
    assert!(is_xml_path(path), "{path}");
  }
  for path in ["main.rs", "a.html", "a.mjml", "xml", ".xml", "pom.xml.bak"] {
    assert!(!is_xml_path(path), "{path}");
  }
}

#[test]
fn takes_another_ignore_directive() {
  let settings = Settings { ignore: "tidy-ignore".into(), ..Settings::default() };
  let text = "<a>\n  <!-- tidy-ignore-start -->\n  <b   c=\"d\"   />\n  <!-- tidy-ignore-end -->\n  <!-- prettier-ignore-start -->\n  <e   f=\"g\"/>\n  <!-- prettier-ignore-end -->\n</a>\n";
  assert_eq!(
    format(text, &settings, &mut Verbatim).unwrap(),
    "<a>\n  <!-- tidy-ignore-start -->\n  <b   c=\"d\"   />\n  <!-- tidy-ignore-end -->\n  <!-- prettier-ignore-start -->\n  <e f=\"g\" />\n  <!-- prettier-ignore-end -->\n</a>\n"
  );
}
