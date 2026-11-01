//! CSS, SCSS and Less, formatted the way Prettier does. Ported from bun's `src/format/css` (MIT).
//!
//! Prettier parses a style sheet with `postcss`, and its contents with three more parsers: `postcss-values-parser` for
//! values, `postcss-selector-parser` for selectors and `postcss-media-query-parser` for media queries. Its output
//! depends on exactly how they split the text, bugs included. So this module ports all four parsers.

mod media_query;
mod memo;
mod misc;
mod parse;
mod postcss;
mod printer;
mod selector_parser;
mod sink;
mod value_groups;
mod value_parser;

use self::memo::Memo;
pub(crate) use self::sink::Document;
use self::sink::{Output, Sink};
use crate::Error;
use crate::doc;
use crate::embed::{Embed, Kind as Purpose, Request};
use crate::stack::Stack;
use std::borrow::Cow;

/// The language of a style sheet.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Dialect {
  #[default]
  Css,
  Scss,
  Less,
}

impl Dialect {
  /// The dialect of the file at `path`, as Prettier infers it, or `None` if it isn't a style sheet.
  pub fn of_path(path: &str) -> Option<Dialect> {
    let path = path.to_ascii_lowercase();
    let extension = path.rsplit_once('.')?.1;
    match extension {
      "css" | "wxss" | "pcss" | "postcss" => Some(Dialect::Css),
      "scss" => Some(Dialect::Scss),
      "less" => Some(Dialect::Less),
      _ => None,
    }
  }
}

/// Options of the CSS printer, Prettier's by default.
#[derive(Clone, Debug)]
pub struct Options {
  pub dialect: Dialect,
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: doc::Eol,
  pub single_quote: bool,
  /// `trailingComma` is not `"none"`: SCSS maps and lists that break get a trailing comma.
  pub trailing_comma: bool,
  /// Prettier's `embeddedLanguageFormatting`: `"auto"` if true, `"off"` if false. Off, front matter stays as it is
  /// written.
  pub format_embedded: bool,
  /// The ignore directive, Prettier's `prettier-ignore` by default. `/* name */`, or `// name` in SCSS and Less, keeps
  /// the next node as it is written.
  pub ignore: String,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      dialect: Dialect::Css,
      width: 80,
      tab_width: 2,
      tabs: false,
      eol: doc::Eol::Lf,
      single_quote: false,
      trailing_comma: true,
      format_embedded: true,
      ignore: "prettier-ignore".into(),
    }
  }
}

/// Formats `text`, handing the code of other languages, such as YAML front matter, to `embed`.
pub fn format(text: &str, options: &Options, embed: &mut dyn Embed) -> Result<String, Error> {
  Formatter::default().format(text, options, embed).map(str::to_owned)
}

/// Formats one style sheet after another, reusing its buffers.
#[derive(Default)]
pub struct Formatter {
  memo: Memo,
  output: Vec<u8>,
  text: String,
}

impl Formatter {
  /// Formats `text`, handing the code of other languages to `embed`.
  pub fn format(
    &mut self,
    text: &str,
    options: &Options,
    embed: &mut dyn Embed,
  ) -> Result<&str, Error> {
    let (bom, text) = crate::without_bom(text);
    let embed = if options.format_embedded { embed } else { &mut crate::embed::Verbatim };
    self.format_text(text, options, embed)?;
    Ok(crate::with_bom(bom, &mut self.text))
  }

  fn format_text(
    &mut self,
    text: &str,
    options: &Options,
    embed: &mut dyn Embed,
  ) -> Result<(), Error> {
    let text = crate::strings::lf(text.as_bytes());
    self.output.clear();
    if crate::strings::trim_js_whitespace(&text).is_empty() {
      self.text.clear();
      return Ok(());
    }
    let print = doc::Options {
      width: options.width,
      tab_width: options.tab_width,
      tabs: options.tabs,
      eol: options.eol,
    };
    let output = Output { out: &mut self.output, options: print };
    let sink = Sink::to_output(output, false);
    let style = Style { options, is_style_attribute: false };
    parse_and_print(&text, style, sink, &mut self.memo, embed)?;
    let printed = std::mem::take(&mut self.output);
    self.text = String::from_utf8(printed)
      .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
    Ok(())
  }
}

/// How a style sheet is printed.
#[derive(Copy, Clone)]
struct Style<'s> {
  options: &'s Options,
  /// Prettier's `__isHTMLStyleAttribute`: the declarations of a `style` attribute go on one line if they fit.
  is_style_attribute: bool,
}

/// The document of `text`, a style sheet embedded in markup: Prettier's `textToDoc`, with the `cleanDoc` that HTML's
/// printer runs if `is_cleaned` is set. `is_style_attribute` is set for the value of a `style` attribute.
pub(crate) fn document(
  text: &str,
  options: &Options,
  is_style_attribute: bool,
  is_cleaned: bool,
  embed: &mut dyn Embed,
) -> Result<Document, Error> {
  let text = crate::strings::lf(text.as_bytes());
  let mut memo = Memo::default();
  let style = Style { options, is_style_attribute };
  let sink = parse_and_print(&text, style, Sink::to_document(is_cleaned), &mut memo, embed)?;
  Ok(sink.into_document())
}

