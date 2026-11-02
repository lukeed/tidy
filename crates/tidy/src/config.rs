//! Options from files: the nearest `tidy.json`, `tidy.jsonc` or `tidy.toml`, and `.editorconfig`.
//!
//! A config sets shared choices at the top, and each language's own in its section, where the shared ones can be set
//! again for that language only:
//!
//! ```toml
//! width = 100
//! ignore = "tidy-ignore"
//!
//! [xml]
//! whitespace = "ignore"
//! sortAttributes = true
//! ```
//!
//! `.editorconfig` sets `indent_style`, `indent_size`, `tab_width`, `max_line_length` and `end_of_line`, as Prettier
//! reads them. A tidy config overrides it.

use crate::options::{Options, Shared};
use crate::{Whitespace, doc, handlebars, json, markdown, toml, xml};
use rustc_hash::FxHashMap;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// The names of a config file, in the order they are looked for.
pub const NAMES: [&str; 3] = ["tidy.json", "tidy.jsonc", "tidy.toml"];

/// The options of a config file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
  pub shared: Shared,
  pub html: Shared,
  pub vue: Vue,
  pub handlebars: Handlebars,
  pub css: Shared,
  pub json: Json,
  pub toml: Toml,
  pub xml: Xml,
  pub yaml: Yaml,
  pub markdown: Markdown,
  pub dockerfile: Dockerfile,
  pub ini: Ini,
  pub graphql: Shared,
  #[cfg(feature = "js")]
  pub js: Js,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vue {
  pub shared: Shared,
  /// `indentBlocks`: Prettier's `vueIndentScriptAndStyle`.
  pub indent_blocks: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Handlebars {
  pub shared: Shared,
  pub dialect: Option<handlebars::Dialect>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Json {
  pub shared: Shared,
  /// `quoteProps`, which only JSON5 follows.
  pub quote_props: Option<json::QuoteProps>,
  /// `objectWrap`
  pub object_wrap: Option<json::ObjectWrap>,
}

/// taplo's options, by their names in camel case. The shared `width` is its `column_width`, and `trailingComma` its
/// `array_trailing_comma`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Toml {
  pub shared: Shared,
  pub align_entries: Option<bool>,
  pub align_comments: Option<bool>,
  pub align_single_comments: Option<bool>,
  pub array_auto_expand: Option<bool>,
  pub array_auto_collapse: Option<bool>,
  pub inline_table_expand: Option<bool>,
  pub compact_arrays: Option<bool>,
  pub compact_inline_tables: Option<bool>,
  pub compact_entries: Option<bool>,
  pub indent_tables: Option<bool>,
  pub indent_entries: Option<bool>,
  pub trailing_newline: Option<bool>,
  pub reorder_keys: Option<bool>,
  pub reorder_arrays: Option<bool>,
  pub reorder_inline_tables: Option<bool>,
  pub allowed_blank_lines: Option<usize>,
}

/// dprint-plugin-dockerfile's options, by their names. The shared `width` is its `lineWidth`, and `indent` its
/// `indentWidth`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dockerfile {
  pub shared: Shared,
  pub indent_stages: Option<bool>,
  pub healthcheck_cmd_new_line: Option<bool>,
}

/// prettier-plugin-ini's options. The shared `indent` is the indent of the values of a key over lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ini {
  pub shared: Shared,
  /// `spaceAroundEquals`: the plugin's `iniSpaceAroundEquals`.
  pub space_around_equals: Option<bool>,
}

