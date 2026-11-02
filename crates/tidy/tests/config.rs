#![cfg(feature = "config")]

use tidy::config::{Config, Resolver};
use tidy::embed::Verbatim;
use tidy::{Language, Options, Whitespace, doc, handlebars, xml};

fn options(config: &Config) -> Options {
  let mut options = Options::default();
  config.apply(&mut options);
  options
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
  let options = options(&json);
  assert_eq!(options.html.width, 100);
  assert_eq!(options.xml.width, 100);
  assert_eq!(options.html.whitespace, Whitespace::Ignore);
  assert_eq!(options.xml.whitespace, Whitespace::Auto);
  assert!(options.xml.sort_attributes);
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
  let options = options(&config);
  assert_eq!((options.html.tab_width, options.vue.tab_width), (4, 2));
  assert!(options.vue.vue_indent_script_and_style && !options.html.vue_indent_script_and_style);
  assert_eq!(options.handlebars.dialect, handlebars::Dialect::Tempura);
  assert!(options.handlebars.single_quote && options.css.single_quote);
  assert!(!options.css.trailing_comma);
  assert!(!options.xml.self_closing_space);
  assert_eq!(options.xml.quotes, xml::Quotes::Double);
  assert_eq!((options.xml.eol, options.html.eol), (doc::Eol::Crlf, doc::Eol::Lf));
  assert_eq!(options.css.ignore, "tidy-ignore");
}

#[test]
fn sets_dockerfile() {
  let config = Config::json(
    r#"{ "width": 100, "dockerfile": { "indentStages": true, "healthcheckCmdNewLine": true, "indent": 4 } }"#,
  )
  .unwrap();
  let options = options(&config).dockerfile;
  assert_eq!((options.width, options.indent_width), (100, 4));
  assert!(options.indent_stages && options.healthcheck_cmd_new_line);
}

