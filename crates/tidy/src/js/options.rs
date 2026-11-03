//! Prettier's options.
//!
//! | Prettier | here |
//! | --- | --- |
//! | `printWidth` | `line_width` |
//! | `tabWidth` | `indent_width` |
//! | `useTabs` | `indent_style` |
//! | `endOfLine` | `line_ending` |
//! | `semi` | `semicolons` |
//! | `singleQuote` | `quote_style` |
//! | `jsxSingleQuote` | `jsx_quote_style` |
//! | `quoteProps` | `quote_properties` |
//! | `trailingComma` | `trailing_commas` |
//! | `bracketSpacing` | `bracket_spacing` |
//! | `bracketSameLine` | `bracket_same_line` |
//! | `arrowParens` | `arrow_parentheses` |
//! | `objectWrap` | `expand` |
//! | `singleAttributePerLine` | `attribute_position` |
//! | `experimentalOperatorPosition` | `experimental_operator_position` |
//! | `experimentalTernaries` | `experimental_ternaries` |
//! | `embeddedLanguageFormatting` | `embedded_language_formatting` |

use crate::js::ir::prelude::*;

#[derive(Debug, Default, Clone)]
pub struct FormatOptions {
  pub indent_style: IndentStyle,
  pub indent_width: IndentWidth,
  pub line_ending: LineEnding,
  pub line_width: LineWidth,
  pub quote_style: QuoteStyle,
  pub jsx_quote_style: QuoteStyle,
  pub quote_properties: QuoteProperties,
  pub trailing_commas: TrailingCommas,
  pub semicolons: Semicolons,
  pub arrow_parentheses: ArrowParentheses,
  pub bracket_spacing: BracketSpacing,
  pub bracket_same_line: BracketSameLine,
  pub attribute_position: AttributePosition,
  pub expand: Expand,
  pub experimental_operator_position: OperatorPosition,
  pub experimental_ternaries: bool,
  pub embedded_language_formatting: EmbeddedLanguageFormatting,
  /// For YAML and Markdown.
  pub prose_wrap: ProseWrap,
  /// For HTML and similar languages.
  pub html_whitespace_sensitivity: HtmlWhitespaceSensitivity,
  pub vue_indent_script_and_style: bool,
  /// The file name, if it isn't the one the file was parsed under: text from stdin.
  pub filepath: Option<Box<[u8]>>,
  /// The text is code in Markdown: Prettier's `parentParser`. A statement that's only JSX gets no semicolon. A line
  /// break that's part of a text (in a template, in a comment) is written as `\r\n` to tell it apart from the others,
  /// because Markdown indents what follows it differently.
  pub is_in_markdown: bool,
  /// The text is `<$>`, JSX from MDX, and `</$>`: Prettier's `rootMarker: "mdx"`. Only the part between the two is
  /// printed.
  pub is_mdx_jsx: bool,
  /// How many JavaScript templates that are printed as HTML the text is nested in.
  pub html_template_depth: u32,
  /// The text is in a file whose name ends with `.html` or `.htm`, where the HTML5 doctype is printed in lowercase.
  pub is_in_html_file: bool,
  /// The code's context, if it's in HTML.
  pub in_html: InHtml,
  /// Which tool's output to match where the two differ.
  pub flavor: Flavor,
  /// How to sort imports, if at all.
  pub sort_imports: Option<std::sync::Arc<crate::js::sort_imports::SortImports>>,
  /// oxfmt's `jsdoc`: how to format JSDoc comments, if at all.
  pub jsdoc: Option<JsdocOptions>,
  /// For tests: what to output instead of the formatted text.
  pub debug_comments: DebugComments,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum DebugComments {
  #[default]
  Off,
  /// Every comment and the node that owns it.
  Owners,
  /// The comments that no printer printed, which is a bug.
  Unprinted,
}

/// The context Prettier passes to the formatter for code in HTML: the options whose names start with `__`, and the
/// parsers for less than a whole program.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct InHtml {
  pub root: HtmlRoot,
  /// `__isInHtmlAttribute`: the code is between double quotes.
  pub is_in_attribute: bool,
  /// `__isInHtmlInterpolation`: `}}` follows the code.
  pub is_in_interpolation: bool,
  /// `__isHtmlInlineEventHandler`
  pub is_inline_event_handler: bool,
  /// `__isHTMLStyleAttribute`
  pub is_style_attribute: bool,
  /// `singleQuote`, which `quote_style` doesn't reflect in an attribute.
  pub quote_style: QuoteStyle,
  /// `singleQuote: true`, regardless of the options: what `prettier-plugin-svelte` asks for between double quotes.
  pub has_single_quotes: bool,
  /// The parser is `babel-ts`: TypeScript in Babel's tree.
  pub has_babel_tree: bool,
}

