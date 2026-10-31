use napi::bindgen_prelude::Function;
use napi_derive::napi;
use tidyrs::embed::{Kind, Request};
use tidyrs::html::{Parser, Settings, Whitespace};
use tidyrs::{css, handlebars};

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

/// Options for `format`.
#[napi(object)]
#[derive(Default)]
pub struct FormatOptions {
  #[napi(ts_type = "'html' | 'vue' | 'handlebars' | 'css' | 'scss' | 'less'")]
  pub parser: Option<String>,
  /// The line width. Default: 80.
  pub width: Option<u32>,
  /// The width of an indent. Default: 2.
  pub tab_width: Option<u32>,
  pub use_tabs: Option<bool>,
  #[napi(ts_type = "'css' | 'strict' | 'ignore'")]
  pub whitespace: Option<String>,
  pub single_attribute_per_line: Option<bool>,
  pub bracket_same_line: Option<bool>,
  pub vue_indent_script_and_style: Option<bool>,
  /// Prefer single quotes in Handlebars and style sheets. Default: false.
  pub single_quote: Option<bool>,
  /// Prettier's `embeddedLanguageFormatting`. Off, the code of other languages stays as it is written. Default:
  /// `"auto"`.
  #[napi(ts_type = "'auto' | 'off'")]
  pub embedded_language_formatting: Option<String>,
  /// Prettier's `trailingComma`, for SCSS maps and lists. Default: `"all"`.
  #[napi(ts_type = "'all' | 'es5' | 'none'")]
  pub trailing_comma: Option<String>,
  /// The file's path, which decides some details.
  pub filepath: Option<String>,
}

/// Format markup. `embed` receives each piece of code in another language.
#[napi(
  ts_args_type = "source: string, options?: FormatOptions, embed?: (request: EmbedRequest) => string | undefined"
)]
pub fn format(
  source: String,
  options: Option<FormatOptions>,
  embed: Option<Callback<'_>>,
) -> napi::Result<String> {
  let options = options.unwrap_or_default();
  let dialect = match options.parser.as_deref() {
    Some("css") => Some(css::Dialect::Css),
    Some("scss") => Some(css::Dialect::Scss),
    Some("less") => Some(css::Dialect::Less),
    _ => None,
  };
  let is_handlebars = options.parser.as_deref() == Some("handlebars") || dialect.is_some();
  let single_quote = options.single_quote.unwrap_or_default();
  let trailing_comma = options.trailing_comma.as_deref() != Some("none");
  let format_embedded = options.embedded_language_formatting.as_deref() != Some("off");
  let settings = settings(options, is_handlebars)?;
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
  let error = |error: &dyn std::fmt::Display| napi::Error::from_reason(error.to_string());
  let output = match is_handlebars {
    true if let Some(dialect) = dialect => {
      let settings = css::Settings {
        dialect,
        width: settings.width,
        tab_width: settings.tab_width,
        tabs: settings.tabs,
        single_quote,
        trailing_comma,
        format_embedded,
        ..css::Settings::default()
      };
      css::format(&source, &settings, &mut hook).map_err(|it| error(&it))?
    }
    true => {
      let settings = handlebars::Settings {
        width: settings.width,
        tab_width: settings.tab_width,
        tabs: settings.tabs,
        whitespace: settings.whitespace,
        single_quote,
        filepath: settings.filepath.clone(),
        format_embedded,
        ..handlebars::Settings::default()
      };
      handlebars::format(&source, &settings, &mut hook).map_err(|it| error(&it))?
    }
    false => {
      let settings = Settings { format_embedded, ..settings };
      tidyrs::html::format(&source, &settings, &mut hook).map_err(|it| error(&it))?
    }
  };
  match failure {
    Some(error) => Err(error),
    None => Ok(output),
  }
}

fn settings(input: FormatOptions, is_handlebars: bool) -> napi::Result<Settings> {
  let invalid =
    |name: &str, value: &str| napi::Error::from_reason(format!("unknown {name}: {value}"));
  let mut settings = Settings::default();
  if let Some(parser) = input.parser.filter(|_| !is_handlebars) {
    settings.parser = match parser.as_str() {
      "html" => Parser::Html,
      "vue" => Parser::Vue,
      other => return Err(invalid("parser", other)),
    };
  }
  if let Some(whitespace) = input.whitespace {
    settings.whitespace = match whitespace.as_str() {
      "css" => Whitespace::Css,
      "strict" => Whitespace::Strict,
      "ignore" => Whitespace::Ignore,
      other => return Err(invalid("whitespace", other)),
    };
  }
  settings.width = input.width.map_or(settings.width, |width| width as usize);
  settings.tab_width = input.tab_width.map_or(settings.tab_width, |width| width as usize);
  settings.tabs = input.use_tabs.unwrap_or(settings.tabs);
  settings.single_attribute_per_line = input.single_attribute_per_line.unwrap_or_default();
  settings.bracket_same_line = input.bracket_same_line.unwrap_or_default();
  settings.vue_indent_script_and_style = input.vue_indent_script_and_style.unwrap_or_default();
  settings.filepath = input.filepath;
  Ok(settings)
}
