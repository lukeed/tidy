//! The options of every language, and a formatter that takes any of them. Each language can also be formatted on its
//! own, with its own options: `tidy::css::format(text, &options.css, embed)`.

use crate::embed::Embed;
#[cfg(feature = "js")]
use crate::js;
use crate::{
  Error, Whitespace, css, doc, dockerfile, graphql, handlebars, html, ini, json, markdown, toml,
  xml, yaml,
};

/// A language that tidy formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
  Html,
  Vue,
  Handlebars,
  Css(css::Dialect),
  Json(json::Dialect),
  Toml,
  Xml,
  Yaml,
  Markdown,
  Dockerfile,
  Ini(ini::Dialect),
  Graphql,
  #[cfg(feature = "js")]
  Js(js::Dialect),
}

impl Language {
  /// The language of the file at `path`, by its name, or `None` if tidy doesn't format it.
  pub fn of_path(path: &str) -> Option<Language> {
    if let Some(dialect) = css::Dialect::of_path(path) {
      return Some(Language::Css(dialect));
    }
    if let Some(dialect) = json::Dialect::of_path(path) {
      return Some(Language::Json(dialect));
    }
    if toml::is_toml_path(path) {
      return Some(Language::Toml);
    }
    if dockerfile::is_dockerfile_path(path) {
      return Some(Language::Dockerfile);
    }
    if graphql::is_graphql_path(path) {
      return Some(Language::Graphql);
    }
    #[cfg(feature = "js")]
    if let Some(dialect) = js::Dialect::of_path(path) {
      return Some(Language::Js(dialect));
    }
    if let Some(dialect) = ini::Dialect::of_path(path) {
      return Some(Language::Ini(dialect));
    }
    if yaml::is_yaml_path(path) {
      return Some(Language::Yaml);
    }
    if handlebars::is_handlebars_path(path) {
      return Some(Language::Handlebars);
    }
    if xml::is_xml_path(path) {
      return Some(Language::Xml);
    }
    let extension = path.rsplit_once('.')?.1.to_ascii_lowercase();
    match extension.as_str() {
      "html" | "htm" | "xht" | "xhtml" => Some(Language::Html),
      "vue" => Some(Language::Vue),
      "md" | "markdown" => Some(Language::Markdown),
      _ => None,
    }
  }
}

/// The options of each language, Prettier's by default.
#[derive(Clone, Debug)]
pub struct Options {
  pub html: html::Options,
  pub vue: html::Options,
  pub handlebars: handlebars::Options,
  /// For CSS, SCSS and Less.
  pub css: css::Options,
  /// For every dialect of JSON.
  pub json: json::Options,
  pub toml: toml::Options,
  pub xml: xml::Options,
  pub yaml: yaml::Options,
  pub markdown: markdown::Options,
  pub dockerfile: dockerfile::Options,
  /// For every dialect of INI.
  pub ini: ini::Options,
  pub graphql: graphql::Options,
  /// For JavaScript and TypeScript, in files and in the scripts, template expressions and code blocks of other
  /// languages, where the host's width and indent apply.
  #[cfg(feature = "js")]
  pub js: js::Options,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      html: html::Options::default(),
      vue: html::Options { parser: html::Parser::Vue, ..html::Options::default() },
      handlebars: handlebars::Options::default(),
      css: css::Options::default(),
      json: json::Options::default(),
      toml: toml::Options::default(),
      xml: xml::Options::default(),
      yaml: yaml::Options::default(),
      markdown: markdown::Options::default(),
      dockerfile: dockerfile::Options::default(),
      ini: ini::Options::default(),
      graphql: graphql::Options::default(),
      #[cfg(feature = "js")]
      js: js::Options::default(),
    }
  }
}

impl Options {
  #[cfg(feature = "js")]
  fn scripts(&self) -> &crate::embed::Scripts {
    &self.js
  }

  #[cfg(not(feature = "js"))]
  fn scripts(&self) -> &crate::embed::Scripts {
    &()
  }

  /// Applies the choices that are set in `shared` to every language that has them.
  pub fn share(&mut self, shared: &Shared) {
    shared.html(&mut self.html);
    shared.html(&mut self.vue);
    shared.handlebars(&mut self.handlebars);
    shared.css(&mut self.css);
    shared.json(&mut self.json);
    shared.toml(&mut self.toml);
    shared.xml(&mut self.xml);
    shared.yaml(&mut self.yaml);
    shared.markdown(&mut self.markdown);
    shared.dockerfile(&mut self.dockerfile);
    shared.ini(&mut self.ini);
    shared.graphql(&mut self.graphql);
    #[cfg(feature = "js")]
    shared.js(&mut self.js);
  }
}