/// Prettier's options for JavaScript and TypeScript, in files and embedded in other languages. `trailingComma` is
/// `"all"`, `"es5"` or `"none"` here, or the shared true or false.
#[cfg(feature = "js")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Js {
  pub shared: Shared,
  pub semi: Option<bool>,
  /// `jsxSingleQuote`
  pub jsx_single_quote: Option<bool>,
  /// `quoteProps`
  pub quote_props: Option<json::QuoteProps>,
  /// `trailingComma`, as a string.
  pub trailing_comma: Option<crate::js::TrailingComma>,
  /// `arrowParens`
  pub arrow_parens: Option<crate::js::ArrowParens>,
  /// `objectWrap`
  pub object_wrap: Option<json::ObjectWrap>,
  /// `experimentalTernaries`
  pub experimental_ternaries: Option<bool>,
  /// `experimentalOperatorPosition`
  pub experimental_operator_position: Option<crate::js::OperatorPosition>,
  /// Format JSDoc comments as prettier-plugin-jsdoc does.
  pub jsdoc: Option<bool>,
  /// How to sort imports: from `sortImports`, or from `plugins` and the options of the plugins it lists. See
  /// [`crate::js::SortSettings`].
  pub sort_imports: Option<Option<Arc<crate::js::SortImports>>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Yaml {
  pub shared: Shared,
  /// `wrap`: Prettier's `proseWrap`, for flow scalars and folded block scalars.
  pub wrap: Option<crate::Wrap>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Markdown {
  pub shared: Shared,
  /// Sets every style option first; the others override it.
  pub preset: Option<markdown::Preset>,
  pub wrap: Option<crate::Wrap>,
  pub bullet: Option<markdown::Bullet>,
  pub emphasis: Option<markdown::Emphasis>,
  pub strong: Option<markdown::Strong>,
  pub numbering: Option<markdown::Numbering>,
  pub rule: Option<markdown::Rule>,
  pub fence: Option<markdown::Fence>,
  pub heading: Option<markdown::Heading>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Xml {
  pub shared: Shared,
  /// `selfClosingSpace`
  pub self_closing_space: Option<bool>,
  /// `sortAttributes`
  pub sort_attributes: Option<bool>,
  /// `attributeQuotes`
  pub quotes: Option<xml::Quotes>,
}

impl Config {
  /// Reads the config file at `path`, TOML if its name ends with `.toml`, and JSON otherwise.
  pub fn read(path: &Path) -> Result<Config, String> {
    let text =
      std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let toml = path.extension().is_some_and(|extension| extension == "toml");
    let value = value(&text, toml).map_err(|error| {
      let separator = if matches!(error, crate::Error::Syntax { .. }) { ":" } else { ": " };
      format!("{}{separator}{}", path.display(), describe(&text, error))
    })?;
    Config::from_value(&value).map_err(|error| format!("{}: {error}", path.display()))
  }

  /// Reads a config from the text of a JSON file, which may have comments and trailing commas.
  pub fn json(text: &str) -> Result<Config, String> {
    Config::from_value(&value(text, false).map_err(|error| describe(text, error))?)
  }

  /// Reads a config from the text of a TOML file.
  pub fn toml(text: &str) -> Result<Config, String> {
    Config::from_value(&value(text, true).map_err(|error| describe(text, error))?)
  }

  /// Reads a config from a JSON value, as the Node binding receives its options.
  pub fn from_value(value: &Value) -> Result<Config, String> {
    let Some(map) = value.as_object() else { return Err("the config must be an object".into()) };
    let mut config = Config::default();
    for (key, value) in map {
      let section =
        |name: &str| value.as_object().ok_or_else(|| format!("`{name}` must be an object"));
      match key.as_str() {
        "html" => config.html = shared_only(section("html")?, "html")?,
        "vue" => config.vue = vue(section("vue")?)?,
        "handlebars" => config.handlebars = handlebars(section("handlebars")?)?,
        "css" => config.css = shared_only(section("css")?, "css")?,
        "json" => config.json = json(section("json")?)?,
        "toml" => config.toml = toml(section("toml")?)?,
        "dockerfile" => config.dockerfile = dockerfile(section("dockerfile")?)?,
        "ini" => config.ini = ini(section("ini")?)?,
        "graphql" => config.graphql = shared_only(section("graphql")?, "graphql")?,
        #[cfg(feature = "js")]
        "js" => config.js = js(section("js")?)?,
        "yaml" => config.yaml = yaml(section("yaml")?)?,
        "xml" => config.xml = xml(section("xml")?)?,
        "markdown" => config.markdown = markdown(section("markdown")?)?,
        _ if shared(&mut config.shared, key, value, "")? => {}
        _ => return Err(format!("unknown option `{key}`")),
      }
    }
    Ok(config)
  }

  /// Applies the config to `options`: the shared choices to every language, and then each language's own.
  pub fn apply(&self, options: &mut Options) {
    if let Some(preset) = self.markdown.preset {
      let width = options.markdown.width;
      let format_embedded = options.markdown.format_embedded;
      options.markdown =
        markdown::Options { width, format_embedded, ..markdown::Options::from(preset) };
    }
    options.share(&self.shared);
    self.html.html(&mut options.html);
    self.vue.shared.html(&mut options.vue);
    set(&mut options.vue.vue_indent_script_and_style, self.vue.indent_blocks);
    self.handlebars.shared.handlebars(&mut options.handlebars);
    set(&mut options.handlebars.dialect, self.handlebars.dialect);
    self.css.css(&mut options.css);
    self.json.shared.json(&mut options.json);
    set(&mut options.json.quote_props, self.json.quote_props);
    set(&mut options.json.object_wrap, self.json.object_wrap);
    self.toml.apply(&mut options.toml);
    self.dockerfile.shared.dockerfile(&mut options.dockerfile);
    set(&mut options.dockerfile.indent_stages, self.dockerfile.indent_stages);
    set(&mut options.dockerfile.healthcheck_cmd_new_line, self.dockerfile.healthcheck_cmd_new_line);
    self.ini.shared.ini(&mut options.ini);
    set(&mut options.ini.spaces, self.ini.space_around_equals);
    self.graphql.graphql(&mut options.graphql);
    #[cfg(feature = "js")]
    self.js.apply(&mut options.js);
    self.yaml.shared.yaml(&mut options.yaml);
    set(&mut options.yaml.wrap, self.yaml.wrap);
    self.xml.shared.xml(&mut options.xml);
    set(&mut options.xml.self_closing_space, self.xml.self_closing_space);
    set(&mut options.xml.sort_attributes, self.xml.sort_attributes);
    set(&mut options.xml.quotes, self.xml.quotes);
    let markdown = &self.markdown;
    markdown.shared.markdown(&mut options.markdown);
    let style = &mut options.markdown;
    set(&mut style.wrap, markdown.wrap);
    set(&mut style.bullet, markdown.bullet);
    set(&mut style.emphasis, markdown.emphasis);
    set(&mut style.strong, markdown.strong);
    set(&mut style.numbering, markdown.numbering);
    set(&mut style.rule, markdown.rule);
    set(&mut style.fence, markdown.fence);
    set(&mut style.heading, markdown.heading);
  }
}

/// Reads a shared option into `shared`. Returns false if `key` isn't one.
fn shared(shared: &mut Shared, key: &str, value: &Value, section: &str) -> Result<bool, String> {
  let name = match section {
    "" => key.to_owned(),
    section => format!("{section}.{key}"),
  };
  match key {
    "width" => shared.width = Some(number(value, &name)?),
    "indent" => shared.indent = Some(number(value, &name)?),
    "tabs" => shared.tabs = Some(flag(value, &name)?),
    "eol" => {
      shared.eol = Some(match text(value, &name)? {
        "lf" => doc::Eol::Lf,
        "crlf" => doc::Eol::Crlf,
        "cr" => doc::Eol::Cr,
        _ => return Err(format!("`{name}` must be \"lf\", \"crlf\" or \"cr\"")),
      })
    }
    "whitespace" => {
      shared.whitespace = Some(match text(value, &name)? {
        "strict" => Whitespace::Strict,
        "auto" => Whitespace::Auto,
        "ignore" => Whitespace::Ignore,
        _ => return Err(format!("`{name}` must be \"strict\", \"auto\" or \"ignore\"")),
      })
    }
    "quote" => {
      shared.single_quote = Some(match text(value, &name)? {
        "double" => false,
        "single" => true,
        _ => return Err(format!("`{name}` must be \"double\" or \"single\"")),
      })
    }
    "trailingComma" => shared.trailing_comma = Some(flag(value, &name)?),
    "bracketSpacing" => shared.bracket_spacing = Some(flag(value, &name)?),
    "bracketSameLine" => shared.bracket_same_line = Some(flag(value, &name)?),
    "singleAttributePerLine" => shared.single_attribute_per_line = Some(flag(value, &name)?),
    "embedded" => shared.format_embedded = Some(flag(value, &name)?),
    "ignore" => match text(value, &name)? {
      "" => return Err(format!("`{name}` must not be empty")),
      ignore => shared.ignore = Some(ignore.to_owned()),
    },
    _ => return Ok(false),
  }
  Ok(true)
}

fn shared_only(map: &Map<String, Value>, section: &str) -> Result<Shared, String> {
  let mut options = Shared::default();
  for (key, value) in map {
    if !shared(&mut options, key, value, section)? {
      return Err(format!("unknown option `{section}.{key}`"));
    }
  }
  Ok(options)
}

fn vue(map: &Map<String, Value>) -> Result<Vue, String> {
  let mut vue = Vue::default();
  for (key, value) in map {
    match key.as_str() {
      "indentBlocks" => vue.indent_blocks = Some(flag(value, "vue.indentBlocks")?),
      _ if shared(&mut vue.shared, key, value, "vue")? => {}
      _ => return Err(format!("unknown option `vue.{key}`")),
    }
  }
  Ok(vue)
}

fn handlebars(map: &Map<String, Value>) -> Result<Handlebars, String> {
  let mut options = Handlebars::default();
  for (key, value) in map {
    match key.as_str() {
      "dialect" => {
        options.dialect = Some(match text(value, "handlebars.dialect")? {
          "auto" => handlebars::Dialect::Auto,
          "handlebars" => handlebars::Dialect::Handlebars,
          "tempura" => handlebars::Dialect::Tempura,
          _ => {
            return Err(
              "`handlebars.dialect` must be \"auto\", \"handlebars\" or \"tempura\"".into(),
            );
          }
        })
      }
      _ if shared(&mut options.shared, key, value, "handlebars")? => {}
      _ => return Err(format!("unknown option `handlebars.{key}`")),
    }
  }
  Ok(options)
}

fn json(map: &Map<String, Value>) -> Result<Json, String> {
  let mut options = Json::default();
  for (key, value) in map {
    match key.as_str() {
      "quoteProps" => options.quote_props = Some(quote_props(value, "json.quoteProps")?),
      "objectWrap" => options.object_wrap = Some(object_wrap(value, "json.objectWrap")?),
      _ if shared(&mut options.shared, key, value, "json")? => {}
      _ => return Err(format!("unknown option `json.{key}`")),
    }
  }
  Ok(options)
}

fn quote_props(value: &Value, name: &str) -> Result<json::QuoteProps, String> {
  Ok(match text(value, name)? {
    "as-needed" => json::QuoteProps::AsNeeded,
    "consistent" => json::QuoteProps::Consistent,
    "preserve" => json::QuoteProps::Preserve,
    _ => return Err(format!("`{name}` must be \"as-needed\", \"consistent\" or \"preserve\"")),
  })
}

fn object_wrap(value: &Value, name: &str) -> Result<json::ObjectWrap, String> {
  Ok(match text(value, name)? {
    "preserve" => json::ObjectWrap::Preserve,
    "collapse" => json::ObjectWrap::Collapse,
    _ => return Err(format!("`{name}` must be \"preserve\" or \"collapse\"")),
  })
}

#[cfg(feature = "js")]
fn js(map: &Map<String, Value>) -> Result<Js, String> {
  use crate::js::{ArrowParens, OperatorPosition, SortSettings, TrailingComma};
  let mut options = Js::default();
  let (mut sort, mut sorts) = (SortSettings::default(), false);
  for (key, value) in map {
    let name = format!("js.{key}");
    match key.as_str() {
      _ if !key.starts_with("tsconfig.") && sort.set(key, value.clone()) => sorts = true,
      "semi" => options.semi = Some(flag(value, &name)?),
      "jsxSingleQuote" => options.jsx_single_quote = Some(flag(value, &name)?),
      "quoteProps" => options.quote_props = Some(quote_props(value, &name)?),
      "trailingComma" if value.is_string() => {
        options.trailing_comma = Some(match text(value, &name)? {
          "all" => TrailingComma::All,
          "es5" => TrailingComma::Es5,
          "none" => TrailingComma::None,
          _ => return Err(format!("`{name}` must be \"all\", \"es5\", \"none\", true or false")),
        })
      }
      "arrowParens" => {
        options.arrow_parens = Some(match text(value, &name)? {
          "always" => ArrowParens::Always,
          "avoid" => ArrowParens::Avoid,
          _ => return Err(format!("`{name}` must be \"always\" or \"avoid\"")),
        })
      }
      "objectWrap" => options.object_wrap = Some(object_wrap(value, &name)?),
      "experimentalTernaries" => options.experimental_ternaries = Some(flag(value, &name)?),
      "jsdoc" => options.jsdoc = Some(flag(value, &name)?),
      "experimentalOperatorPosition" => {
        options.experimental_operator_position = Some(match text(value, &name)? {
          "start" => OperatorPosition::Start,
          "end" => OperatorPosition::End,
          _ => return Err(format!("`{name}` must be \"start\" or \"end\"")),
        })
      }
      _ if shared(&mut options.shared, key, value, "js")? => {}
      _ => return Err(format!("unknown option `{name}`")),
    }
  }
  if sorts {
    options.sort_imports = Some(sort.compile().map_err(|error| format!("`js`: {error}"))?);
  }
  Ok(options)
}

#[cfg(feature = "js")]
impl Js {
  fn apply(&self, options: &mut crate::js::Options) {
    self.shared.js(options);
    set(&mut options.semi, self.semi);
    set(&mut options.jsx_single_quote, self.jsx_single_quote);
    set(&mut options.quote_props, self.quote_props);
    set(&mut options.trailing_comma, self.trailing_comma);
    set(&mut options.arrow_parens, self.arrow_parens);
    set(&mut options.object_wrap, self.object_wrap);
    set(&mut options.experimental_ternaries, self.experimental_ternaries);
    set(&mut options.experimental_operator_position, self.experimental_operator_position);
    set(&mut options.jsdoc, self.jsdoc);
    set(&mut options.sort_imports, self.sort_imports.clone());
  }
}

impl Toml {
  fn apply(&self, options: &mut toml::Options) {
    self.shared.toml(options);
    set(&mut options.align_entries, self.align_entries);
    set(&mut options.align_comments, self.align_comments);
    set(&mut options.align_single_comments, self.align_single_comments);
    set(&mut options.array_auto_expand, self.array_auto_expand);
    set(&mut options.array_auto_collapse, self.array_auto_collapse);
    set(&mut options.inline_table_expand, self.inline_table_expand);
    set(&mut options.compact_arrays, self.compact_arrays);
    set(&mut options.compact_inline_tables, self.compact_inline_tables);
    set(&mut options.compact_entries, self.compact_entries);
    set(&mut options.indent_tables, self.indent_tables);
    set(&mut options.indent_entries, self.indent_entries);
    set(&mut options.trailing_newline, self.trailing_newline);
    set(&mut options.reorder_keys, self.reorder_keys);
    set(&mut options.reorder_arrays, self.reorder_arrays);
    set(&mut options.reorder_inline_tables, self.reorder_inline_tables);
    set(&mut options.allowed_blank_lines, self.allowed_blank_lines);
  }
}

fn dockerfile(map: &Map<String, Value>) -> Result<Dockerfile, String> {
  let mut options = Dockerfile::default();
  for (key, value) in map {
    let name = format!("dockerfile.{key}");
    let field = match key.as_str() {
      "indentStages" => &mut options.indent_stages,
      "healthcheckCmdNewLine" => &mut options.healthcheck_cmd_new_line,
      _ if shared(&mut options.shared, key, value, "dockerfile")? => continue,
      _ => return Err(format!("unknown option `{name}`")),
    };
    *field = Some(flag(value, &name)?);
  }
  Ok(options)
}

fn ini(map: &Map<String, Value>) -> Result<Ini, String> {
  let mut options = Ini::default();
  for (key, value) in map {
    let name = format!("ini.{key}");
    match key.as_str() {
      "spaceAroundEquals" => options.space_around_equals = Some(flag(value, &name)?),
      _ if shared(&mut options.shared, key, value, "ini")? => {}
      _ => return Err(format!("unknown option `{name}`")),
    }
  }
  Ok(options)
}

fn toml(map: &Map<String, Value>) -> Result<Toml, String> {
  let mut options = Toml::default();
  for (key, value) in map {
    let name = format!("toml.{key}");
    let field = match key.as_str() {
      "alignEntries" => &mut options.align_entries,
      "alignComments" => &mut options.align_comments,
      "alignSingleComments" => &mut options.align_single_comments,
      "arrayAutoExpand" => &mut options.array_auto_expand,
      "arrayAutoCollapse" => &mut options.array_auto_collapse,
      "inlineTableExpand" => &mut options.inline_table_expand,
      "compactArrays" => &mut options.compact_arrays,
      "compactInlineTables" => &mut options.compact_inline_tables,
      "compactEntries" => &mut options.compact_entries,
      "indentTables" => &mut options.indent_tables,
      "indentEntries" => &mut options.indent_entries,
      "trailingNewline" => &mut options.trailing_newline,
      "reorderKeys" => &mut options.reorder_keys,
      "reorderArrays" => &mut options.reorder_arrays,
      "reorderInlineTables" => &mut options.reorder_inline_tables,
      "allowedBlankLines" => {
        options.allowed_blank_lines = Some(number(value, &name)?);
        continue;
      }
      _ if shared(&mut options.shared, key, value, "toml")? => continue,
      _ => return Err(format!("unknown option `{name}`")),
    };
    *field = Some(flag(value, &name)?);
  }
  Ok(options)
}

fn yaml(map: &Map<String, Value>) -> Result<Yaml, String> {
  let mut options = Yaml::default();
  for (key, value) in map {
    match key.as_str() {
      "wrap" => {
        let wrap = text(value, "yaml.wrap")?
          .parse()
          .map_err(|error: String| format!("`yaml.wrap`: {error}"))?;
        options.wrap = Some(wrap);
      }
      _ if shared(&mut options.shared, key, value, "yaml")? => {}
      _ => return Err(format!("unknown option `yaml.{key}`")),
    }
  }
  Ok(options)
}

fn markdown(map: &Map<String, Value>) -> Result<Markdown, String> {
  fn choice<T: std::str::FromStr<Err = String>>(
    value: &Value,
    key: &str,
  ) -> Result<Option<T>, String> {
    let name = format!("markdown.{key}");
    text(value, &name)?.parse().map(Some).map_err(|error: String| format!("`{name}`: {error}"))
  }
  let mut options = Markdown::default();
  for (key, value) in map {
    match key.as_str() {
      "preset" => options.preset = choice(value, key)?,
      "wrap" => options.wrap = choice(value, key)?,
      "bullet" => options.bullet = choice(value, key)?,
      "emphasis" => options.emphasis = choice(value, key)?,
      "strong" => options.strong = choice(value, key)?,
      "numbering" => options.numbering = choice(value, key)?,
      "rule" => options.rule = choice(value, key)?,
      "fence" => options.fence = choice(value, key)?,
      "heading" => options.heading = choice(value, key)?,
      _ if shared(&mut options.shared, key, value, "markdown")? => {}
      _ => return Err(format!("unknown option `markdown.{key}`")),
    }
  }
  Ok(options)
}

fn xml(map: &Map<String, Value>) -> Result<Xml, String> {
  let mut options = Xml::default();
  for (key, value) in map {
    match key.as_str() {
      "selfClosingSpace" => options.self_closing_space = Some(flag(value, "xml.selfClosingSpace")?),
      "sortAttributes" => options.sort_attributes = Some(flag(value, "xml.sortAttributes")?),
      "attributeQuotes" => {
        options.quotes = Some(match text(value, "xml.attributeQuotes")? {
          "preserve" => xml::Quotes::Preserve,
          "single" => xml::Quotes::Single,
          "double" => xml::Quotes::Double,
          _ => {
            return Err(
              "`xml.attributeQuotes` must be \"preserve\", \"single\" or \"double\"".into(),
            );
          }
        })
      }
      _ if shared(&mut options.shared, key, value, "xml")? => {}
      _ => return Err(format!("unknown option `xml.{key}`")),
    }
  }
  Ok(options)
}

fn set<T>(field: &mut T, value: Option<T>) {
  if let Some(value) = value {
    *field = value;
  }
}

fn value(text: &str, toml: bool) -> Result<Value, crate::Error> {
  match toml {
    true => toml::value(text),
    false => json::value(text),
  }
}

/// An error of a config file's text, with `line:column: ` before a syntax error.
fn describe(text: &str, error: crate::Error) -> String {
  match error {
    crate::Error::Syntax { message, at } => {
      let before = crate::without_bom(&text[..at.min(text.len())]).1;
      let line = before.matches('\n').count() + 1;
      let column = before.rsplit('\n').next().unwrap_or_default().chars().count() + 1;
      format!("{line}:{column}: {message}")
    }
    crate::Error::TooDeep => error.to_string(),
  }
}

fn number(value: &Value, name: &str) -> Result<usize, String> {
  value
    .as_u64()
    .map(|number| number as usize)
    .ok_or_else(|| format!("`{name}` must be a whole number"))
}

fn flag(value: &Value, name: &str) -> Result<bool, String> {
  value.as_bool().ok_or_else(|| format!("`{name}` must be true or false"))
}

fn text<'v>(value: &'v Value, name: &str) -> Result<&'v str, String> {
  value.as_str().ok_or_else(|| format!("`{name}` must be a string"))
}