/// The kind of code.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum HtmlRoot {
  /// It isn't in HTML.
  #[default]
  None,
  /// `babel`, `typescript`, `css`, ..
  Program,
  /// `__js_expression`, `__ts_expression`
  JsExpression,
  /// `__vue_expression`, `__vue_ts_expression`: `a | b` is a filter.
  VueExpression,
  /// `__vue_event_binding`, `__vue_ts_event_binding`
  VueEventBinding,
  /// `__ng_action`
  #[expect(dead_code, reason = "tidy formats no Angular bindings yet")]
  NgAction,
  /// `__ng_binding`
  #[expect(dead_code, reason = "tidy formats no Angular bindings yet")]
  NgBinding,
  /// `__ng_directive`
  NgDirective,
  /// `__ng_interpolation`
  #[expect(dead_code, reason = "tidy formats no Angular bindings yet")]
  NgInterpolation,
  /// `svelteExpressionParser`, `svelteTSExpressionParser`: the root is a `File` whose `program` is an expression, or a function.
  SvelteExpression,
  /// `svelteStatementParser`, `svelteTSStatementParser`: the same with a statement, which loses its `;`.
  SvelteStatement,
}

impl HtmlRoot {
  /// Prettier's `NGRoot`: the code is an Angular expression, where `a | b(c)` represents the pipe `a | b: c`.
  #[inline]
  pub fn is_angular(self) -> bool {
    matches!(
      self,
      HtmlRoot::NgAction | HtmlRoot::NgBinding | HtmlRoot::NgDirective | HtmlRoot::NgInterpolation
    )
  }
}

/// oxfmt's `jsdoc`, which has prettier-plugin-jsdoc's options.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct JsdocOptions {
  /// Capitalize the first letter of a description.
  pub capitalize_descriptions: bool,
  pub comment_line_strategy: CommentLineStrategy,
  /// An empty line between tags of different kinds.
  pub separate_tag_groups: bool,
  /// An empty line between the last `@param` and `@returns`.
  pub separate_returns_from_param: bool,
  /// `{ string }` instead of `{string}`.
  pub bracket_spacing: bool,
  /// End a description with a dot.
  pub description_with_dot: bool,
  /// "Default is `value`" at the end of the description of `@param [name=value]`.
  pub add_default_to_description: bool,
  /// Put code without a language between fences instead of indenting it.
  pub prefer_code_fences: bool,
  pub line_wrapping_style: LineWrappingStyle,
  /// Put the description after `@description`.
  pub description_tag: bool,
  /// Keep the indentation of an `@example` that can't be parsed.
  pub keep_unparsable_example_indent: bool,
}

impl Default for JsdocOptions {
  fn default() -> Self {
    JsdocOptions {
      capitalize_descriptions: true,
      comment_line_strategy: CommentLineStrategy::SingleLine,
      separate_tag_groups: false,
      separate_returns_from_param: false,
      bracket_spacing: false,
      description_with_dot: false,
      add_default_to_description: true,
      prefer_code_fences: false,
      line_wrapping_style: LineWrappingStyle::Greedy,
      description_tag: false,
      keep_unparsable_example_indent: false,
    }
  }
}

