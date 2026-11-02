//! Hooks that format the code of other languages in a template: scripts, style sheets, expressions.

/// What a piece of embedded code is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  /// The contents of a `<script>`, a `<style>`, or a block of a Vue file.
  Block,
  /// An expression in a template, as in `{{ value }}`.
  Expression,
  /// The value of an attribute that holds code, as in `style="..."` or `onclick="..."`.
  Attribute,
  /// The front matter of a file.
  FrontMatter,
}

/// A piece of code for the host to format.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
  pub code: &'a str,
  /// Prettier's name for the parser of the code: `babel`, `typescript`, `css`, `scss`, `json`, `yaml`, ...
  pub language: &'a str,
  pub kind: Kind,
  /// How many indents the code starts at. Lines after the first are indented by the host.
  pub indent: usize,
  /// The width of the text before the code on its first line, after the indents, as `name="` before an attribute
  /// value that the code follows on its line.
  pub prefix: usize,
  /// The width of the text after the code on its last line, as the `"` that closes such a value.
  pub suffix: usize,
  /// The line width.
  pub width: usize,
  /// JavaScript that is a module, not a classic script: Prettier's `__babelSourceType`. Only a `<script>` in HTML is
  /// a classic script, unless its `type` makes it a module.
  pub module: bool,
}

/// Formats the code of another language. `None` leaves it as it is written.
pub trait Embed {
  fn format(&mut self, request: Request<'_>) -> Option<String>;
}

impl<F: FnMut(Request<'_>) -> Option<String>> Embed for F {
  fn format(&mut self, request: Request<'_>) -> Option<String> {
    self(request)
  }
}

/// Leaves all embedded code as it is written.
pub struct Verbatim;

impl Embed for Verbatim {
  fn format(&mut self, _: Request<'_>) -> Option<String> {
    None
  }
}

/// The parser name of a language that tidy formats and Prettier doesn't, so that it is never one of Prettier's: TOML,
/// Dockerfile and INI.
pub(crate) fn own(language: &str) -> Option<&'static str> {
  match language {
    "toml" => Some("toml"),
    "dockerfile" | "Dockerfile" | "docker" | "containerfile" | "Containerfile" => {
      Some("dockerfile")
    }
    "ini" | "dosini" | "cfg" | "cnf" | "properties" | "editorconfig" | "editor-config"
    | "npmrc" | "shellcheckrc" => Some("ini"),
    "gitconfig" | "gitmodules" => Some("gitconfig"),
    _ => None,
  }
}

/// The options of the JavaScript and TypeScript in other languages: [`crate::js::Options`] with the `js` feature.
#[cfg(feature = "js")]
pub(crate) type Scripts = crate::js::Options;
#[cfg(not(feature = "js"))]
pub(crate) type Scripts = ();

/// Formats `request` with tidy's own formatter for its language, as a hook would, for the languages that tidy formats as
/// text: JSON, YAML, TOML, Dockerfile, INI, GraphQL, and with the `js` feature, JavaScript and TypeScript, with
/// `scripts` but the host's width and indent. `None` if it has none, or the code doesn't parse. TOML, Dockerfile and
/// INI print at the start of a line only, since indenting their lines would change multi-line strings, heredocs and
/// values over lines.
pub(crate) fn builtin(
  request: &Request<'_>,
  tab_width: usize,
  tabs: bool,
  scripts: &Scripts,
) -> Option<String> {
  builtin_in(request, tab_width, tabs, scripts, Host::Markup)
}

/// [`builtin`] for a Markdown code block, where Prettier prints a statement that is only JSX without a semicolon.
pub(crate) fn builtin_in_markdown(
  request: &Request<'_>,
  tab_width: usize,
  tabs: bool,
  scripts: &Scripts,
) -> Option<String> {
  builtin_in(request, tab_width, tabs, scripts, Host::Markdown)
}

/// [`builtin`] for `v-for="left operator right"`, where `left` asks for the aliases: the whole value, formatted as one.
pub(crate) fn builtin_for(
  left: &Request<'_>,
  operator: &str,
  right: &str,
  tab_width: usize,
  tabs: bool,
  scripts: &Scripts,
) -> Option<String> {
  #[cfg(feature = "js")]
  {
    let options = script_options(left, tab_width, tabs, scripts);
    crate::js::template::format_for(left, operator, right, &options)
  }
  #[cfg(not(feature = "js"))]
  {
    let _ = (left, operator, right, tab_width, tabs, scripts);
    None
  }
}

/// `scripts` with the host's width and indent.
#[cfg(feature = "js")]
fn script_options(
  request: &Request<'_>,
  tab_width: usize,
  tabs: bool,
  scripts: &Scripts,
) -> crate::js::Options {
  let eol = crate::doc::Eol::Lf;
  crate::js::Options { width: request.width, tab_width, tabs, eol, ..scripts.clone() }
}

/// Where tidy's own formatter prints the code that it formats as text.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Host {
  Markup,
  /// A code block, where Prettier's `parentParser` is `markdown`.
  Markdown,
  /// Markup in a JavaScript template, which indents the code's lines itself: a line break that is part of a text of
  /// JavaScript is `\r\n`, to tell it apart from the others.
  Template,
}

/// [`builtin`] for code that `host` prints.
pub(crate) fn builtin_in(
  request: &Request<'_>,
  tab_width: usize,
  tabs: bool,
  scripts: &Scripts,
  host: Host,
) -> Option<String> {
  use crate::{dockerfile, graphql, ini, json, toml, yaml};
  #[cfg(feature = "js")]
  if crate::js::template::formats(request.language) {
    let options = script_options(request, tab_width, tabs, scripts);
    return crate::js::template::format(request, &options, host);
  }
  #[cfg(not(feature = "js"))]
  let _ = (scripts, host);
  let dialect = match request.language {
    "json" => json::Dialect::Json,
    "jsonc" => json::Dialect::Jsonc,
    "json5" => json::Dialect::Json5,
    "json-stringify" => json::Dialect::Stringify,
    "yaml" => {
      let options =
        yaml::Options { width: request.width, tab_width, tabs, ..yaml::Options::default() };
      let mut formatter = yaml::Formatter::default();
      return formatter.embedded(request.code, &options, request.indent).map(str::to_owned);
    }
    "toml" if request.indent == 0 => {
      let options =
        toml::Options { column_width: request.width, tab_width, tabs, ..toml::Options::default() };
      let formatted = toml::format(request.code, &options).ok()?;
      return Some(formatted.trim_end().to_owned());
    }
    "dockerfile" if request.indent == 0 => {
      let options = dockerfile::Options { width: request.width, ..dockerfile::Options::default() };
      let formatted = dockerfile::format(request.code, &options).ok()?;
      return Some(formatted.trim_end().to_owned());
    }
    "graphql" => {
      let options =
        graphql::Options { width: request.width, tab_width, tabs, ..graphql::Options::default() };
      return graphql::Formatter::default().embedded(request.code, &options).map(str::to_owned);
    }
    "ini" | "gitconfig" if request.indent == 0 => {
      let dialect = if request.language == "ini" { ini::Dialect::Ini } else { ini::Dialect::Git };
      let options = ini::Options { dialect, tab_width, tabs, ..ini::Options::default() };
      let formatted = ini::format(request.code, &options).ok()?;
      return Some(formatted.trim_end().to_owned());
    }
    _ => return None,
  };
  let options =
    json::Options { dialect, width: request.width, tab_width, tabs, ..json::Options::default() };
  json::Formatter::default().embedded(request.code, &options, request.indent).map(str::to_owned)
}
