use napi::bindgen_prelude::Function;
use napi_derive::napi;
use serde_json::{Map, Value};
use tidyrs::config::Config;
use tidyrs::embed::{Kind, Request};
use tidyrs::{Formatter, Language, Settings, css};

/// A piece of embedded code for the host to format.
#[napi(object)]
pub struct EmbedRequest {
  pub code: String,
  /// Prettier's name for the parser: `babel`, `typescript`, `css`, `json`, ...
  pub language: String,
  #[napi(ts_type = "'block' | 'expression' | 'attribute' | 'frontmatter'")]
  pub kind: String,
  /// How many indents the code starts at. Indent the lines after the first by this much.
  pub indent: u32,
  pub width: u32,
}

/// Called with each piece of embedded code; returns its formatted text, or `undefined` to keep it.
type Callback<'a> = Function<'a, EmbedRequest, Option<String>>;

/// Options for `format`: the options of a `tidy.json` file, and what to format.
#[napi(object)]
#[derive(Default)]
pub struct FormatOptions {
  /// The language. Default: by `filepath`, and HTML without one.
  #[napi(ts_type = "'html' | 'vue' | 'handlebars' | 'tempura' | 'css' | 'scss' | 'less' | 'xml'")]
  pub parser: Option<String>,
  /// The file's path, which decides the language and some details.
  pub filepath: Option<String>,
  /// Hand embedded code to `embed`, and format style sheets. Default: true.
  pub embedded: Option<bool>,
  /// The line width. Default: 80.
  pub width: Option<u32>,
  /// The width of an indent. Default: 2.
  pub indent: Option<u32>,
  /// Indent with tabs. Default: false.
  pub tabs: Option<bool>,
  #[napi(ts_type = "'lf' | 'crlf' | 'cr'")]
  pub eol: Option<String>,
  /// Default: `auto`, and `strict` for XML.
  #[napi(ts_type = "'strict' | 'auto' | 'ignore'")]
  pub whitespace: Option<String>,
  /// Quotes in Handlebars and style sheets. Default: `double`.
  #[napi(ts_type = "'double' | 'single'")]
  pub quote: Option<String>,
  pub bracket_same_line: Option<bool>,
  pub single_attribute_per_line: Option<bool>,
  /// The ignore directive. Default: `prettier-ignore`.
  pub ignore: Option<String>,
  /// Options for HTML only: the shared ones above.
  #[napi(ts_type = "Record<string, unknown>")]
  pub html: Option<Value>,
  /// Options for Vue only: the shared ones, and `indentBlocks`.
  #[napi(ts_type = "Record<string, unknown>")]
  pub vue: Option<Value>,
  /// Options for Handlebars only: the shared ones, and `dialect`.
  #[napi(ts_type = "Record<string, unknown>")]
  pub handlebars: Option<Value>,
  /// Options for style sheets only: the shared ones, and `trailingComma`.
  #[napi(ts_type = "Record<string, unknown>")]
  pub css: Option<Value>,
  /// Options for XML only: the shared ones, and `selfClosingSpace`, `sortAttributes` and `attributeQuotes`.
  #[napi(ts_type = "Record<string, unknown>")]
  pub xml: Option<Value>,
}

/// Format a file. `embed` receives each piece of code in another language.
#[napi(
  ts_args_type = "source: string, options?: FormatOptions, embed?: (request: EmbedRequest) => string | undefined"
)]
pub fn format(
  source: String,
  options: Option<FormatOptions>,
  embed: Option<Callback<'_>>,
) -> napi::Result<String> {
  let options = options.unwrap_or_default();
  let error = |error: &dyn std::fmt::Display| napi::Error::from_reason(error.to_string());
  let (language, dialect) = language(&options)?;
  let mut config = Config::from_value(&config(&options)).map_err(|it| error(&it))?;
  if dialect.is_some() {
    config.handlebars.dialect = dialect;
  }
  let mut settings = Settings::default();
  config.apply(&mut settings);
  if options.embedded == Some(false) {
    settings.share(&tidyrs::Shared { format_embedded: Some(false), ..Default::default() });
  }
  let mut failure = None;
  let mut hook = |request: Request<'_>| -> Option<String> {
    let embed = embed.as_ref()?;
    if failure.is_some() {
      return None;
    }
    let kind = match request.kind {
      Kind::Block => "block",
      Kind::Expression => "expression",
      Kind::Attribute => "attribute",
      Kind::FrontMatter => "frontmatter",
    };
    let request = EmbedRequest {
      code: request.code.to_string(),
      language: request.language.to_string(),
      kind: kind.to_string(),
      indent: request.indent as u32,
      width: request.width as u32,
    };
    match embed.call(request) {
      Ok(text) => text,
      Err(error) => {
        failure = Some(error);
        None
      }
    }
  };
  let output = Formatter::default()
    .format(&source, language, &settings, options.filepath.as_deref(), &mut hook)
    .map(str::to_owned)
    .map_err(|it| error(&it))?;
  match failure {
    Some(error) => Err(error),
    None => Ok(output),
  }
}

/// The language of `parser`, or of `filepath`, and the Handlebars dialect that `parser` asks for.
fn language(
  options: &FormatOptions,
) -> napi::Result<(Language, Option<tidyrs::handlebars::Dialect>)> {
  use tidyrs::handlebars::Dialect;
  Ok(match options.parser.as_deref() {
    Some("html") => (Language::Html, None),
    Some("vue") => (Language::Vue, None),
    Some("handlebars") => (Language::Handlebars, None),
    Some("tempura") => (Language::Handlebars, Some(Dialect::Tempura)),
    Some("css") => (Language::Css(css::Dialect::Css), None),
    Some("scss") => (Language::Css(css::Dialect::Scss), None),
    Some("less") => (Language::Css(css::Dialect::Less), None),
    Some("xml") => (Language::Xml, None),
    Some(other) => return Err(napi::Error::from_reason(format!("unknown parser: {other}"))),
    None => {
      let language = options.filepath.as_deref().and_then(Language::of_path);
      (language.unwrap_or(Language::Html), None)
    }
  })
}

/// The options as a config, which reads and checks them as it reads a `tidy.json` file.
fn config(options: &FormatOptions) -> Value {
  let mut map = Map::new();
  let mut put = |key: &str, value: Option<Value>| {
    if let Some(value) = value {
      map.insert(key.to_owned(), value);
    }
  };
  put("width", options.width.map(Value::from));
  put("indent", options.indent.map(Value::from));
  put("tabs", options.tabs.map(Value::from));
  put("eol", options.eol.clone().map(Value::from));
  put("whitespace", options.whitespace.clone().map(Value::from));
  put("quote", options.quote.clone().map(Value::from));
  put("bracketSameLine", options.bracket_same_line.map(Value::from));
  put("singleAttributePerLine", options.single_attribute_per_line.map(Value::from));
  put("ignore", options.ignore.clone().map(Value::from));
  put("html", options.html.clone());
  put("vue", options.vue.clone());
  put("handlebars", options.handlebars.clone());
  put("css", options.css.clone());
  put("xml", options.xml.clone());
  Value::Object(map)
}
