//! JavaScript and TypeScript, with a port of bun's formatter onto oxc's parser. See `readme.md`.

pub(crate) mod ast;
pub(crate) mod ast_nodes;
pub(crate) mod attach;
pub(crate) mod builders;
pub(crate) mod comments;
pub(crate) mod context;
pub(crate) mod embed;
pub(crate) mod fields;
pub(crate) mod format;
pub(crate) mod ir;
pub(crate) mod jsdoc;
pub(crate) mod options;
pub(crate) mod owners;
pub(crate) mod parentheses;
pub(crate) mod print;
pub(crate) mod sort;
pub(crate) mod sort_imports;
pub(crate) mod source_text;
pub(crate) mod stack;
pub(crate) mod strings;
pub(crate) mod template;
pub(crate) mod text;
pub(crate) mod trivia;
pub(crate) mod utils;

pub(crate) use ir::run::Scratch;
pub(crate) use options::FormatOptions;
pub use sort_imports::{Settings as SortSettings, SortImports};

use crate::doc::Eol;
use crate::json::{ObjectWrap, QuoteProps};

// Not in the prelude: a glob import can't shadow the `std` macros with the same names.
pub(crate) use ir::macros::{best_fitting, format_args, write};

/// Imports that nearly every file of the printer needs.
pub(crate) mod base {
  pub(crate) use super::ast::*;
  pub(crate) use super::ast::{Span, Spanned};
  pub(crate) use super::ir::prelude::*;
  pub(crate) use super::options::*;
  pub(crate) use super::prelude::*;
}

pub(crate) mod prelude {
  pub(crate) use super::ast_nodes::{AsAstNodes, AstNodes, ChainElement};
  pub(crate) use super::builders::FormatSeparatedIter;
  pub(crate) use super::comments::{CommentExt, CommentList, Comments};
  pub(crate) use super::fields::{ExprFields, StmtFields};
  pub(crate) use super::source_text::SourceText;
  pub(crate) use super::trivia::{
    DanglingIndentMode, FormatDanglingComments, FormatLeadingComments, FormatTrailingComments,
    dangling_comments, leading_comments, trailing_comments,
  };
  pub(crate) use super::utils::operators::{BinOpExt, Precedence, UnOpExt};
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum FormatError {
  /// The file has syntax errors and is left as is.
  SyntaxError,
  /// The syntax is nested so deeply that formatting it would overflow the stack.
  NestedTooDeeply,
  /// The formatted text is [longer than a string can be](is_too_long).
  TooLong,
  /// A bug in the formatter: the document it wrote is malformed.
  InvalidDocument,
}

/// The longest string that V8 has, which is where Prettier fails with `RangeError: Invalid string length`.
const MAX_STRING_LENGTH: u64 = (1 << 29) - 24;

/// True if a text of `len` bytes is longer than a JavaScript string can be. Something nested `n` deep is indented by
/// `n * n` spaces.
pub(crate) fn is_too_long(len: usize) -> bool {
  len as u64 > MAX_STRING_LENGTH
}

/// JavaScript or TypeScript: what Prettier's `babel` and `typescript` parsers read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dialect {
  #[default]
  Js,
  Ts,
}

/// The names that Prettier formats as JavaScript.
const NAMES: [&str; 3] = ["jakefile", "start.frag", "end.frag"];

/// The extensions of Prettier's JavaScript and JSX languages, but Flow's `.js.flow`.
const JS: [&str; 27] = [
  ".js",
  "._js",
  ".bones",
  ".cjs",
  ".es",
  ".es6",
  ".gs",
  ".jake",
  ".javascript",
  ".jsb",
  ".jscad",
  ".jsfl",
  ".jslib",
  ".jsm",
  ".jspre",
  ".jss",
  ".mjs",
  ".njs",
  ".pac",
  ".sjs",
  ".ssjs",
  ".xsjs",
  ".xsjslib",
  ".start.frag",
  ".end.frag",
  ".wxs",
  ".jsx",
];

