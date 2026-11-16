use napi::bindgen_prelude::Function;
use napi_derive::napi;
use serde_json::{Map, Value};
use tidy::config::Config;
use tidy::embed::{Kind, Request};
use tidy::{Formatter, Language, Options};

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
  /// The width of the text before the code on its first line, after the indents, as `name="` before an attribute
  /// value. Print the code after this much.
  pub prefix: u32,
  /// The width of the text after the code on its last line, as the `"` that closes such a value.
  pub suffix: u32,
  pub width: u32,
  /// JavaScript that is a module, not a classic script.
  pub module: bool,
}

/// Called with each piece of embedded code; returns its formatted text, or `undefined` to keep it.
type Callback<'a> = Function<'a, EmbedRequest, Option<String>>;

/// Options for `format`: the options of a `tidy.json` file, and what to format.
#[napi(object)]
#[derive(Default)]
pub struct FormatOptions {
  /// The language, by the name a code fence would give it, such as `ts`, `css` or `md`, or by a file extension,
  /// such as `xml`. Default: by `filepath`.
  #[napi(
    ts_type = "'css' | 'scss' | 'less' | 'html' | 'vue' | 'mjml' | 'hbs' | 'tempura' | 'js' | 'jsx' | 'ts' | 'tsx' | 'json' | 'jsonc' | 'json5' | 'yaml' | 'toml' | 'md' | 'graphql' | 'xml' | 'dockerfile' | 'ini' | 'gitconfig' | (string & {})"
  )]
  pub lang: Option<String>,
  /// The file's path, which decides the language and some details.
  pub filepath: Option<String>,
  /// Hand embedded code to `embed`, and format the style sheets, JSON, YAML and TOML it declines. Default: true.
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
  /// Quotes in Handlebars, style sheets, JSON5, YAML and JS. Default: `double`.
  #[napi(ts_type = "'double' | 'single'")]
  pub quote: Option<String>,
  /// Trailing commas in SCSS, JSONC, JSON5, YAML flow collections, TOML arrays and JS. Default: true.
  pub trailing_comma: Option<bool>,
  /// Spaces inside the braces of JSON and JS objects and YAML flow mappings. Default: true.
  pub bracket_spacing: Option<bool>,
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
  /// Options for style sheets only: the shared ones above.
  #[napi(ts_type = "Record<string, unknown>")]
  pub css: Option<Value>,
  /// Options for JSON only: the shared ones, and `quoteProps` and `objectWrap`.
  #[napi(ts_type = "Record<string, unknown>")]
  pub json: Option<Value>,
  /// Options for YAML only: the shared ones, and `wrap` (`preserve`, `always` or `never`).
  #[napi(ts_type = "Record<string, unknown>")]
  pub yaml: Option<Value>,
  /// Options for TOML only: the shared ones, and taplo's, in camel case: `alignEntries`, `reorderKeys`, ...
  #[napi(ts_type = "Record<string, unknown>")]
  pub toml: Option<Value>,
  /// Options for Dockerfiles only: the shared ones, and dprint-plugin-dockerfile's `indentStages` and
  /// `healthcheckCmdNewLine`.
  #[napi(ts_type = "Record<string, unknown>")]
  pub dockerfile: Option<Value>,
  /// Options for INI only: the shared ones, and `spaceAroundEquals`.
  #[napi(ts_type = "Record<string, unknown>")]
  pub ini: Option<Value>,
  /// Options for GraphQL only: the shared ones.
  #[napi(ts_type = "Record<string, unknown>")]
  pub graphql: Option<Value>,
  /// Options for JavaScript and TypeScript, also where tidy formats them in other languages: the shared ones,
  /// `trailingComma` as `all`, `es5` or `none`, and `semi`, `jsxSingleQuote`, `quoteProps`, `arrowParens`,
  /// `objectWrap`, `experimentalTernaries`, `experimentalOperatorPosition` and `jsdoc`. Imports are sorted with oxfmt's
  /// `sortImports`, or with `plugins` that lists `@trivago/prettier-plugin-sort-imports`,
  /// `@ianvs/prettier-plugin-sort-imports` or `prettier-plugin-organize-imports` and that plugin's options
  /// (`importOrder`, `importOrderSeparation`, `organizeImportsTypeOrder`, ...). Off by default.
  #[napi(ts_type = "Record<string, unknown>")]
  pub js: Option<Value>,
  /// Options for XML only: the shared ones, and `selfClosingSpace`, `sortAttributes` and `attributeQuotes`.
  #[napi(ts_type = "Record<string, unknown>")]
  pub xml: Option<Value>,
  /// Options for Markdown only: `width` and `ignore`, a `preset` (`prettier` or `preserve`), and `wrap`, `bullet`,
  /// `emphasis`, `strong`, `numbering`, `rule`, `fence` and `heading`, which override the preset.
  #[napi(ts_type = "Record<string, unknown>")]
  pub markdown: Option<Value>,
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
  let input = options.unwrap_or_default();
  let error = |error: &dyn std::fmt::Display| napi::Error::from_reason(error.to_string());
  let (language, dialect) = language(&input)?;
  let mut config = Config::from_value(&config(&input)).map_err(|it| error(&it))?;
  if dialect.is_some() {
    config.handlebars.dialect = dialect;
  }
  let mut options = Options::default();
  config.apply(&mut options);
  if input.embedded == Some(false) {
    options.share(&tidy::Shared { format_embedded: Some(false), ..Default::default() });
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
      prefix: request.prefix as u32,
      suffix: request.suffix as u32,
      width: request.width as u32,
      module: request.module,
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
    .format(&source, language, &options, input.filepath.as_deref(), &mut hook)
    .map(str::to_owned)
    .map_err(|it| error(&it))?;
  match failure {
    Some(error) => Err(error),
    None => Ok(output),
  }
}