/// Choices that more than one language makes. `None` leaves a language's own setting as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shared {
  pub width: Option<usize>,
  /// The width of an indent.
  pub indent: Option<usize>,
  /// Indent with tabs.
  pub tabs: Option<bool>,
  pub eol: Option<doc::Eol>,
  /// HTML, Vue, Handlebars and XML.
  pub whitespace: Option<Whitespace>,
  /// Prefer single quotes: Handlebars, style sheets, JSON5 and scripts.
  pub single_quote: Option<bool>,
  /// Prettier's `trailingComma` is not `"none"`: SCSS, JSONC, JSON5, TOML arrays and scripts.
  pub trailing_comma: Option<bool>,
  /// Spaces inside the braces of an object on one line: JSON, YAML's flow mappings and scripts.
  pub bracket_spacing: Option<bool>,
  /// HTML, Vue, XML and JSX.
  pub bracket_same_line: Option<bool>,
  /// HTML, Vue and XML.
  pub single_attribute_per_line: Option<bool>,
  /// The ignore directive.
  pub ignore: Option<String>,
  /// Prettier's `embeddedLanguageFormatting`.
  pub format_embedded: Option<bool>,
}

impl Shared {
  /// The choices of `self`, with those that `other` sets instead.
  pub fn and(&self, other: &Shared) -> Shared {
    Shared {
      width: other.width.or(self.width),
      indent: other.indent.or(self.indent),
      tabs: other.tabs.or(self.tabs),
      eol: other.eol.or(self.eol),
      whitespace: other.whitespace.or(self.whitespace),
      single_quote: other.single_quote.or(self.single_quote),
      trailing_comma: other.trailing_comma.or(self.trailing_comma),
      bracket_spacing: other.bracket_spacing.or(self.bracket_spacing),
      bracket_same_line: other.bracket_same_line.or(self.bracket_same_line),
      single_attribute_per_line: other.single_attribute_per_line.or(self.single_attribute_per_line),
      ignore: other.ignore.clone().or_else(|| self.ignore.clone()),
      format_embedded: other.format_embedded.or(self.format_embedded),
    }
  }

  pub fn html(&self, options: &mut html::Options) {
    set(&mut options.width, self.width);
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
    set(&mut options.whitespace, self.whitespace);
    set(&mut options.bracket_same_line, self.bracket_same_line);
    set(&mut options.single_attribute_per_line, self.single_attribute_per_line);
    set(&mut options.ignore, self.ignore.clone());
    set(&mut options.format_embedded, self.format_embedded);
  }

  pub fn handlebars(&self, options: &mut handlebars::Options) {
    set(&mut options.width, self.width);
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
    set(&mut options.whitespace, self.whitespace);
    set(&mut options.single_quote, self.single_quote);
    set(&mut options.ignore, self.ignore.clone());
    set(&mut options.format_embedded, self.format_embedded);
  }

  pub fn css(&self, options: &mut css::Options) {
    set(&mut options.width, self.width);
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
    set(&mut options.single_quote, self.single_quote);
    set(&mut options.trailing_comma, self.trailing_comma);
    set(&mut options.ignore, self.ignore.clone());
    set(&mut options.format_embedded, self.format_embedded);
  }

  pub fn json(&self, options: &mut json::Options) {
    set(&mut options.width, self.width);
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
    set(&mut options.single_quote, self.single_quote);
    set(&mut options.trailing_comma, self.trailing_comma);
    set(&mut options.bracket_spacing, self.bracket_spacing);
    set(&mut options.ignore, self.ignore.clone());
  }

  pub fn yaml(&self, options: &mut yaml::Options) {
    set(&mut options.width, self.width);
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
    set(&mut options.single_quote, self.single_quote);
    set(&mut options.trailing_comma, self.trailing_comma);
    set(&mut options.bracket_spacing, self.bracket_spacing);
    set(&mut options.ignore, self.ignore.clone());
  }

  pub fn toml(&self, options: &mut toml::Options) {
    set(&mut options.column_width, self.width);
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
    set(&mut options.array_trailing_comma, self.trailing_comma);
  }

  /// The indent is the indent of a stage, with `indentStages`: dprint-plugin-dockerfile's `indentWidth`.
  pub fn dockerfile(&self, options: &mut dockerfile::Options) {
    set(&mut options.width, self.width);
    set(&mut options.indent_width, self.indent);
    set(&mut options.eol, self.eol);
  }

  /// The indent is that of the values of a key over lines.
  pub fn ini(&self, options: &mut ini::Options) {
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
  }

  pub fn graphql(&self, options: &mut graphql::Options) {
    set(&mut options.width, self.width);
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
    set(&mut options.bracket_spacing, self.bracket_spacing);
    set(&mut options.ignore, self.ignore.clone());
  }

