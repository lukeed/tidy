//! The settings of every language, and a formatter that takes any of them. Each language can also be formatted on its
//! own, with its own settings: `tidy::css::format(text, &settings.css, embed)`.

use crate::embed::Embed;
use crate::{Error, Whitespace, css, doc, handlebars, html, xml};

/// A language that tidy formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
  Html,
  Vue,
  Handlebars,
  Css(css::Dialect),
  Xml,
}

impl Language {
  /// The language of the file at `path`, by its name, or `None` if tidy doesn't format it.
  pub fn of_path(path: &str) -> Option<Language> {
    if let Some(dialect) = css::Dialect::of_path(path) {
      return Some(Language::Css(dialect));
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
      _ => None,
    }
  }
}

/// The settings of each language, Prettier's by default.
#[derive(Clone, Debug)]
pub struct Settings {
  pub html: html::Settings,
  pub vue: html::Settings,
  pub handlebars: handlebars::Settings,
  /// For CSS, SCSS and Less.
  pub css: css::Settings,
  pub xml: xml::Settings,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      html: html::Settings::default(),
      vue: html::Settings { parser: html::Parser::Vue, ..html::Settings::default() },
      handlebars: handlebars::Settings::default(),
      css: css::Settings::default(),
      xml: xml::Settings::default(),
    }
  }
}

impl Settings {
  /// Applies the choices that are set in `shared` to every language that has them.
  pub fn share(&mut self, shared: &Shared) {
    shared.html(&mut self.html);
    shared.html(&mut self.vue);
    shared.handlebars(&mut self.handlebars);
    shared.css(&mut self.css);
    shared.xml(&mut self.xml);
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
  /// Prefer single quotes: Handlebars and style sheets.
  pub single_quote: Option<bool>,
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
      bracket_same_line: other.bracket_same_line.or(self.bracket_same_line),
      single_attribute_per_line: other.single_attribute_per_line.or(self.single_attribute_per_line),
      ignore: other.ignore.clone().or_else(|| self.ignore.clone()),
      format_embedded: other.format_embedded.or(self.format_embedded),
    }
  }

  pub fn html(&self, settings: &mut html::Settings) {
    set(&mut settings.width, self.width);
    set(&mut settings.tab_width, self.indent);
    set(&mut settings.tabs, self.tabs);
    set(&mut settings.eol, self.eol);
    set(&mut settings.whitespace, self.whitespace);
    set(&mut settings.bracket_same_line, self.bracket_same_line);
    set(&mut settings.single_attribute_per_line, self.single_attribute_per_line);
    set(&mut settings.ignore, self.ignore.clone());
    set(&mut settings.format_embedded, self.format_embedded);
  }

  pub fn handlebars(&self, settings: &mut handlebars::Settings) {
    set(&mut settings.width, self.width);
    set(&mut settings.tab_width, self.indent);
    set(&mut settings.tabs, self.tabs);
    set(&mut settings.eol, self.eol);
    set(&mut settings.whitespace, self.whitespace);
    set(&mut settings.single_quote, self.single_quote);
    set(&mut settings.ignore, self.ignore.clone());
    set(&mut settings.format_embedded, self.format_embedded);
  }

  pub fn css(&self, settings: &mut css::Settings) {
    set(&mut settings.width, self.width);
    set(&mut settings.tab_width, self.indent);
    set(&mut settings.tabs, self.tabs);
    set(&mut settings.eol, self.eol);
    set(&mut settings.single_quote, self.single_quote);
    set(&mut settings.ignore, self.ignore.clone());
    set(&mut settings.format_embedded, self.format_embedded);
  }

  pub fn xml(&self, settings: &mut xml::Settings) {
    set(&mut settings.width, self.width);
    set(&mut settings.tab_width, self.indent);
    set(&mut settings.tabs, self.tabs);
    set(&mut settings.eol, self.eol);
    set(&mut settings.whitespace, self.whitespace);
    set(&mut settings.bracket_same_line, self.bracket_same_line);
    set(&mut settings.single_attribute_per_line, self.single_attribute_per_line);
    set(&mut settings.ignore, self.ignore.clone());
    set(&mut settings.format_embedded, self.format_embedded);
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
  settings: &Settings,
  path: Option<&str>,
  embed: &mut dyn Embed,
) -> Result<String, Error> {
  Formatter::default().format(text, language, settings, path, embed).map(str::to_owned)
}

/// Formats one file after another, in any language, reusing the buffers of each.
#[derive(Default)]
pub struct Formatter {
  html: html::Formatter,
  handlebars: handlebars::Formatter,
  css: css::Formatter,
  xml: xml::Formatter,
}

impl Formatter {
  /// Formats `text` as `language`. `path` is the file's path.
  pub fn format(
    &mut self,
    text: &str,
    language: Language,
    settings: &Settings,
    path: Option<&str>,
    embed: &mut dyn Embed,
  ) -> Result<&str, Error> {
    let filepath = path.map(str::to_owned);
    match language {
      Language::Html | Language::Vue => {
        let settings = if language == Language::Vue { &settings.vue } else { &settings.html };
        let settings = html::Settings { filepath, ..settings.clone() };
        self.html.format(text, &settings, embed)
      }
      Language::Handlebars => {
        let settings = handlebars::Settings { filepath, ..settings.handlebars.clone() };
        self.handlebars.format(text, &settings, embed)
      }
      Language::Css(dialect) => {
        let settings = css::Settings { dialect, ..settings.css.clone() };
        self.css.format(text, &settings, embed)
      }
      Language::Xml => self.xml.format(text, &settings.xml, embed),
    }
  }
}
