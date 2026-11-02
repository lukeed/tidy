//! Formats code and types embedded in comments, each in its own language.

use super::strings;
use super::text::trim_end_matches;
use crate::js::options::{
  EmbeddedLanguageFormatting, Expand, FormatOptions, IndentStyle, LineEnding, LineWidth, ProseWrap,
  QuoteStyle, TrailingCommas,
};

/// oxfmt's maximum line width.
const MAX_LINE_WIDTH: usize = 320;

/// Returns the options for content embedded in a comment of a file that's formatted with `options`.
pub(super) fn embedded_options(options: &FormatOptions, print_width: usize) -> FormatOptions {
  // oxfmt stores it in 16 bits.
  let width = u16::try_from(print_width).map_or(80, usize::from).clamp(1, MAX_LINE_WIDTH);
  FormatOptions {
    line_width: LineWidth(width as u32),
    // The lines are written to the file's document one by one.
    line_ending: LineEnding::Lf,
    jsdoc: None,
    sort_imports: None,
    filepath: None,
    is_in_markdown: false,
    ..options.clone()
  }
}

/// Formats `code` as if it were the file at `path`, and trims trailing whitespace. Returns `None` on a syntax error.
fn parse_and_build(path: &[u8], code: &[u8], options: &FormatOptions) -> Option<Vec<u8>> {
  let text = std::str::from_utf8(code).ok()?;
  let name = std::str::from_utf8(path).ok()?;
  let dialect = match name.ends_with(".tsx") {
    true => crate::js::Dialect::Ts,
    false => crate::js::Dialect::Js,
  };
  let allocator = oxc_allocator::Allocator::default();
  let types = crate::js::source_types(dialect, name, text);
  let parsed = crate::js::parse_any(&allocator, text, types).ok()?;
  let source_type = parsed.program.source_type;
  let program = allocator.alloc(parsed.program);
  let file = crate::js::ast::File::new(text, path, source_type, program, &allocator);
  let mut out = Vec::new();
  let mut scratch = crate::js::Scratch::default();
  crate::js::ir::run::format(&file, options, &mut scratch, &mut out, crate::js::format_file)
    .ok()?;
  out.truncate(strings::trim_unicode_whitespace_end(&out).len());
  Some(out)
}

const TSX: &[u8] = b"dummy.tsx";
const JSX: &[u8] = b"dummy.jsx";

pub(super) fn is_js_ts_lang(lang: &[u8]) -> bool {
  [&b"js"[..], b"javascript", b"jsx", b"ts", b"typescript", b"tsx"]
    .iter()
    .any(|name| lang.eq_ignore_ascii_case(name))
}

/// Formats code in a description, for the languages other than JavaScript and TypeScript that prettier-plugin-jsdoc
/// formats. `None` means the code stays as it is.
pub(super) fn format_embedded_language(
  lang: &[u8],
  code: &[u8],
  print_width: usize,
  options: &FormatOptions,
) -> Option<Vec<u8>> {
  if matches!(options.embedded_language_formatting, EmbeddedLanguageFormatting::Off) {
    return None;
  }
  let text = std::str::from_utf8(code).ok()?;
  let tabs = options.indent_style == IndentStyle::Tab;
  let tab_width = options.indent_width.value() as usize;
  let trailing_comma = options.trailing_commas != TrailingCommas::None;
  let css = |dialect| {
    let css = crate::css::Options {
      dialect,
      width: print_width,
      tab_width,
      tabs,
      single_quote: options.quote_style == QuoteStyle::Single,
      trailing_comma,
      ..Default::default()
    };
    crate::css::format(text, &css, &mut crate::embed::Verbatim)
  };
  let out = match &lang.to_ascii_lowercase()[..] {
    b"css" => css(crate::css::Dialect::Css),
    b"less" => css(crate::css::Dialect::Less),
    b"scss" => css(crate::css::Dialect::Scss),
    b"json" => {
      let json = crate::json::Options {
        width: print_width,
        tab_width,
        tabs,
        bracket_spacing: options.bracket_spacing.value(),
        object_wrap: match options.expand {
          Expand::Never => crate::json::ObjectWrap::Collapse,
          _ => crate::json::ObjectWrap::Preserve,
        },
        ..Default::default()
      };
      crate::json::format(text, &json)
    }
    b"yaml" => {
      let yaml = crate::yaml::Options {
        width: print_width,
        tab_width,
        tabs,
        single_quote: options.quote_style == QuoteStyle::Single,
        bracket_spacing: options.bracket_spacing.value(),
        trailing_comma,
        wrap: match options.prose_wrap {
          ProseWrap::Always => crate::Wrap::Always,
          ProseWrap::Never => crate::Wrap::Never,
          ProseWrap::Preserve => crate::Wrap::Preserve,
        },
        ..Default::default()
      };
      crate::yaml::format(text, &yaml)
    }
    _ => return None,
  }
  .ok()?
  .into_bytes();
  let mut out = out;
  out.truncate(strings::trim_unicode_whitespace_end(&out).len());
  Some(out)
}