  /// `trailing_comma` turns trailing commas on or off, and keeps `es5` when on.
  #[cfg(feature = "js")]
  pub fn js(&self, options: &mut js::Options) {
    set(&mut options.width, self.width);
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
    set(&mut options.single_quote, self.single_quote);
    set(&mut options.bracket_spacing, self.bracket_spacing);
    set(&mut options.bracket_same_line, self.bracket_same_line);
    set(&mut options.format_embedded, self.format_embedded);
    match self.trailing_comma {
      Some(false) => options.trailing_comma = js::TrailingComma::None,
      Some(true) if options.trailing_comma == js::TrailingComma::None => {
        options.trailing_comma = js::TrailingComma::All
      }
      _ => {}
    }
  }

  pub fn xml(&self, options: &mut xml::Options) {
    set(&mut options.width, self.width);
    set(&mut options.tab_width, self.indent);
    set(&mut options.tabs, self.tabs);
    set(&mut options.eol, self.eol);
    set(&mut options.whitespace, self.whitespace);
    set(&mut options.bracket_same_line, self.bracket_same_line);
    set(&mut options.single_attribute_per_line, self.single_attribute_per_line);
    set(&mut options.ignore, self.ignore.clone());
    set(&mut options.format_embedded, self.format_embedded);
  }

  pub fn markdown(&self, options: &mut markdown::Options) {
    set(&mut options.width, self.width);
    set(&mut options.ignore, self.ignore.clone());
    set(&mut options.format_embedded, self.format_embedded);
  }
}

fn set<T>(field: &mut T, value: Option<T>) {
  if let Some(value) = value {
    *field = value;
  }
}

/// Formats `text` as `language`. `path` is the file's path, which decides some details, such as whether a `.hbs`
/// template is HTML.
pub fn format(
  text: &str,
  language: Language,
  options: &Options,
  path: Option<&str>,
  embed: &mut dyn Embed,
) -> Result<String, Error> {
  Formatter::default().format(text, language, options, path, embed).map(str::to_owned)
}

/// Formats one file after another, in any language, reusing the buffers of each.
#[derive(Default)]
pub struct Formatter {
  html: html::Formatter,
  handlebars: handlebars::Formatter,
  css: css::Formatter,
  json: json::Formatter,
  toml: toml::Formatter,
  xml: xml::Formatter,
  yaml: yaml::Formatter,
  dockerfile: dockerfile::Formatter,
  ini: ini::Formatter,
  graphql: graphql::Formatter,
  #[cfg(feature = "js")]
  js: js::Formatter,
  markdown: String,
}

impl Formatter {
  /// Formats `text` as `language`. `path` is the file's path.
  pub fn format(
    &mut self,
    text: &str,
    language: Language,
    options: &Options,
    path: Option<&str>,
    embed: &mut dyn Embed,
  ) -> Result<&str, Error> {
    let filepath = path.map(str::to_owned);
    match language {
      Language::Html | Language::Vue => {
        let base = if language == Language::Vue { &options.vue } else { &options.html };
        let options = html::Options {
          filepath,
          #[cfg(feature = "js")]
          js: options.js.clone(),
          ..base.clone()
        };
        self.html.format(text, &options, embed)
      }
      Language::Handlebars => {
        let options = handlebars::Options {
          filepath,
          #[cfg(feature = "js")]
          js: options.js.clone(),
          ..options.handlebars.clone()
        };
        self.handlebars.format(text, &options, embed)
      }
      Language::Css(dialect) => {
        let options = css::Options { dialect, ..options.css.clone() };
        self.css.format(text, &options, embed)
      }
      Language::Json(dialect) => {
        let options = json::Options { dialect, ..options.json.clone() };
        self.json.format(text, &options)
      }
      Language::Toml => self.toml.format(text, &options.toml),
      Language::Dockerfile => self.dockerfile.format(text, &options.dockerfile),
      Language::Ini(dialect) => {
        let options = ini::Options { dialect, ..options.ini.clone() };
        self.ini.format(text, &options)
      }
      Language::Graphql => self.graphql.format(text, &options.graphql),
      #[cfg(feature = "js")]
      Language::Js(dialect) => {
        let options = js::Options { dialect, ..options.js.clone() };
        self.js.format(text, path.unwrap_or_default(), &options)
      }
      Language::Xml => {
        #[cfg(feature = "js")]
        let options = &xml::Options { js: options.js.clone(), ..options.xml.clone() };
        #[cfg(not(feature = "js"))]
        let options = &options.xml;
        self.xml.format(text, options, embed)
      }
      Language::Yaml => {
        let options = yaml::Options { filepath, ..options.yaml.clone() };
        self.yaml.format(text, &options)
      }
      Language::Markdown => {
        let (bom, text) = crate::without_bom(text);
        self.markdown = match options.markdown.format_embedded {
          true => markdown::format_with(text, &options.markdown, |code| {
            let (tab_width, tabs) = (options.yaml.tab_width, options.yaml.tabs);
            crate::embed::code_block(code, &options.css, tab_width, tabs, options.scripts(), embed)
          }),
          false => markdown::format(text, &options.markdown),
        };
        Ok(crate::with_bom(bom, &mut self.markdown))
      }
    }
  }
}
