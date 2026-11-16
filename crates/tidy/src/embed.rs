//! Hooks that format the code of other languages in a template: scripts, style sheets, expressions.

/// What a piece of embedded code is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  /// A whole script, style sheet or document: a `<script>`, a `<style>`, a block of a Vue file, a code block.
  Block,
  /// An expression, as in `{{ value }}` or `:value="..."`.
  Expression,
  /// The statements of an event handler, as in `@click="..."`.
  Event,
  /// The parameters of a slot, as in `v-slot="{ item }"`.
  Bindings,
  /// The left side of a loop, as in `v-for="(item, index) in list"`.
  For,
  /// The type parameters of a component, as in `<script setup generic="T">`.
  Generic,
  /// A structural directive's microsyntax, as in `*ngFor="..."`.
  Directive,
  /// The front matter of a file.
  FrontMatter,
}

/// Where tidy's printers find a piece of code, before its parser tells what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hole {
  Block,
  Expression,
  Attribute,
  FrontMatter,
}

/// A piece of code for the host to format.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
  pub code: &'a str,
  /// The language of the code, by the name `--lang` takes: `js`, `ts`, `css`, `scss`, `json`, `yaml`, `md`, ...
  pub lang: &'a str,
  pub kind: Kind,
  /// The framework whose rules the code follows, where they differ from the language's own: `vue`, whose
  /// expressions may hold filters, or `ng`, for Angular's directives.
  pub host: Option<&'static str>,
  /// The code is the value of an attribute, as in `style="..."` or `@click="..."`, where a host may prefer single
  /// quotes.
  pub attribute: bool,
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

impl<'a> Request<'a> {
  /// The request for `code`, which tidy's printers name by Prettier's parser, found in `hole`, at no indent, with a
  /// width of 80, as a module.
  pub(crate) fn new(code: &'a str, parser: &'a str, hole: Hole) -> Request<'a> {
    let block = if hole == Hole::FrontMatter { Kind::FrontMatter } else { Kind::Block };
    let (lang, kind, host) = match parser {
      "babel" => ("js", block, None),
      "typescript" => ("ts", block, None),
      "markdown" => ("md", block, None),
      "glimmer" => ("hbs", block, None),
      "angular" => ("html", block, Some("ng")),
      "__js_expression" => ("js", Kind::Expression, None),
      "__ts_expression" => ("ts", Kind::Expression, None),
      "__vue_expression" => ("js", Kind::Expression, Some("vue")),
      "__vue_ts_expression" => ("ts", Kind::Expression, Some("vue")),
      "__vue_event_binding" => ("js", Kind::Event, Some("vue")),
      "__vue_ts_event_binding" => ("ts", Kind::Event, Some("vue")),
      "__vue_bindings" => ("js", Kind::Bindings, Some("vue")),
      "__vue_ts_bindings" => ("ts", Kind::Bindings, Some("vue")),
      "__vue_for_left" => ("js", Kind::For, Some("vue")),
      "__vue_ts_for_left" => ("ts", Kind::For, Some("vue")),
      "__vue_generic" => ("ts", Kind::Generic, Some("vue")),
      "__ng_directive" => ("js", Kind::Directive, Some("ng")),
      other => (other, block, None),
    };
    let attribute = hole == Hole::Attribute;
    Request {
      code,
      lang,
      kind,
      host,
      attribute,
      indent: 0,
      prefix: 0,
      suffix: 0,
      width: 80,
      module: true,
    }
  }

  /// Prettier's name for the parser of the code, which tidy's printers still go by: [`Request::new`] backwards.
  pub(crate) fn parser(&self) -> &'a str {
    let block = matches!(self.kind, Kind::Block | Kind::FrontMatter);
    match (self.lang, self.kind, self.host) {
      ("js", _, None) if block => "babel",
      ("ts", _, None) if block => "typescript",
      ("md", _, None) if block => "markdown",
      ("hbs", _, None) if block => "glimmer",
      ("html", _, Some("ng")) if block => "angular",
      ("js", Kind::Expression, None) => "__js_expression",
      ("ts", Kind::Expression, None) => "__ts_expression",
      ("js", Kind::Expression, Some("vue")) => "__vue_expression",
      ("ts", Kind::Expression, Some("vue")) => "__vue_ts_expression",
      ("js", Kind::Event, Some("vue")) => "__vue_event_binding",
      ("ts", Kind::Event, Some("vue")) => "__vue_ts_event_binding",
      ("js", Kind::Bindings, Some("vue")) => "__vue_bindings",
      ("ts", Kind::Bindings, Some("vue")) => "__vue_ts_bindings",
      ("js", Kind::For, Some("vue")) => "__vue_for_left",
      ("ts", Kind::For, Some("vue")) => "__vue_ts_for_left",
      (_, Kind::Generic, Some("vue")) => "__vue_generic",
      (_, Kind::Directive, Some("ng")) => "__ng_directive",
      (lang, ..) => lang,
    }
  }
}