/// Returns the template literal nesting depth at the end of `line`, given `depth` at its start.
pub(super) fn update_template_depth(line: &[u8], mut depth: u32) -> u32 {
  // For each open `${`, the number of open braces inside it.
  let mut expression_brace_depth: smallvec::SmallVec<[u32; 4]> = smallvec::SmallVec::new();
  let mut i = 0;
  while i < line.len() {
    match line[i] {
      b'\\' => i += 1,
      b'`' if expression_brace_depth.is_empty() => {
        depth = u32::from(depth == 0) + depth.saturating_sub(1)
      }
      b'`' => expression_brace_depth.push(0),
      b'$' if depth > 0 && line.get(i + 1) == Some(&b'{') => {
        expression_brace_depth.push(0);
        i += 1;
      }
      b'{' => {
        if let Some(last) = expression_brace_depth.last_mut() {
          *last += 1;
        }
      }
      b'}' => match expression_brace_depth.last_mut() {
        Some(0) => {
          expression_brace_depth.pop();
        }
        Some(last) => *last -= 1,
        None => {}
      },
      _ => {}
    }
    i += 1;
  }
  depth
}

/// Formats `code` as JavaScript or TypeScript with lines of at most `print_width` columns. Returns `None` if it parses
/// as neither.
pub(super) fn format_embedded_js(
  code: &[u8],
  print_width: usize,
  options: &FormatOptions,
) -> Option<Vec<u8>> {
  let base_options = embedded_options(options, print_width);
  let trimmed = strings::trim_unicode_whitespace(code);
  if !trimmed.starts_with(b"{") {
    // Try TSX first, because `getItem<number>(..)` means something else in JSX.
    return parse_and_build(TSX, code, &base_options)
      .or_else(|| parse_and_build(JSX, code, &base_options));
  }
  // An object literal, which would parse as a block with labels. prettier-plugin-jsdoc formats it as JSON, which
  // keeps quoted keys, so an object with quotes is left as is.
  let has_quoted_keys = strings::index_of_char_usize(trimmed, b'"').is_some_and(|quote| {
    strings::index_of_char_usize(trimmed, b'}').is_some_and(|brace| quote < brace)
  });
  if has_quoted_keys {
    return None;
  }
  let wrapped = [b"(", trimmed, b")"].concat();
  let object_options = FormatOptions { trailing_commas: TrailingCommas::None, ..base_options };
  let format_object = |path: &[u8]| -> Option<Vec<u8>> {
    let formatted = parse_and_build(path, &wrapped, &object_options)?;
    let Some(inner) = formatted.strip_prefix(b"(").and_then(|inner| inner.strip_suffix(b");"))
    else {
      return Some(formatted);
    };
    // Something follows the object, so this is a call, and the parentheses aren't the ones we added.
    strings::trim_unicode_whitespace(inner).ends_with(b"}").then(|| inner.to_vec())
  };
  format_object(JSX).or_else(|| format_object(TSX))
}

/// prettier-plugin-jsdoc's `formatType`: formats the type as the right side of `type __t = ..;`. Returns `None` if that
/// fails or changes nothing. `options` come from [`embedded_options`].
pub(super) fn format_type_via_formatter(
  type_str: &[u8],
  options: &FormatOptions,
) -> Option<Vec<u8>> {
  if type_str.is_empty() {
    return None;
  }
  // `...Type` is formatted as `(Type)[]`.
  if let Some(rest) = type_str.strip_prefix(b"...") {
    let rest = strings::trim_unicode_whitespace_start(rest);
    if rest.is_empty() {
      return None;
    }
    let formatted = format_type_via_formatter(&[b"(", rest, b")[]"].concat(), options)?;
    return Some([b"...", formatted.strip_suffix(b"[]")?].concat());
  }
  if !needs_formatter_pass(type_str) {
    return None;
  }
  const START: &[u8] = b"type __t = ";
  let formatted = parse_and_build(TSX, &[START, type_str, b";"].concat(), options)?;
  let result = strings::trim_unicode_whitespace_start(formatted.get(START.len()..)?);
  let result = trim_end_matches(result, |c| c == ';' || c == '\n');
  let result = strings::trim_unicode_whitespace(result.strip_prefix(b"|").unwrap_or(result));
  (!result.is_empty() && result != type_str).then(|| result.to_vec())
}

/// True if the type contains anything the formatter could change.
fn needs_formatter_pass(type_str: &[u8]) -> bool {
  strings::index_of_any(type_str, b"|&{}()\n").is_some() || strings::contains(type_str, b"=>")
}