/// The extensions of Prettier's TypeScript and TSX languages.
const TS: [&str; 4] = [".ts", ".cts", ".mts", ".tsx"];

impl Dialect {
  /// The dialect of the file at `path`, by its name, as Prettier infers its parser, or `None` if it isn't a script.
  pub fn of_path(path: &str) -> Option<Dialect> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let lower = name.to_ascii_lowercase();
    if TS.iter().any(|extension| lower.ends_with(extension)) {
      return Some(Dialect::Ts);
    }
    let js =
      NAMES.contains(&lower.as_str()) || JS.iter().any(|extension| lower.ends_with(extension));
    js.then_some(Dialect::Js)
  }
}

/// `trailingComma`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrailingComma {
  /// Wherever the syntax allows one.
  #[default]
  All,
  /// Where ES5 allows one: in arrays and objects.
  Es5,
  None,
}

/// `arrowParens`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArrowParens {
  /// `(x) => x`
  #[default]
  Always,
  /// `x => x`
  Avoid,
}

/// `experimentalOperatorPosition`: where a binary expression that breaks puts its operators.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OperatorPosition {
  /// At the start of the next line.
  Start,
  /// At the end of the line.
  #[default]
  End,
}

/// Options of the JavaScript and TypeScript formatter, Prettier's by default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
  pub dialect: Dialect,
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: Eol,
  /// `semi`: a semicolon at the end of each statement.
  pub semi: bool,
  pub single_quote: bool,
  /// `jsxSingleQuote`: single quotes in JSX attributes.
  pub jsx_single_quote: bool,
  pub quote_props: QuoteProps,
  pub trailing_comma: TrailingComma,
  /// `bracketSpacing`: `{ a }` rather than `{a}`.
  pub bracket_spacing: bool,
  /// `bracketSameLine`: the `>` of a multi-line JSX element on its last line.
  pub bracket_same_line: bool,
  /// `singleAttributePerLine`: each attribute of a multi-line JSX element on its own line.
  pub single_attribute_per_line: bool,
  pub arrow_parens: ArrowParens,
  pub object_wrap: ObjectWrap,
  pub experimental_ternaries: bool,
  pub experimental_operator_position: OperatorPosition,
  /// `embeddedLanguageFormatting`: format the code in templates, such as `css` and `graphql` tags.
  pub format_embedded: bool,
  /// Format JSDoc comments as prettier-plugin-jsdoc does.
  pub jsdoc: bool,
  /// `proseWrap` of the Markdown in templates. [`crate::Formatter`] takes it from the Markdown options.
  pub wrap: crate::Wrap,
  /// `htmlWhitespaceSensitivity` of the HTML in templates. [`crate::Formatter`] takes it from the HTML options.
  pub whitespace: crate::Whitespace,
  /// How to sort imports, if at all: as oxfmt's or bun's `sortImports`, or as `@trivago/prettier-plugin-sort-imports`,
  /// `@ianvs/prettier-plugin-sort-imports` or `prettier-plugin-organize-imports`. See [`SortSettings`].
  pub sort_imports: Option<std::sync::Arc<SortImports>>,
}

impl Default for Options {
  fn default() -> Self {
    Options {
      dialect: Dialect::Js,
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: Eol::Lf,
      semi: true,
      single_quote: false,
      jsx_single_quote: false,
      quote_props: QuoteProps::AsNeeded,
      trailing_comma: TrailingComma::All,
      bracket_spacing: true,
      bracket_same_line: false,
      arrow_parens: ArrowParens::Always,
      object_wrap: ObjectWrap::Preserve,
      experimental_ternaries: false,
      experimental_operator_position: OperatorPosition::End,
      format_embedded: true,
      jsdoc: true,
      single_attribute_per_line: false,
      wrap: crate::Wrap::Preserve,
      whitespace: crate::Whitespace::Auto,
      sort_imports: None,
    }
  }
}