/// Formats the code of another language. tidy asks it first; on `None`, tidy formats the code itself if it knows the
/// language and formats embedded code, and leaves it as written if not.
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
  #[cfg(feature = "js")]
  let scripts = &crate::js::Options {
    sort_imports: crate::js::sort_imports::in_markdown(scripts.sort_imports.as_ref()),
    ..scripts.clone()
  };
  builtin_in(request, tab_width, tabs, scripts, Host::Markdown)
}

/// Formats the code of a Markdown code block, as Prettier's Markdown does: with `embed`, then with tidy's own
/// formatters.
pub(crate) fn code_block(
  code: crate::markdown::Code<'_>,
  css: &crate::css::Options,
  tab_width: usize,
  tabs: bool,
  scripts: &Scripts,
  embed: &mut dyn Embed,
) -> Option<String> {
  let language = crate::languages::parser(code.lang).or(own(code.lang))?;
  let request = Request { width: code.width, ..Request::new(code.text, language, Hole::Block) };
  embed
    .format(request)
    .or_else(|| {
      let dialect = crate::css::Dialect::of_path(&format!("code.{language}"))?;
      let options = crate::css::Options { dialect, width: code.width, ..css.clone() };
      crate::css::format(code.text, &options, &mut *embed).ok()
    })
    .or_else(|| builtin_in_markdown(&request, tab_width, tabs, scripts))
}

/// Formats Markdown in markup, and the code blocks in it with tidy's own formatters.
fn markdown(request: &Request<'_>, tab_width: usize, tabs: bool, scripts: &Scripts) -> String {
  let options =
    crate::markdown::Options { width: request.width, ..crate::markdown::Options::default() };
  let css = crate::css::Options { tab_width, tabs, ..crate::css::Options::default() };
  crate::markdown::format_with(request.code, &options, |code| {
    code_block(code, &css, tab_width, tabs, scripts, &mut Verbatim)
  })
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

/// The markup parser of a code block's language.
fn markup(language: &str) -> Option<crate::html::Parser> {
  use crate::html::Parser;
  Some(match language {
    "html" => Parser::Html,
    "vue" => Parser::Vue,
    "angular" => Parser::Angular,
    "lwc" => Parser::Lwc,
    "mjml" => Parser::Mjml,
    _ => return None,
  })
}

/// Formats a code block of markup, as Prettier's Markdown formats it.
fn markup_block(
  request: &Request<'_>,
  parser: crate::html::Parser,
  tab_width: usize,
  tabs: bool,
  scripts: &Scripts,
) -> Option<String> {
  #[cfg(feature = "js")]
  let js = {
    // oxfmt leaves Markdown to Prettier, where only a plugin sorts the imports of a Vue file.
    let vue = parser == crate::html::Parser::Vue;
    let sort_imports =
      scripts.sort_imports.clone().filter(|how| !vue || !how.is_applied_by_format());
    crate::js::Options { sort_imports, ..scripts.clone() }
  };
  #[cfg(not(feature = "js"))]
  let _ = scripts;
  let options = crate::html::Options {
    parser,
    width: request.width,
    tab_width,
    tabs,
    #[cfg(feature = "js")]
    js,
    ..crate::html::Options::default()
  };
  let formatted = crate::html::format(request.code, &options, &mut Verbatim).ok()?;
  Some(formatted.trim_end().to_owned())
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
  let parser = request.parser();
  #[cfg(feature = "js")]
  if crate::js::template::formats(parser) {
    let options = script_options(request, tab_width, tabs, scripts);
    return crate::js::template::format(request, &options, host);
  }
  #[cfg(not(feature = "js"))]
  let _ = (scripts, host);
  if host == Host::Markdown
    && let Some(markup) = markup(parser)
  {
    return markup_block(request, markup, tab_width, tabs, scripts);
  }
  if parser == "markdown" && request.indent == 0 {
    return Some(markdown(request, tab_width, tabs, scripts).trim_end().to_owned());
  }
  let dialect = match parser {
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
      let dialect = if parser == "ini" { ini::Dialect::Ini } else { ini::Dialect::Git };
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
