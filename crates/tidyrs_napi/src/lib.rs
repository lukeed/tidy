use napi::bindgen_prelude::Function;
use napi_derive::napi;
use tidyrs::Whitespace;
use tidyrs::embed::{Kind, Request};
use tidyrs::html::{Parser, Settings};
use tidyrs::{css, handlebars, xml};

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
  #[napi(ts_type = "'html' | 'vue' | 'handlebars' | 'css' | 'scss' | 'less' | 'xml'")]
  pub parser: Option<String>,
  /// The line width. Default: 80.
  pub width: Option<u32>,
  /// The width of an indent. Default: 2.
  pub tab_width: Option<u32>,
  pub use_tabs: Option<bool>,
  /// `css` is for HTML and Handlebars only, `preserve` for XML only. Default: `css`, and `strict` for XML.
  #[napi(ts_type = "'css' | 'strict' | 'preserve' | 'ignore'")]
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
  /// XML: a space before `/>` and `?>`. Default: true.
  pub self_closing_space: Option<bool>,
  /// XML: sort attributes by name, `xmlns` first. Default: false.
  pub sort_attributes: Option<bool>,
  /// XML: the quotes around attribute values. Default: `"preserve"`.
  #[napi(ts_type = "'preserve' | 'single' | 'double'")]
  pub quote_attributes: Option<String>,
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
  let language = match options.parser.as_deref() {
    None | Some("html" | "vue") => Language::Markup,
    Some("handlebars") => Language::Handlebars,
    Some("css") => Language::Css(css::Dialect::Css),
    Some("scss") => Language::Css(css::Dialect::Scss),
    Some("less") => Language::Css(css::Dialect::Less),
    Some("xml") => Language::Xml,
    Some(other) => return Err(invalid("parser", other)),
  };
  let single_quote = options.single_quote.unwrap_or_default();
  let trailing_comma = options.trailing_comma.as_deref() != Some("none");
  let format_embedded = options.embedded_language_formatting.as_deref() != Some("off");
  let xml = match language {
    Language::Xml => Some(xml_settings(&options, format_embedded)?),
    _ => None,
  };
  let settings = settings(options, language)?;
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
  let output = match language {
    Language::Css(dialect) => {
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
    Language::Handlebars => {
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
    Language::Xml => {
      let settings = xml.unwrap_or_default();
      xml::format(&source, &settings, &mut hook).map_err(|it| error(&it))?
    }
    Language::Markup => {
      let settings = Settings { format_embedded, ..settings };
      tidyrs::html::format(&source, &settings, &mut hook).map_err(|it| error(&it))?
    }
  };
  match failure {
    Some(error) => Err(error),
    None => Ok(output),
  }
}

#[derive(Clone, Copy)]
enum Language {
  Markup,
  Handlebars,
  Css(css::Dialect),
  Xml,
}

fn invalid(name: &str, value: &str) -> napi::Error {
  napi::Error::from_reason(format!("unknown {name}: {value}"))
}

fn xml_settings(input: &FormatOptions, format_embedded: bool) -> napi::Result<xml::Settings> {
  let whitespace = match input.whitespace.as_deref() {
    None | Some("strict") => tidyrs::Whitespace::Strict,
    Some("preserve") => tidyrs::Whitespace::Auto,
    Some("ignore") => tidyrs::Whitespace::Ignore,
    Some(other) => return Err(invalid("whitespace", other)),
  };
  let quotes = match input.quote_attributes.as_deref() {
    None | Some("preserve") => xml::Quotes::Preserve,
    Some("single") => xml::Quotes::Single,
    Some("double") => xml::Quotes::Double,
    Some(other) => return Err(invalid("quoteAttributes", other)),
  };
  let defaults = xml::Settings::default();
  Ok(xml::Settings {
    width: input.width.map_or(defaults.width, |width| width as usize),
    tab_width: input.tab_width.map_or(defaults.tab_width, |width| width as usize),
    tabs: input.use_tabs.unwrap_or(defaults.tabs),
    whitespace,
    self_closing_space: input.self_closing_space.unwrap_or(defaults.self_closing_space),
    sort_attributes: input.sort_attributes.unwrap_or_default(),
    quotes,
    bracket_same_line: input.bracket_same_line.unwrap_or_default(),
    single_attribute_per_line: input.single_attribute_per_line.unwrap_or_default(),
    format_embedded,
    ..defaults
  })
}

fn settings(input: FormatOptions, language: Language) -> napi::Result<Settings> {
  let mut settings = Settings::default();
  if input.parser.as_deref() == Some("vue") {
    settings.parser = Parser::Vue;
  }
  if let Some(whitespace) = input.whitespace.filter(|_| !matches!(language, Language::Xml)) {
    settings.whitespace = match whitespace.as_str() {
      "css" => Whitespace::Auto,
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