/// When a JSDoc comment goes on one line.
#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
pub enum CommentLineStrategy {
  /// Whenever it can.
  #[default]
  SingleLine,
  /// Never.
  #[expect(dead_code, reason = "tidy's jsdoc option has no line strategy")]
  Multiline,
  /// If it already is.
  #[expect(dead_code, reason = "tidy's jsdoc option has no line strategy")]
  Keep,
}

/// How descriptions in JSDoc comments are wrapped.
#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
pub enum LineWrappingStyle {
  /// As much on each line as fits.
  #[default]
  Greedy,
  /// The lines stay as they are if they all fit.
  Balance,
}

/// Which whitespace in HTML is significant.
#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
pub enum HtmlWhitespaceSensitivity {
  /// Decided by the default value of the CSS property `display`.
  #[default]
  Css,
  /// All of it.
  Strict,
  /// None of it.
  Ignore,
}

/// How a paragraph is wrapped.
#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
pub enum ProseWrap {
  /// At the print width.
  Always,
  /// Not at all.
  Never,
  /// As in the source.
  #[default]
  Preserve,
}

/// oxfmt follows an older Prettier (3.8) in a few places, and has a few rules of its own. A project with an
/// `.oxfmtrc.json` gets no diff from switching.
#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
pub enum Flavor {
  #[default]
  Prettier,
  #[expect(dead_code, reason = "tidy follows Prettier; bun's printer still branches on it")]
  Oxfmt,
}

impl Flavor {
  pub const fn is_oxfmt(self) -> bool {
    matches!(self, Flavor::Oxfmt)
  }

  /// True if a comment that contains only `text` means that the code after it is left as is. oxfmt also recognizes
  /// Prettier's comment. Prettier only recognizes its own.
  pub fn is_ignore_comment(self, text: &[u8]) -> bool {
    text == b"prettier-ignore" || (text == b"oxfmt-ignore" && self.is_oxfmt())
  }
}

#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
pub enum IndentStyle {
  Tab,
  #[default]
  Space,
}

impl IndentStyle {
  pub const fn is_tab(self) -> bool {
    matches!(self, IndentStyle::Tab)
  }

  pub const fn is_space(self) -> bool {
    matches!(self, IndentStyle::Space)
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Default)]
pub enum LineEnding {
  #[default]
  Lf,
  Crlf,
  Cr,
  /// The line ending of the file's first line.
  Auto,
}

