#![cfg(feature = "config")]

use tidyrs::config::{Config, Resolver};
use tidyrs::embed::Verbatim;
use tidyrs::{Language, Settings, Whitespace, doc, handlebars, xml};

fn settings(config: &Config) -> Settings {
  let mut settings = Settings::default();
  config.apply(&mut settings);
  settings
}

#[test]
fn reads_json_with_comments_and_toml_alike() {
  let json = Config::json(
    r#"{
      // shared
      "width": 100,
      "whitespace": "ignore",
      "xml": { "whitespace": "auto", "sortAttributes": true, },
    }"#,
  )
  .unwrap();
  let toml = Config::toml(
    "width = 100\nwhitespace = \"ignore\"\n[xml]\nwhitespace = \"auto\"\nsortAttributes = true\n",
  )
  .unwrap();
  assert_eq!(json, toml);
  let settings = settings(&json);
  assert_eq!(settings.html.width, 100);
  assert_eq!(settings.xml.width, 100);
  assert_eq!(settings.html.whitespace, Whitespace::Ignore);
  assert_eq!(settings.xml.whitespace, Whitespace::Auto);
  assert!(settings.xml.sort_attributes);
}

#[test]
fn sets_each_language_on_its_own() {
  let config = Config::json(
    r#"{
      "indent": 4,
      "quote": "single",
      "ignore": "tidy-ignore",
      "vue": { "indentBlocks": true, "indent": 2 },
      "handlebars": { "dialect": "tempura" },
      "css": { "trailingComma": false },
      "xml": { "selfClosingSpace": false, "attributeQuotes": "double", "eol": "crlf" }
    }"#,
  )
  .unwrap();
  let settings = settings(&config);
  assert_eq!((settings.html.tab_width, settings.vue.tab_width), (4, 2));
  assert!(settings.vue.vue_indent_script_and_style && !settings.html.vue_indent_script_and_style);
  assert_eq!(settings.handlebars.dialect, handlebars::Dialect::Tempura);
  assert!(settings.handlebars.single_quote && settings.css.single_quote);
  assert!(!settings.css.trailing_comma);
  assert!(!settings.xml.self_closing_space);
  assert_eq!(settings.xml.quotes, xml::Quotes::Double);
  assert_eq!((settings.xml.eol, settings.html.eol), (doc::Eol::Crlf, doc::Eol::Lf));
  assert_eq!(settings.css.ignore, "tidy-ignore");
}

#[test]
fn names_what_is_wrong() {
  let error = |text: &str| Config::json(text).unwrap_err();
  assert_eq!(error(r#"{ "colour": 1 }"#), "unknown option `colour`");
  assert_eq!(error(r#"{ "xml": { "dialect": "x" } }"#), "unknown option `xml.dialect`");
  assert_eq!(error(r#"{ "width": "80" }"#), "`width` must be a whole number");
  assert_eq!(
    error(r#"{ "css": { "whitespace": "css" } }"#),
    "`css.whitespace` must be \"strict\", \"auto\" or \"ignore\""
  );
  assert_eq!(error(r#"{ "ignore": "" }"#), "`ignore` must not be empty");
  assert_eq!(error("[]"), "the config must be an object");
}

#[test]
fn resolves_the_nearest_config_and_editorconfig() {
  let root = std::env::temp_dir().join(format!("tidyrs-config-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  let write = |path: &str, text: &str| {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
  };
  write(
    ".editorconfig",
    "root = true\n[*]\nindent_style = tab\nmax_line_length = 60\n[*.xml]\nindent_style = space\nindent_size = 3\n",
  );
  write("tidy.toml", "width = 100\n");
  write("a/tidy.json", r#"{ "quote": "single" }"#);
  write("b/tidy.json", "{}");
  write("b/tidy.toml", "");

  let resolver = Resolver::default();
  let top = resolver.settings(&root.join("x.html")).unwrap();
  assert_eq!((top.html.width, top.html.tabs), (100, true));
  let xml = resolver.settings(&root.join("deep/er/x.xml")).unwrap();
  assert_eq!((xml.xml.width, xml.xml.tabs, xml.xml.tab_width), (100, false, 3));
  let nested = resolver.settings(&root.join("a/x.css")).unwrap();
  assert_eq!((nested.css.width, nested.css.single_quote), (60, true));
  let error = resolver.settings(&root.join("b/x.css")).unwrap_err();
  assert!(error.contains("more than one config in this folder"), "{error}");

  let fixed = Resolver::fixed(Config::json(r#"{ "width": 20 }"#).unwrap());
  assert_eq!(fixed.settings(&root.join("a/x.css")).unwrap().css.width, 20);
  std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn formats_by_language() {
  let settings = Settings::default();
  let mut formatter = tidyrs::Formatter::default();
  let cases = [
    ("a.vue", "<template><p   >x</p></template>\n", "<template><p>x</p></template>\n"),
    ("a.xml", "<a  b='c'/>", "<a b='c' />\n"),
    ("a.less", "@a:1;", "@a: 1;\n"),
    ("a.hbs", "{{#if a}}<b/>{{/if}}", "{{#if a}}<b />{{/if}}"),
  ];
  for (path, text, expected) in cases {
    let language = Language::of_path(path).unwrap();
    let printed = formatter.format(text, language, &settings, Some(path), &mut Verbatim).unwrap();
    assert_eq!(printed, expected, "{path}");
  }
  assert_eq!(Language::of_path("a.rs"), None);
  assert_eq!(Language::of_path("icons/a.svg"), Some(Language::Xml));
}