/// Whether a file is formatted already. `embed` receives each piece of code in another language, as for `format`.
#[napi(
  ts_args_type = "source: string, options?: FormatOptions, embed?: (request: EmbedRequest) => string | undefined"
)]
pub fn check(
  source: String,
  options: Option<FormatOptions>,
  embed: Option<Callback<'_>>,
) -> napi::Result<bool> {
  let formatted = format(source.clone(), options, embed)?;
  Ok(formatted == source)
}

/// The language of `lang`, or of `filepath`, and the Handlebars dialect that `lang` asks for, as the command line
/// reads `--lang`.
fn language(input: &FormatOptions) -> napi::Result<(Language, Option<tidy::handlebars::Dialect>)> {
  let fail = |message: String| napi::Error::from_reason(message);
  match (input.lang.as_deref(), input.filepath.as_deref()) {
    // A dialect of Handlebars, which tidy otherwise tells from the template.
    (Some("tempura"), _) => Ok((Language::Handlebars, Some(tidy::handlebars::Dialect::Tempura))),
    (Some(name), _) => Language::hint(name)
      .map(|it| (it, None))
      .ok_or_else(|| fail(format!("unknown language: {name}"))),
    (None, Some(path)) => Language::of_path(path)
      .map(|it| (it, None))
      .ok_or_else(|| fail(format!("{path}: can't tell the language of this file, pass `lang`"))),
    (None, None) => Err(fail("can't tell the language: pass `lang` or `filepath`".into())),
  }
}

/// The options as a config, which reads and checks them as it reads a `tidy.json` file.
fn config(input: &FormatOptions) -> Value {
  let mut map = Map::new();
  let mut put = |key: &str, value: Option<Value>| {
    if let Some(value) = value {
      map.insert(key.to_owned(), value);
    }
  };
  put("width", input.width.map(Value::from));
  put("indent", input.indent.map(Value::from));
  put("tabs", input.tabs.map(Value::from));
  put("eol", input.eol.clone().map(Value::from));
  put("whitespace", input.whitespace.clone().map(Value::from));
  put("quote", input.quote.clone().map(Value::from));
  put("trailingComma", input.trailing_comma.map(Value::from));
  put("bracketSpacing", input.bracket_spacing.map(Value::from));
  put("bracketSameLine", input.bracket_same_line.map(Value::from));
  put("singleAttributePerLine", input.single_attribute_per_line.map(Value::from));
  put("ignore", input.ignore.clone().map(Value::from));
  put("html", input.html.clone());
  put("vue", input.vue.clone());
  put("handlebars", input.handlebars.clone());
  put("css", input.css.clone());
  put("json", input.json.clone());
  put("toml", input.toml.clone());
  put("dockerfile", input.dockerfile.clone());
  put("ini", input.ini.clone());
  put("graphql", input.graphql.clone());
  put("js", input.js.clone());
  put("yaml", input.yaml.clone());
  put("xml", input.xml.clone());
  put("markdown", input.markdown.clone());
  Value::Object(map)
}