// ───────────────────────────── files ─────────────────────────────

/// Finds the options of each file from the config files around it. It looks at each folder once, so one resolver
/// can serve a whole walk, from several threads.
pub struct Resolver {
  /// The config of every file, instead of the nearest one.
  fixed: Option<Arc<Config>>,
  /// The folder that relative paths are in.
  cwd: PathBuf,
  folders: Mutex<FxHashMap<OsString, Arc<Folder>>>,
  /// The config files that a walk found in each folder, so that the resolver doesn't look for them again.
  listings: Mutex<FxHashMap<OsString, Listing>>,
}

/// What a resolver knows about a folder.
#[derive(Default)]
struct Folder {
  /// The `.editorconfig` files that apply to the files in it, and the folder of each, nearest last.
  editors: Vec<(PathBuf, Arc<Editor>)>,
  /// Its nearest config.
  config: Option<Arc<Config>>,
}

/// The config files in a folder.
#[derive(Clone, Copy, Default)]
struct Listing {
  editorconfig: bool,
  /// A bit for each of [`NAMES`].
  configs: u8,
}

/// An `.editorconfig` file.
struct Editor {
  is_root: bool,
  sections: Vec<ec4rs::Section>,
}

impl Default for Resolver {
  fn default() -> Self {
    Resolver {
      fixed: None,
      cwd: std::env::current_dir().unwrap_or_default(),
      folders: Mutex::default(),
      listings: Mutex::default(),
    }
  }
}

