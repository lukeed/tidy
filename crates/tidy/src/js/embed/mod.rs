pub(crate) mod css;
pub(crate) mod doc;
pub(crate) mod graphql;
pub(crate) mod html;
pub(crate) mod markdown;

use crate::js::options::{
  ArrowParentheses, AttributePosition, Expand, FormatOptions, HtmlWhitespaceSensitivity,
  IndentStyle, OperatorPosition, ProseWrap, QuoteProperties, QuoteStyle, Semicolons,
  TrailingCommas,
};

/// The options of the scripts in code that is embedded in a script, as `options` formats that script.
pub(crate) fn scripts(options: &FormatOptions) -> crate::js::Options {
  crate::js::Options {
    width: options.line_width.value() as usize,
    tab_width: options.indent_width.value() as usize,
    tabs: options.indent_style == IndentStyle::Tab,
    semi: options.semicolons == Semicolons::Always,
    single_quote: options.quote_style == QuoteStyle::Single,
    jsx_single_quote: options.jsx_quote_style == QuoteStyle::Single,
    quote_props: match options.quote_properties {
      QuoteProperties::AsNeeded => crate::json::QuoteProps::AsNeeded,
      QuoteProperties::Consistent => crate::json::QuoteProps::Consistent,
      QuoteProperties::Preserve => crate::json::QuoteProps::Preserve,
    },
    trailing_comma: match options.trailing_commas {
      TrailingCommas::All => crate::js::TrailingComma::All,
      TrailingCommas::Es5 => crate::js::TrailingComma::Es5,
      TrailingCommas::None => crate::js::TrailingComma::None,
    },
    bracket_spacing: options.bracket_spacing.value(),
    bracket_same_line: options.bracket_same_line.value(),
    arrow_parens: match options.arrow_parentheses {
      ArrowParentheses::Always => crate::js::ArrowParens::Always,
      ArrowParentheses::AsNeeded => crate::js::ArrowParens::Avoid,
    },
    object_wrap: match options.expand {
      Expand::Auto => crate::json::ObjectWrap::Preserve,
      Expand::Never => crate::json::ObjectWrap::Collapse,
    },
    experimental_ternaries: options.experimental_ternaries,
    experimental_operator_position: match options.experimental_operator_position {
      OperatorPosition::Start => crate::js::OperatorPosition::Start,
      OperatorPosition::End => crate::js::OperatorPosition::End,
    },
    jsdoc: options.jsdoc.is_some(),
    sort_imports: options.sort_imports.clone(),
    single_attribute_per_line: options.attribute_position == AttributePosition::Multiline,
    wrap: match options.prose_wrap {
      ProseWrap::Always => crate::Wrap::Always,
      ProseWrap::Never => crate::Wrap::Never,
      ProseWrap::Preserve => crate::Wrap::Preserve,
    },
    whitespace: match options.html_whitespace_sensitivity {
      HtmlWhitespaceSensitivity::Strict => crate::Whitespace::Strict,
      HtmlWhitespaceSensitivity::Css => crate::Whitespace::Auto,
      HtmlWhitespaceSensitivity::Ignore => crate::Whitespace::Ignore,
    },
    ..crate::js::Options::default()
  }
}