#[test]
fn sets_graphql() {
  let config = Config::json(r#"{ "width": 100, "graphql": { "bracketSpacing": false } }"#).unwrap();
  let options = options(&config).graphql;
  assert_eq!((options.width, options.bracket_spacing), (100, false));
}

#[cfg(feature = "js")]
#[test]
fn sets_js() {
  use tidy::js::{ArrowParens, OperatorPosition, TrailingComma};
  let config = Config::json(
    r#"{
      "quote": "single",
      "trailingComma": false,
      "js": {
        "width": 100,
        "semi": false,
        "jsxSingleQuote": true,
        "quoteProps": "consistent",
        "arrowParens": "avoid",
        "objectWrap": "collapse",
        "experimentalTernaries": true,
        "experimentalOperatorPosition": "start"
      }
    }"#,
  )
  .unwrap();
  let js = options(&config).js;
  assert_eq!((js.width, js.semi), (100, false));
  assert!(js.single_quote && js.jsx_single_quote && js.experimental_ternaries);
  assert_eq!(js.quote_props, tidy::json::QuoteProps::Consistent);
  assert_eq!(js.trailing_comma, TrailingComma::None);
  assert_eq!(js.arrow_parens, ArrowParens::Avoid);
  assert_eq!(js.object_wrap, tidy::json::ObjectWrap::Collapse);
  assert_eq!(js.experimental_operator_position, OperatorPosition::Start);
  let config = Config::json(r#"{ "js": { "trailingComma": "es5" } }"#).unwrap();
  assert_eq!(options(&config).js.trailing_comma, TrailingComma::Es5);
  assert!(Config::json(r#"{ "js": { "trailingComma": "some" } }"#).is_err());
}

#[test]
fn sets_ini() {
  let config =
    Config::json(r#"{ "indent": 4, "ini": { "spaceAroundEquals": false, "tabs": true } }"#)
      .unwrap();
  let options = options(&config).ini;
  assert_eq!((options.spaces, options.tab_width, options.tabs), (false, 4, true));
}

#[test]
fn sets_json_yaml_and_toml() {
  let config = Config::toml(
    r#"
      width = 100
      trailingComma = false

      [json]
      quoteProps = "consistent"
      objectWrap = "collapse"
      bracketSpacing = false

      [yaml]
      wrap = "never"
      bracketSpacing = false

      [toml]
      alignEntries = true
      reorderKeys = true
      allowedBlankLines = 1
      indent = 4
    "#,
  )
  .unwrap();
  let options = options(&config);
  assert_eq!((options.json.width, options.toml.column_width), (100, 100));
  assert!(!options.json.trailing_comma && !options.toml.array_trailing_comma);
  assert_eq!(options.json.quote_props, tidy::json::QuoteProps::Consistent);
  assert_eq!(options.json.object_wrap, tidy::json::ObjectWrap::Collapse);
  assert!(!options.json.bracket_spacing);
  assert!(options.toml.align_entries && options.toml.reorder_keys);
  assert_eq!((options.toml.allowed_blank_lines, options.toml.tab_width), (1, 4));
  assert_eq!(options.json.tab_width, 2);
  assert_eq!(options.yaml.wrap, tidy::Wrap::Never);
  assert!(!options.yaml.bracket_spacing && !options.yaml.trailing_comma);
  assert_eq!(options.yaml.width, 100);
}

#[test]
fn names_what_is_wrong() {
  let error = |text: &str| Config::json(text).unwrap_err();
  assert_eq!(error(r#"{ "colour": 1 }"#), "unknown option `colour`");
  assert_eq!(error(r#"{ "xml": { "dialect": "x" } }"#), "unknown option `xml.dialect`");
  assert_eq!(
    error(r#"{ "dockerfile": { "lineWidth": 9 } }"#),
    "unknown option `dockerfile.lineWidth`"
  );
  assert_eq!(error(r#"{ "width": "80" }"#), "`width` must be a whole number");
  assert_eq!(
    error(r#"{ "css": { "whitespace": "css" } }"#),
    "`css.whitespace` must be \"strict\", \"auto\" or \"ignore\""
  );
  assert_eq!(error(r#"{ "ignore": "" }"#), "`ignore` must not be empty");
  assert_eq!(error("[]"), "the config must be an object");
  assert_eq!(error("{\n  \"width\": }"), "2:12: Unexpected token");
  assert_eq!(
    Config::toml("width = 80\nwidth = 90").unwrap_err(),
    "2:1: `width` is already defined"
  );
}

#[test]
fn resolves_the_nearest_config_and_editorconfig() {
  let root = std::env::temp_dir().join(format!("tidy-config-{}", std::process::id()));
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
  let top = resolver.options(&root.join("x.html")).unwrap();
  assert_eq!((top.html.width, top.html.tabs), (100, true));
  let xml = resolver.options(&root.join("deep/er/x.xml")).unwrap();
  assert_eq!((xml.xml.width, xml.xml.tabs, xml.xml.tab_width), (100, false, 3));
  let nested = resolver.options(&root.join("a/x.css")).unwrap();
  assert_eq!((nested.css.width, nested.css.single_quote), (60, true));
  let error = resolver.options(&root.join("b/x.css")).unwrap_err();
  assert!(error.contains("more than one config in this folder"), "{error}");

  let fixed = Resolver::fixed(Config::json(r#"{ "width": 20 }"#).unwrap());
  assert_eq!(fixed.options(&root.join("a/x.css")).unwrap().css.width, 20);
  std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn resolves_parent_folders_before_looking_up() {
  let root = std::env::temp_dir().join(format!("tidy-parent-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("proj")).unwrap();
  std::fs::write(root.join("proj/tidy.json"), r#"{ "width": 20 }"#).unwrap();
  let options = Resolver::default().options(&root.join("proj/../docs/a.html")).unwrap();
  assert_eq!(options.html.width, 80);
  std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn formats_by_language() {
  let options = Options::default();
  let mut formatter = tidy::Formatter::default();
  let cases = [
    ("a.vue", "<template><p   >x</p></template>\n", "<template><p>x</p></template>\n"),
    ("a.xml", "<a  b='c'/>", "<a b='c' />\n"),
    ("a.less", "@a:1;", "@a: 1;\n"),
    ("a.hbs", "{{#if a}}<b/>{{/if}}", "{{#if a}}<b />{{/if}}"),
  ];
  for (path, text, expected) in cases {
    let language = Language::of_path(path).unwrap();
    let printed = formatter.format(text, language, &options, Some(path), &mut Verbatim).unwrap();
    assert_eq!(printed, expected, "{path}");
  }
  assert_eq!(Language::of_path("a.rs"), None);
  assert_eq!(Language::of_path("icons/a.svg"), Some(Language::Xml));
}

#[test]
fn sets_markdown_from_a_preset_and_its_own_options() {
  let config = Config::toml(
    "width = 100\nignore = \"skip\"\n[markdown]\npreset = \"preserve\"\nwrap = \"always\"\nbullet = \"plus\"\n",
  )
  .unwrap();
  let markdown = options(&config).markdown;
  assert_eq!(markdown.width, 100);
  assert_eq!(markdown.ignore, "skip");
  assert_eq!(markdown.wrap, tidy::Wrap::Always);
  assert_eq!(markdown.bullet, tidy::markdown::Bullet::Plus);
  assert_eq!(markdown.emphasis, tidy::markdown::Emphasis::Keep);
  let preserve =
    options(&Config::json(r#"{ "markdown": { "preset": "preserve" } }"#).unwrap()).markdown;
  assert_eq!(preserve.ignore, "tidy-ignore");
  assert_eq!(
    Config::json(r#"{ "markdown": { "wrap": "sometimes" } }"#).unwrap_err(),
    "`markdown.wrap`: unknown wrap `sometimes`, expected one of: preserve, always, never"
  );
}

#[test]
fn turns_embedded_formatting_off() {
  let options = options(&Config::json(r#"{ "embedded": false }"#).unwrap());
  assert!(!options.html.format_embedded);
  assert!(!options.markdown.format_embedded);
  #[cfg(feature = "js")]
  assert!(!options.js.format_embedded);
}
