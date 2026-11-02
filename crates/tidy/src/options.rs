//! The options of every language, and a formatter that takes any of them. Each language can also be formatted on its
//! own, with its own options: `tidy::css::format(text, &options.css, embed)`.

use crate::embed::Embed;
use crate::embed::{Kind, Request};
use crate::{Error, Whitespace, css, doc, handlebars, html, json, markdown, toml, xml, yaml};

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
    }
  }
}

impl Options {
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
  /// Prefer single quotes: Handlebars, style sheets and JSON5.
  pub single_quote: Option<bool>,
  /// Prettier's `trailingComma` is not `"none"`: SCSS, JSONC, JSON5, and TOML arrays.
  pub trailing_comma: Option<bool>,
  /// Spaces inside the braces of an object on one line: JSON, and YAML's flow mappings.
  pub bracket_spacing: Option<bool>,
  /// HTML, Vue and XML.
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
        let options = html::Options { filepath, ..base.clone() };
        self.html.format(text, &options, embed)
      }
      Language::Handlebars => {
        let options = handlebars::Options { filepath, ..options.handlebars.clone() };
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
      Language::Xml => self.xml.format(text, &options.xml, embed),
      Language::Yaml => {
        let options = yaml::Options { filepath, ..options.yaml.clone() };
        self.yaml.format(text, &options)
      }
      Language::Markdown => {
        let (bom, text) = crate::without_bom(text);
        self.markdown = match options.markdown.format_embedded {
          true => markdown::format_with(text, &options.markdown, |code| {
            let language = crate::languages::parser(code.lang).or(crate::embed::own(code.lang))?;
            let request = Request {
              code: code.text,
              language,
              kind: Kind::Block,
              indent: 0,
              width: code.width,
              module: true,
            };
            embed
              .format(request)
              .or_else(|| {
                let dialect = css::Dialect::of_path(&format!("code.{language}"))?;
                let options = css::Options { dialect, width: code.width, ..options.css.clone() };
                css::format(code.text, &options, &mut *embed).ok()
              })
              .or_else(|| {
                crate::embed::builtin(&request, options.yaml.tab_width, options.yaml.tabs)
              })
          }),
          false => markdown::format(text, &options.markdown),
        };
        Ok(crate::with_bom(bom, &mut self.markdown))
      }
    }
  }
}
