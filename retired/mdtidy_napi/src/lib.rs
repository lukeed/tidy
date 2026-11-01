use std::str::FromStr;

use napi::bindgen_prelude::{FnArgs, Function};
use napi_derive::napi;

/// Called with a code block's language, text, and width; returns its formatted text.
type Callback<'a> = Function<'a, FnArgs<(String, String, u32)>, Option<String>>;

/// Options for `format` and `check`. A `preset` sets every choice; the other fields override it.
#[napi(object)]
#[derive(Default)]
pub struct FormatOptions {
  /// `prettier` (default) or `preserve`.
  #[napi(ts_type = "'prettier' | 'preserve'")]
  pub preset: Option<String>,
  /// The line width. Default: 80.
  pub width: Option<u32>,
  /// `preserve`, `always`, or `never`.
  #[napi(ts_type = "'preserve' | 'always' | 'never'")]
  pub wrap: Option<String>,
  /// `keep`, `dash`, `asterisk`, or `plus`.
  #[napi(ts_type = "'keep' | 'dash' | 'asterisk' | 'plus'")]
  pub bullet: Option<String>,
  /// `keep`, `underscore`, or `asterisk`.
  #[napi(ts_type = "'keep' | 'underscore' | 'asterisk'")]
  pub emphasis: Option<String>,
  /// `keep`, `asterisk`, or `underscore`.
  #[napi(ts_type = "'keep' | 'asterisk' | 'underscore'")]
  pub strong: Option<String>,
  /// `keep` or `auto`.
  #[napi(ts_type = "'keep' | 'auto'")]
  pub numbering: Option<String>,
  /// `keep`, `dash`, `asterisk`, or `underscore`.
  #[napi(ts_type = "'keep' | 'dash' | 'asterisk' | 'underscore'")]
  pub rule: Option<String>,
  /// `keep`, `backtick`, or `tilde`.
  #[napi(ts_type = "'keep' | 'backtick' | 'tilde'")]
  pub fence: Option<String>,
  /// `keep` or `atx`.
  #[napi(ts_type = "'keep' | 'atx'")]
  pub heading: Option<String>,
  /// The name of the ignore directive.
  pub ignore: Option<String>,
}

/// Format Markdown. `code` receives each code block with a language, and front matter, and
/// returns its formatted text or `undefined` to keep it as written.
///
/// ```js
/// import { format } from "mdtidy";
/// let output = format(input, { wrap: "always" }, (lang, text) => {
///   if (lang === "json") return JSON.stringify(JSON.parse(text), null, 2);
/// });
/// ```
#[napi(
  ts_args_type = "source: string, options?: FormatOptions, code?: (lang: string, text: string, width: number) => string | undefined"
)]
pub fn format(
  source: String,
  options: Option<FormatOptions>,
  code: Option<Callback<'_>>,
) -> napi::Result<String> {
  let options = resolve(options.unwrap_or_default())?;
  let Some(code) = code else {
    return Ok(mdtidy::format(&source, &options));
  };
  let mut failure = None;
  let output = mdtidy::format_with(&source, &options, |block| {
    if failure.is_some() {
      return None;
    }
    let width = u32::try_from(block.width).unwrap_or(u32::MAX);
    match code.call((block.lang.to_string(), block.text.to_string(), width).into()) {
      Ok(text) => text,
      Err(error) => {
        failure = Some(error);
        None
      }
    }
  });
  match failure {
    Some(error) => Err(error),
    None => Ok(output),
  }
}

/// Whether Markdown is already formatted.
///
/// ```js
/// import { check } from "mdtidy";
/// if (!check(input)) process.exitCode = 1;
/// ```
#[napi(ts_args_type = "source: string, options?: FormatOptions")]
pub fn check(source: String, options: Option<FormatOptions>) -> napi::Result<bool> {
  let options = resolve(options.unwrap_or_default())?;
  Ok(mdtidy::check(&source, &options))
}

fn resolve(input: FormatOptions) -> napi::Result<mdtidy::Options> {
  let mut options =
    mdtidy::Options::from(parse::<mdtidy::Preset>(input.preset)?.unwrap_or_default());
  if let Some(width) = input.width {
    options.width = width as usize;
  }
  if let Some(wrap) = parse(input.wrap)? {
    options.wrap = wrap;
  }
  if let Some(bullet) = parse(input.bullet)? {
    options.bullet = bullet;
  }
  if let Some(emphasis) = parse(input.emphasis)? {
    options.emphasis = emphasis;
  }
  if let Some(strong) = parse(input.strong)? {
    options.strong = strong;
  }
  if let Some(numbering) = parse(input.numbering)? {
    options.numbering = numbering;
  }
  if let Some(rule) = parse(input.rule)? {
    options.rule = rule;
  }
  if let Some(fence) = parse(input.fence)? {
    options.fence = fence;
  }
  if let Some(heading) = parse(input.heading)? {
    options.heading = heading;
  }
  if let Some(ignore) = input.ignore {
    options.ignore = ignore;
  }
  Ok(options)
}

fn parse<T: FromStr<Err = String>>(value: Option<String>) -> napi::Result<Option<T>> {
  value.map(|value| T::from_str(&value)).transpose().map_err(napi::Error::from_reason)
}