/// Prettier's `replaceQuotesInInlineComments`. Returns the inline comments whose quotes and asterisks must be replaced
/// with spaces so that `postcss-less` doesn't trip over them.
fn inline_comments_with_quotes(text: &[u8]) -> Vec<(usize, usize)> {
  #[derive(Copy, Clone, PartialEq)]
  enum State {
    Initial,
    Quotes(u8),
    Url,
    CommentBlock,
    CommentInline,
  }
  let mut state = State::Initial;
  let mut state_to_return_from_quotes = State::Initial;
  let mut inline_comment_start = 0;
  let mut inline_comment_contains_quotes = false;
  let mut comments = Vec::new();
  let mut i = 0;
  while let Some(&c) = text.get(i) {
    match state {
      State::Initial => match c {
        b'\'' | b'"' => state = State::Quotes(c),
        b'u' | b'U' if text.get(i..i + 4).is_some_and(|it| it.eq_ignore_ascii_case(b"url(")) => {
          state = State::Url;
          i += 3;
        }
        b'/' if text.get(i + 1) == Some(&b'*') => {
          state = State::CommentBlock;
          i += 1;
        }
        b'/' if text.get(i + 1) == Some(&b'/') => {
          state = State::CommentInline;
          inline_comment_start = i;
          i += 1;
        }
        _ => {}
      },
      State::Quotes(quote) => {
        if c == quote && text[i - 1] != b'\\' {
          state = std::mem::replace(&mut state_to_return_from_quotes, State::Initial);
        }
        if matches!(c, b'\n' | b'\r') {
          return Vec::new();
        }
      }
      State::Url => match c {
        b')' => state = State::Initial,
        b'\n' | b'\r' => return Vec::new(),
        b'\'' | b'"' => {
          state = State::Quotes(c);
          state_to_return_from_quotes = State::Url;
        }
        _ => {}
      },
      State::CommentBlock => {
        if c == b'/' && text[i - 1] == b'*' {
          state = State::Initial;
        }
      }
      State::CommentInline => match c {
        b'"' | b'\'' | b'*' => inline_comment_contains_quotes = true,
        b'\n' | b'\r' => {
          if inline_comment_contains_quotes {
            comments.push((inline_comment_start, i));
          }
          state = State::Initial;
          inline_comment_contains_quotes = false;
        }
        _ => {}
      },
    }
    i += 1;
  }
  comments
}

/// Parses `text`, whose line breaks must be `\n`, and writes it to `sink`.
fn parse_and_print<'o>(
  text: &[u8],
  style: Style<'_>,
  mut sink: Sink<'o>,
  memo: &'o mut Memo,
  embed: &mut dyn Embed,
) -> Result<Sink<'o>, Error> {
  let options = style.options;
  let dialect = options.dialect;
  // The front matter is replaced with spaces before parsing, so all offsets stay valid.
  let front_matter = std::str::from_utf8(text)
    .ok()
    .and_then(crate::front_matter::parse)
    .map(|matter| &text[..matter.len]);
  let mut blanked: Cow<'_, [u8]> = match front_matter {
    None => Cow::Borrowed(text),
    Some(front_matter) => {
      let mut blanked = text.to_vec();
      for byte in &mut blanked[..front_matter.len()] {
        if *byte != b'\n' {
          *byte = b' ';
        }
      }
      Cow::Owned(blanked)
    }
  };

  if dialect == Dialect::Less {
    for (start, end) in inline_comments_with_quotes(&blanked) {
      for byte in &mut blanked.to_mut()[start..end] {
        if matches!(byte, b'"' | b'\'' | b'*') {
          *byte = b' ';
        }
      }
    }
  }

  let tree = postcss::parse(&blanked, dialect)?;
  if let Some(front_matter) = front_matter {
    let has_nodes = tree.nodes[0].first_child != 0;
    print_front_matter(front_matter, &mut sink, options, embed);
    sink.hard_line();
    if has_nodes {
      sink.hard_line();
    }
  }
  let mut printer = printer::Printer {
    context: parse::Context {
      text: &blanked,
      original_text: text,
      is_original_text: matches!(blanked, Cow::Borrowed(_)),
      extra: &tree.extra,
      syntax: dialect,
      rejection: Default::default(),
      stack: Stack::new(),
      convertible_sets: Default::default(),
    },
    single_quote: options.single_quote,
    ignore: options.ignore.as_bytes(),
    trailing_comma: options.trailing_comma,
    is_html_style_attribute: style.is_style_attribute,
    value_stack: Vec::new(),
    index_in_comma_group: 0,
    scratch: Vec::new(),
    failure: None,
    sink,
    memo,
    is_memoizable: false,
  };
  printer.print_root(&tree, &mut parse::Parsed::default());
  if printer.context.stack.ran_out() || printer.sink.is_too_deep() {
    return Err(Error::TooDeep);
  }
  match printer.failure {
    Some(error) => Err(error.into()),
    None => Ok(printer.sink),
  }
}

/// Prettier's `printEmbedFrontMatter`, which formats YAML front matter with the host, or the front matter as it is
/// written.
fn print_front_matter(raw: &[u8], sink: &mut Sink<'_>, options: &Options, embed: &mut dyn Embed) {
  let matter = std::str::from_utf8(raw).ok().and_then(crate::front_matter::parse);
  let formatted = matter.as_ref().filter(|matter| matter.language == "yaml").and_then(|matter| {
    let value = matter.value.trim();
    if value.is_empty() {
      return Some(String::new());
    }
    let request = Request {
      code: value,
      language: "yaml",
      kind: Purpose::FrontMatter,
      indent: 0,
      width: options.width,
      module: true,
    };
    embed.format(request).map(|text| text.trim_end_matches(['\n', '\r']).to_owned())
  });
  let (Some(matter), Some(formatted)) = (matter, formatted) else {
    return sink.text(raw);
  };
  sink.text(matter.start.as_bytes());
  sink.text(matter.explicit.unwrap_or_default().as_bytes());
  sink.hard_line();
  if !formatted.is_empty() {
    for (index, line) in formatted.lines().enumerate() {
      if index > 0 {
        sink.hard_line();
      }
      sink.text(line.as_bytes());
    }
    sink.hard_line();
  }
  sink.text(matter.end.as_bytes());
}