impl From<&Options> for FormatOptions {
  fn from(options: &Options) -> Self {
    use crate::js::options::{
      ArrowParentheses, BracketSameLine, BracketSpacing, EmbeddedLanguageFormatting, Expand,
      IndentStyle, IndentWidth, LineEnding, LineWidth, OperatorPosition as Position,
      QuoteProperties, QuoteStyle, Semicolons, TrailingCommas,
    };
    let quote = |single: bool| if single { QuoteStyle::Single } else { QuoteStyle::Double };
    FormatOptions {
      indent_style: if options.tabs { IndentStyle::Tab } else { IndentStyle::Space },
      indent_width: IndentWidth(u32::try_from(options.tab_width).unwrap_or(u32::MAX)),
      line_ending: match options.eol {
        Eol::Lf => LineEnding::Lf,
        Eol::Crlf => LineEnding::Crlf,
        Eol::Cr => LineEnding::Cr,
      },
      line_width: LineWidth(u32::try_from(options.width).unwrap_or(u32::MAX)),
      quote_style: quote(options.single_quote),
      jsx_quote_style: quote(options.jsx_single_quote),
      quote_properties: match options.quote_props {
        QuoteProps::AsNeeded => QuoteProperties::AsNeeded,
        QuoteProps::Consistent => QuoteProperties::Consistent,
        QuoteProps::Preserve => QuoteProperties::Preserve,
      },
      trailing_commas: match options.trailing_comma {
        TrailingComma::All => TrailingCommas::All,
        TrailingComma::Es5 => TrailingCommas::Es5,
        TrailingComma::None => TrailingCommas::None,
      },
      semicolons: if options.semi { Semicolons::Always } else { Semicolons::AsNeeded },
      arrow_parentheses: match options.arrow_parens {
        ArrowParens::Always => ArrowParentheses::Always,
        ArrowParens::Avoid => ArrowParentheses::AsNeeded,
      },
      bracket_spacing: BracketSpacing(options.bracket_spacing),
      bracket_same_line: BracketSameLine(options.bracket_same_line),
      expand: match options.object_wrap {
        ObjectWrap::Preserve => Expand::Auto,
        ObjectWrap::Collapse => Expand::Never,
      },
      experimental_ternaries: options.experimental_ternaries,
      experimental_operator_position: match options.experimental_operator_position {
        OperatorPosition::Start => Position::Start,
        OperatorPosition::End => Position::End,
      },
      embedded_language_formatting: match options.format_embedded {
        true => EmbeddedLanguageFormatting::Auto,
        false => EmbeddedLanguageFormatting::Off,
      },
      jsdoc: options.jsdoc.then(crate::js::options::JsdocOptions::default),
      attribute_position: match options.single_attribute_per_line {
        true => crate::js::options::AttributePosition::Multiline,
        false => crate::js::options::AttributePosition::Auto,
      },
      prose_wrap: match options.wrap {
        crate::Wrap::Always => crate::js::options::ProseWrap::Always,
        crate::Wrap::Never => crate::js::options::ProseWrap::Never,
        crate::Wrap::Preserve => crate::js::options::ProseWrap::Preserve,
      },
      html_whitespace_sensitivity: match options.whitespace {
        crate::Whitespace::Strict => crate::js::options::HtmlWhitespaceSensitivity::Strict,
        crate::Whitespace::Auto => crate::js::options::HtmlWhitespaceSensitivity::Css,
        crate::Whitespace::Ignore => crate::js::options::HtmlWhitespaceSensitivity::Ignore,
      },
      ..FormatOptions::default()
    }
  }
}

/// Formats `text`, the script at `path`, which may be empty. The dialect tells JavaScript from TypeScript, and the path
/// tells TypeScript from TSX.
pub fn format(text: &str, path: &str, options: &Options) -> Result<String, crate::Error> {
  Formatter::default().format(text, path, options).map(str::to_owned)
}