impl LineEnding {
  #[inline]
  pub const fn as_bytes(self) -> &'static [u8] {
    match self {
      LineEnding::Lf | LineEnding::Auto => b"\n",
      LineEnding::Crlf => b"\r\n",
      LineEnding::Cr => b"\r",
    }
  }

  /// Prettier's `guessEndOfLine`: the first `\r` decides, even if lines before it end in `\n`.
  pub(crate) fn resolve(self, text: &[u8]) -> LineEnding {
    if self != LineEnding::Auto {
      return self;
    }
    match crate::js::strings::index_of_char_usize(text, b'\r') {
      Some(at) if text.get(at + 1) == Some(&b'\n') => LineEnding::Crlf,
      Some(_) => LineEnding::Cr,
      None => LineEnding::Lf,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IndentWidth(pub u32);

impl IndentWidth {
  #[inline]
  pub const fn value(self) -> u32 {
    self.0
  }
}

impl Default for IndentWidth {
  fn default() -> Self {
    Self(2)
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LineWidth(pub u32);

impl LineWidth {
  #[inline]
  pub const fn value(self) -> u32 {
    self.0
  }
}

impl Default for LineWidth {
  fn default() -> Self {
    Self(80)
  }
}

#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
pub enum QuoteStyle {
  #[default]
  Double,
  Single,
}

impl QuoteStyle {
  pub fn from_byte(byte: u8) -> Option<Self> {
    match byte {
      b'"' => Some(Self::Double),
      b'\'' => Some(Self::Single),
      _ => None,
    }
  }

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Double => "\"",
      Self::Single => "'",
    }
  }

  pub const fn as_byte(self) -> u8 {
    match self {
      Self::Double => b'"',
      Self::Single => b'\'',
    }
  }

  #[must_use]
  pub const fn other(self) -> Self {
    match self {
      Self::Double => Self::Single,
      Self::Single => Self::Double,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum QuoteProperties {
  #[default]
  AsNeeded,
  Preserve,
  Consistent,
}

impl QuoteProperties {
  pub const fn is_consistent(self) -> bool {
    matches!(self, Self::Consistent)
  }
}

#[derive(Clone, Copy, Default, Debug, Eq, Hash, PartialEq)]
pub enum TrailingCommas {
  #[default]
  All,
  Es5,
  None,
}

impl TrailingCommas {
  pub const fn is_all(self) -> bool {
    matches!(self, TrailingCommas::All)
  }

  pub const fn is_none(self) -> bool {
    matches!(self, TrailingCommas::None)
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Semicolons {
  #[default]
  Always,
  AsNeeded,
}

impl Semicolons {
  pub const fn is_as_needed(self) -> bool {
    matches!(self, Self::AsNeeded)
  }

  pub const fn is_always(self) -> bool {
    matches!(self, Self::Always)
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ArrowParentheses {
  #[default]
  Always,
  AsNeeded,
}

impl ArrowParentheses {
  pub const fn is_as_needed(self) -> bool {
    matches!(self, Self::AsNeeded)
  }
}

/// Where a trailing comma is printed if `trailing_commas` allows it: anywhere, or only where ES5 allows one.
#[derive(Debug, Copy, Clone)]
pub enum FormatTrailingCommas {
  All,
  /// After a call's last argument and a function's last parameter: like `All`, but Angular doesn't accept one there.
  Arguments,
  ES5,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum TrailingSeparator {
  /// Printed if the enclosing group breaks.
  #[default]
  Allowed,
  /// A syntax error, after a rest element for example.
  Disallowed,
  Mandatory,
  /// Omitted because of the options.
  Omit,
}

impl FormatTrailingCommas {
  pub fn trailing_separator(self, options: &FormatOptions) -> TrailingSeparator {
    match self {
      _ if options.trailing_commas.is_none() => TrailingSeparator::Omit,
      FormatTrailingCommas::All | FormatTrailingCommas::Arguments
        if !options.trailing_commas.is_all() =>
      {
        TrailingSeparator::Omit
      }
      FormatTrailingCommas::Arguments if options.in_html.root.is_angular() => {
        TrailingSeparator::Omit
      }
      FormatTrailingCommas::All | FormatTrailingCommas::Arguments | FormatTrailingCommas::ES5 => {
        TrailingSeparator::Allowed
      }
    }
  }
}

impl<'a> Format<'a> for FormatTrailingCommas {
  fn fmt(&self, f: &mut Formatter<'a>) {
    if self.trailing_separator(f.options()) == TrailingSeparator::Allowed {
      if_group_breaks(&token(",")).fmt(f);
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum AttributePosition {
  #[default]
  Auto,
  Multiline,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Expand {
  #[default]
  Auto,
  Never,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BracketSpacing(pub bool);

impl BracketSpacing {
  #[inline]
  pub const fn value(self) -> bool {
    self.0
  }
}

impl Default for BracketSpacing {
  fn default() -> Self {
    Self(true)
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct BracketSameLine(pub bool);

impl BracketSameLine {
  #[inline]
  pub const fn value(self) -> bool {
    self.0
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum OperatorPosition {
  Start,
  #[default]
  End,
}

impl OperatorPosition {
  pub const fn is_end(self) -> bool {
    matches!(self, Self::End)
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum EmbeddedLanguageFormatting {
  #[default]
  Auto,
  Off,
}