impl Resolver {
  /// A resolver that gives every file the options of `config`, and of its `.editorconfig` files.
  pub fn fixed(config: Config) -> Resolver {
    Resolver { fixed: Some(Arc::new(config)), ..Resolver::default() }
  }

  /// Notes the names of the files in `dir`, as a walk lists them, so that the resolver doesn't look for config files
  /// there again.
  pub fn listing<'n>(&self, dir: &Path, names: impl IntoIterator<Item = &'n OsStr>) {
    let mut listing = Listing::default();
    for name in names {
      if name == ".editorconfig" {
        listing.editorconfig = true;
      } else if let Some(index) = NAMES.iter().position(|it| name == *it) {
        listing.configs |= 1 << index;
      }
    }
    let key = self.absolute(dir).into_os_string();
    self.listings.lock().unwrap_or_else(PoisonError::into_inner).insert(key, listing);
  }

  /// The options of the file at `path`: Prettier's, then `.editorconfig`, then the nearest tidy config.
  pub fn options(&self, path: &Path) -> Result<Options, String> {
    let path = self.absolute(path);
    let folder = match path.parent() {
      Some(dir) => self.folder(dir)?,
      None => Arc::default(),
    };
    let mut options = Options::default();
    options.share(&editor_settings(&properties(&folder.editors, &path)));
    if let Some(config) = self.fixed.as_ref().or(folder.config.as_ref()) {
      config.apply(&mut options);
    }
    Ok(options)
  }

  /// `path` from the root, without `.` and `..`, as Node's `path.resolve` gives it.
  fn absolute(&self, path: &Path) -> PathBuf {
    let mut absolute = PathBuf::new();
    for component in self.cwd.join(path).components() {
      match component {
        Component::ParentDir => {
          absolute.pop();
        }
        Component::CurDir => {}
        component => absolute.push(component),
      }
    }
    absolute
  }

  fn folder(&self, dir: &Path) -> Result<Arc<Folder>, String> {
    if let Some(found) =
      self.folders.lock().unwrap_or_else(PoisonError::into_inner).get(dir.as_os_str())
    {
      return Ok(found.clone());
    }
    let parent = match dir.parent() {
      Some(parent) => self.folder(parent)?,
      None => Arc::default(),
    };
    let listing =
      self.listings.lock().unwrap_or_else(PoisonError::into_inner).get(dir.as_os_str()).copied();
    let listing = listing.unwrap_or_else(|| look(dir));
    let mut folder = Folder { editors: Vec::new(), config: parent.config.clone() };
    let editor = match listing.editorconfig {
      true => editor(&dir.join(".editorconfig"))?,
      false => None,
    };
    match editor {
      Some(editor) if editor.is_root => folder.editors.push((dir.to_owned(), editor)),
      Some(editor) => {
        folder.editors = parent.editors.clone();
        folder.editors.push((dir.to_owned(), editor));
      }
      None => folder.editors = parent.editors.clone(),
    }
    let mut names = NAMES.iter().enumerate().filter(|(index, _)| listing.configs & 1 << index != 0);
    if let Some((_, name)) = names.next() {
      if let Some((_, other)) = names.next() {
        return Err(format!(
          "{}: more than one config in this folder, with {other}",
          dir.join(name).display()
        ));
      }
      folder.config = Some(Arc::new(Config::read(&dir.join(name))?));
    }
    let folder = Arc::new(folder);
    self
      .folders
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .insert(dir.as_os_str().to_owned(), folder.clone());
    Ok(folder)
  }
}