/// Formats one script after another, reusing its buffers.
#[derive(Default)]
pub struct Formatter {
  allocator: oxc_allocator::Allocator,
  scratch: Scratch,
  output: String,
}

impl Formatter {
  /// Formats `text`, the script at `path`, which may be empty.
  pub fn format(
    &mut self,
    text: &str,
    path: &str,
    options: &Options,
  ) -> Result<&str, crate::Error> {
    let (bom, text) = crate::without_bom(text);
    self.output.clear();
    // Prettier's core prints nothing for a text of white space only.
    if !crate::strings::trim_js_whitespace(text.as_bytes()).is_empty() {
      self.print(text, path, options)?;
    }
    Ok(crate::with_bom(bom, &mut self.output))
  }

  fn print(&mut self, text: &str, path: &str, options: &Options) -> Result<(), crate::Error> {
    self.allocator.reset();
    let allocator = &self.allocator;
    let format_options =
      FormatOptions { sort_imports: options.sort_imports.clone(), ..options.into() };
    let parsed = parse_any(allocator, text, source_types(options.dialect, path, text))?;
    let source_type = parsed.program.source_type;
    let mut recovered = parsed.diagnostics.to_vec();
    let program = allocator.alloc(parsed.program);
    let mut file = ast::File::new(text, path.as_bytes(), source_type, program, allocator);
    let how = options.sort_imports.as_deref();
    if let Some(sorted) = how.and_then(|how| sort_imports::sorted_text(&file, how)) {
      let sorted = String::from_utf8(sorted).map_err(|_| crate::Error::Syntax {
        message: "sorting imports gave invalid UTF-8".to_owned(),
        at: 0,
      })?;
      let sorted = allocator.alloc_str(&sorted);
      let parsed = parse_any(allocator, sorted, (source_type, None))?;
      recovered = parsed.diagnostics.to_vec();
      let program = allocator.alloc(parsed.program);
      file = ast::File::new(sorted, path.as_bytes(), source_type, program, allocator);
    }
    let mut out = std::mem::take(&mut self.output).into_bytes();
    let formatted =
      ir::run::format(&file, &format_options, &mut self.scratch, &mut out, format_file);
    self.output = String::from_utf8(out)
      .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
    match formatted {
      Ok(()) if recovered.is_empty() => Ok(()),
      Ok(()) => keeps_errors(&recovered, &self.output, source_type),
      Err(FormatError::NestedTooDeeply) => Err(crate::Error::TooDeep),
      Err(error) => Err(crate::Error::Syntax { message: format!("{error:?}"), at: 0 }),
    }
  }
}

/// The source type to read `text` as, and the one to try if it fails, as Prettier's parsers do: Babel reads JSX, and
/// TypeScript reads it in `.tsx` and JavaScript files, and when the path doesn't tell, first if the text looks like it
/// has some. A `.cjs` or `.cts` file is CommonJS, where `await` is a name.
fn source_types(
  dialect: Dialect,
  path: &str,
  text: &str,
) -> (oxc_span::SourceType, Option<oxc_span::SourceType>) {
  use oxc_span::SourceType;
  let lower = path.to_ascii_lowercase();
  let extension = lower.rsplit_once('.').map(|(_, extension)| extension);
  let commonjs = matches!(extension, Some("cjs" | "cts"));
  let (first, second) = match extension {
    _ if dialect == Dialect::Js => (SourceType::jsx(), None),
    Some("ts" | "mts" | "cts") => (SourceType::ts(), None),
    Some("tsx" | "js" | "jsx" | "mjs" | "cjs") => (SourceType::tsx(), None),
    _ if text.contains("</") || text.contains("/>") => (SourceType::tsx(), Some(SourceType::ts())),
    _ => (SourceType::ts(), Some(SourceType::tsx())),
  };
  let name = lower.rsplit(['/', '\\']).next().unwrap_or_default();
  let declaration = [".d.ts", ".d.mts", ".d.cts"].iter().any(|end| name.ends_with(end))
    || name
      .strip_suffix(".ts")
      .and_then(|stem| stem.rsplit_once(".d."))
      .is_some_and(|(_, inner)| !inner.contains('.'));
  // As Prettier's parsers do, read a module first, where `await(1)` awaits.
  let definition = |it: SourceType| {
    it.with_module(true).with_typescript_definition(it.is_typescript() && declaration)
  };
  (definition(first).with_commonjs(commonjs), second.map(definition))
}

