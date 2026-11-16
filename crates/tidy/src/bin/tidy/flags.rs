//! What the flags choose: the language of `--parser`, and the formatting options.

use tidy::{Language, Shared, Whitespace, css, doc, handlebars, ini, js, json};

/// The language a `--parser` name stands for, and the Handlebars dialect it implies.
pub(crate) fn parser(name: &str) -> Result<(Language, Option<handlebars::Dialect>), String> {
  Ok(match name {
    "html" => (Language::Html, None),
    "vue" => (Language::Vue, None),
    "mjml" => (Language::Mjml, None),
    "handlebars" => (Language::Handlebars, None),
    "tempura" => (Language::Handlebars, Some(handlebars::Dialect::Tempura)),
    "css" => (Language::Css(css::Dialect::Css), None),
    "scss" => (Language::Css(css::Dialect::Scss), None),
    "less" => (Language::Css(css::Dialect::Less), None),
    "json" => (Language::Json(json::Dialect::Json), None),
    "jsonc" => (Language::Json(json::Dialect::Jsonc), None),
    "json5" => (Language::Json(json::Dialect::Json5), None),
    "json-stringify" => (Language::Json(json::Dialect::Stringify), None),
    "toml" => (Language::Toml, None),
    "dockerfile" => (Language::Dockerfile, None),
    "ini" => (Language::Ini(ini::Dialect::Ini), None),
    "gitconfig" => (Language::Ini(ini::Dialect::Git), None),
    "graphql" => (Language::Graphql, None),
    "babel" => (Language::Js(js::Dialect::Js), None),
    "typescript" | "babel-ts" => (Language::Js(js::Dialect::Ts), None),
    "yaml" => (Language::Yaml, None),
    "xml" => (Language::Xml, None),
    "markdown" => (Language::Markdown, None),
    other => return Err(format!("unknown parser `{other}`")),
  })
}

/// The shared choices that the flags make.
pub(crate) fn shared(args: &mut pico_args::Arguments) -> Result<Shared, String> {
  let error = |error: pico_args::Error| error.to_string();
  let mut choice = |name: &'static str, options: &[&str]| -> Result<Option<String>, String> {
    let value: Option<String> = args.opt_value_from_str(name).map_err(error)?;
    match value {
      Some(value) if !options.contains(&value.as_str()) => {
        Err(format!("unknown {name} `{value}`, expected one of: {}", options.join(", ")))
      }
      value => Ok(value),
    }
  };
  let eol = choice("--eol", &["lf", "crlf", "cr"])?;
  let whitespace = choice("--whitespace", &["strict", "auto", "ignore"])?;
  let quote = choice("--quote", &["double", "single"])?;
  let flag =
    |args: &mut pico_args::Arguments, name: &'static str| args.contains(name).then_some(true);
  Ok(Shared {
    width: args.opt_value_from_str("--width").map_err(error)?,
    indent: args.opt_value_from_str("--indent").map_err(error)?,
    tabs: flag(args, "--tabs"),
    eol: eol.map(|eol| match eol.as_str() {
      "crlf" => doc::Eol::Crlf,
      "cr" => doc::Eol::Cr,
      _ => doc::Eol::Lf,
    }),
    whitespace: whitespace.map(|how| match how.as_str() {
      "strict" => Whitespace::Strict,
      "ignore" => Whitespace::Ignore,
      _ => Whitespace::Auto,
    }),
    single_quote: quote.map(|quote| quote == "single"),
    trailing_comma: args.contains("--no-trailing-comma").then_some(false),
    bracket_spacing: args.contains("--no-bracket-spacing").then_some(false),
    bracket_same_line: flag(args, "--bracket-same-line"),
    single_attribute_per_line: flag(args, "--single-attribute-per-line"),
    ignore: args.opt_value_from_str("--ignore").map_err(error)?,
    format_embedded: args.contains("--no-embedded").then_some(false),
  })
}