/// The config files in `dir`, from the file system.
fn look(dir: &Path) -> Listing {
  let mut listing = Listing { editorconfig: dir.join(".editorconfig").is_file(), configs: 0 };
  for (index, name) in NAMES.iter().enumerate() {
    if dir.join(name).is_file() {
      listing.configs |= 1 << index;
    }
  }
  listing
}

fn editor(path: &Path) -> Result<Option<Arc<Editor>>, String> {
  match ec4rs::ConfigFile::open(path) {
    Err(ec4rs::ParseError::Io(_)) => Ok(None),
    Err(error) => Err(format!("{}: {error}", path.display())),
    Ok(mut file) => {
      let is_root = file.reader.is_root;
      let sections = file
        .by_ref()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", path.display()))?;
      Ok(Some(Arc::new(Editor { is_root, sections })))
    }
  }
}

/// The properties that the `.editorconfig` files set for the file at `path`, as lower case.
fn properties<'k>(editors: &[(PathBuf, Arc<Editor>)], path: &Path) -> HashMap<&'k str, String> {
  let mut properties = HashMap::new();
  for (dir, editor) in editors {
    let relative = path.strip_prefix(dir).unwrap_or(path);
    for section in editor.sections.iter().filter(|section| section.applies_to(relative)) {
      for key in ["indent_style", "indent_size", "tab_width", "max_line_length", "end_of_line"] {
        let value = section.props().get_raw_for_key(key).into_str();
        if !value.is_empty() {
          properties.insert(key, value.to_ascii_lowercase());
        }
      }
    }
  }
  properties
}

/// Prettier's `editorconfig-to-prettier`.
fn editor_settings(properties: &HashMap<&str, String>) -> Shared {
  let get = |key: &str| properties.get(key).map(String::as_str);
  let number = |key: &str| get(key).and_then(|value| value.parse::<usize>().ok());
  let mut shared = Shared::default();
  if let Some(style) = get("indent_style") {
    shared.tabs = Some(style == "tab");
  }
  if get("indent_size") == Some("tab") {
    shared.tabs = Some(true);
  }
  shared.indent = match shared.tabs {
    Some(true) => number("tab_width").or(number("indent_size")),
    _ => number("indent_size").or(number("tab_width")),
  };
  shared.width = match get("max_line_length") {
    Some("off") => Some(usize::MAX),
    _ => number("max_line_length"),
  };
  shared.eol = match get("end_of_line") {
    Some("lf") => Some(doc::Eol::Lf),
    Some("crlf") => Some(doc::Eol::Crlf),
    Some("cr") => Some(doc::Eol::Cr),
    _ => None,
  };
  shared
}