/// Parses `text` as the first source type, or else the second, or fails with the error of the first.
fn parse_any<'a>(
  allocator: &'a oxc_allocator::Allocator,
  text: &'a str,
  (first, second): (oxc_span::SourceType, Option<oxc_span::SourceType>),
) -> Result<oxc_parser::ParserReturn<'a>, crate::Error> {
  let parsed = parse_module_or_script(allocator, text, first);
  let Some(error) = rejection(&parsed) else { return Ok(parsed) };
  if let Some(second) = second {
    let other = parse_module_or_script(allocator, text, second);
    if rejection(&other).is_none() {
      return Ok(other);
    }
  }
  let at = error.labels.as_ref().first().map_or(0, |label| label.offset() as usize);
  Err(crate::Error::Syntax { message: error.message.to_string(), at })
}

/// Parses `text` as a module, or else as a script, with the first one's errors if both fail. CommonJS is tried before a
/// module, which stands for Prettier's `allowImportExportEverywhere`.
fn parse_module_or_script<'a>(
  allocator: &'a oxc_allocator::Allocator,
  text: &'a str,
  source_type: oxc_span::SourceType,
) -> oxc_parser::ParserReturn<'a> {
  let other = match source_type.is_commonjs() {
    true => source_type.with_module(true),
    false => source_type.with_script(true),
  };
  let first = parse(allocator, text, source_type);
  match rejection(&first) {
    None => first,
    Some(_) => match parse(allocator, text, other) {
      second if rejection(&second).is_none() => second,
      _ => first,
    },
  }
}

/// Checks that `output`, formatted from a text with the errors `recovered`, has the same errors. Where oxc's tree has no
/// room for what an error is about, as the `declare` of `declare accessor a`, the printer drops it.
fn keeps_errors(
  recovered: &[oxc_diagnostics::OxcDiagnostic],
  output: &str,
  source_type: oxc_span::SourceType,
) -> Result<(), crate::Error> {
  let allocator = oxc_allocator::Allocator::default();
  if same_errors(recovered, &parse(&allocator, output, source_type).diagnostics) {
    return Ok(());
  }
  let error = &recovered[0];
  let at = error.labels.as_ref().first().map_or(0, |label| label.offset() as usize);
  Err(crate::Error::Syntax { message: error.message.to_string(), at })
}

/// True if formatting kept the errors of a text: `again` are those of the output.
pub(crate) fn same_errors(
  recovered: &[oxc_diagnostics::OxcDiagnostic],
  again: &[oxc_diagnostics::OxcDiagnostic],
) -> bool {
  // Printing puts modifiers in order, drops repeated ones and the comma after a rest element, as Prettier does.
  let fixed = |error: &&oxc_diagnostics::OxcDiagnostic| {
    matches!(error.code.number.as_deref(), Some("1029" | "1030"))
      || error.message == "A rest parameter or binding pattern may not have a trailing comma."
  };
  let messages = |errors: &[oxc_diagnostics::OxcDiagnostic]| {
    errors
      .iter()
      .filter(|error| !fixed(error))
      .map(|error| error.message.clone())
      .collect::<Vec<_>>()
  };
  messages(again) == messages(recovered)
}

/// The first error that Prettier's parser throws for `parsed`, if any. Its `typescript` parser throws on TypeScript's
/// syntax errors only, and its `babel` parser recovers from some errors (`allowedReasonCodes`), where oxc's tree is
/// whole.
pub(crate) fn rejection<'p>(
  parsed: &'p oxc_parser::ParserReturn<'_>,
) -> Option<&'p oxc_diagnostics::OxcDiagnostic> {
  let typescript = parsed.program.source_type.is_typescript();
  match parsed.fatal_error {
    true => parsed.diagnostics.first(),
    false => parsed.diagnostics.iter().find(|error| !is_recoverable(error, typescript)),
  }
}

fn is_recoverable(error: &oxc_diagnostics::OxcDiagnostic, typescript: bool) -> bool {
  let message = error.message.as_ref();
  let code = error.code.number.as_deref().filter(|_| error.code.scope.as_deref() == Some("TS"));
  if typescript && code.is_some_and(|code| CHECKER_ONLY.contains(&code)) {
    return true;
  }
  // TypeScript's checker reports these, as 1206, 1308 and 1014, where oxc has no code for them.
  if typescript
    && (message == "Decorators are not valid here."
      || message
        == "`await` is only allowed within async functions and at the top levels of modules"
      || message == "A rest parameter must be last in a parameter list")
  {
    return true;
  }
  // TypeScript's checker reports it as 1013, and Babel recovers from it (`RestTrailingComma`).
  if message == "A rest parameter or binding pattern may not have a trailing comma." {
    return true;
  }
  // `VarRedeclaration`, `DuplicateExport`, `DeclarationMissingInitializer`, `allowNewTargetOutsideFunction` and,
  // where TypeScript's scanner rejects it, `InvalidEscapeSequenceTemplate`.
  matches!(
    message,
    "Missing initializer in const declaration"
      | "Missing initializer in destructuring declaration"
      | "Unexpected new.target expression"
  ) || (message.starts_with("Identifier `") && message.ends_with("` has already been declared"))
    || message.starts_with("Duplicated export '")
    || (!typescript && message == "Bad escape sequence in untagged template literal")
}

/// The codes of the errors that oxc's parser reports, that TypeScript 5.9 only reports in its checker or binder, never in
/// its scanner or parser, and that typescript-estree doesn't check either.
const CHECKER_ONLY: &[&str] = &[
  "1009", "1015", "1016", "1019", "1021", "1024", "1028", "1029", "1030", "1031", "1035", "1036",
  "1038", "1039", "1040", "1047", "1049", "1051", "1063", "1070", "1071", "1079", "1090", "1092",
  "1093", "1094", "1095", "1096", "1097", "1098", "1099", "1108", "1113", "1141", "1147", "1164",
  "1172", "1173", "1174", "1175", "1176", "1183", "1184", "1187", "1194", "1221", "1222", "1242",
  "1243", "1248", "1249", "1257", "1268", "1273", "1274", "1275", "1276", "1277", "1316", "1318",
  "1319", "1325", "1337", "1354", "1363", "1392", "1545", "1546", "2206", "2207", "2398", "2452",
  "2499", "2528", "2566", "2681", "2730", "2784", "5085", "5086", "5087", "7059", "7060", "8002",
  "8005", "8011", "8012", "8016", "8037", "18007", "18010", "18019", "18041", "18058", "18059",
];

fn parse<'a>(
  allocator: &'a oxc_allocator::Allocator,
  text: &'a str,
  source_type: oxc_span::SourceType,
) -> oxc_parser::ParserReturn<'a> {
  let options = oxc_parser::ParseOptions {
    preserve_parens: true,
    allow_return_outside_function: true,
    ..oxc_parser::ParseOptions::default()
  };
  oxc_parser::Parser::new(allocator, text, source_type).with_options(options).parse()
}

pub(crate) fn format_file<'a>(file: &'a ast::File<'a>, f: &mut ir::formatter::Formatter<'a>) {
  print::program::write_program(file, f);
}
