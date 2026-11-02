//! HTML in JavaScript templates: Prettier's `language-js/embed/html.js`.
//!
//! The text has a placeholder instead of each substitution. Its document is written with the substitutions in place of
//! the placeholders.

use super::doc::{Texts, Writer, write_string};
use crate::doc::{Docs, Element, Line};
use crate::html::Parser;
use crate::js::base::*;
use crate::js::embed::css::is_blank;
use crate::js::ir::element::{Group, Interned, LineMode};
use crate::js::options::{AttributePosition, HtmlRoot, HtmlWhitespaceSensitivity, IndentStyle};
use crate::js::print::template::write_embedded_template_expression;
use crate::js::strings;
use crate::js::text;
use smallvec::SmallVec;
use std::cell::RefCell;

pub(crate) const PLACEHOLDER_START: &[u8] = b"PRETTIER_HTML_PLACEHOLDER_";
const PLACEHOLDER_END: &[u8] = b"_IN_JS";

/// `/^PRETTIER_HTML_PLACEHOLDER_\d+_\d+_IN_JS$/.test(value)`
pub(crate) fn is_placeholder_in_js(value: &[u8]) -> bool {
  let digits = |text: &[u8]| text.iter().take_while(|byte| byte.is_ascii_digit()).count();
  let Some(rest) = value.strip_prefix(PLACEHOLDER_START) else {
    return false;
  };
  let first = digits(rest);
  let Some(rest) = rest[first..].strip_prefix(b"_") else {
    return false;
  };
  let second = digits(rest);
  first > 0 && second > 0 && &rest[second..] == PLACEHOLDER_END
}

/// `isAngularComponentTemplate`: `` @Component({ template: `..` }) ``. `parent` is the template's parent.
fn is_angular_component_template(parent: AstNodes<'_>) -> bool {
  crate::js::embed::css::is_angular_component_property(parent, b"template")
}

/// `` html`..` ``. `e` is a template and `parent` is its parent.
fn has_html_tag<'a>(e: Expr<'a>, parent: AstNodes<'a>) -> bool {
  matches!(parent, AstNodes::TaggedTemplateExpression(tagged)
        if matches!(tagged.kind(), ExprKind::TaggedTemplate(call)
            if call.callee() != e && matches!(call.callee().kind(), ExprKind::Ident(_)) && call.callee().text() == b"html"))
}

/// Returns the parser for the text of the template `e`, if `embed` treats it as HTML.
fn parser_of<'a>(e: Expr<'a>, template: Template<'a>, f: &Formatter<'a>) -> Option<Parser> {
  if !matches!(f.options().embedded_language_formatting, EmbeddedLanguageFormatting::Auto) {
    return None;
  }
  let parent = e.ast_parent();
  let parser = if has_html_tag(e, parent)
    || crate::js::embed::graphql::has_language_comment(e, parent, b" HTML ", f)
  {
    Parser::Html
  } else if is_angular_component_template(parent) {
    Parser::Angular
  } else {
    return None;
  };
  // CSS and GraphQL take precedence.
  let is_in_another_language =
    crate::js::embed::css::is_embed_css(e) || crate::js::embed::graphql::is_embed_graphql(e, f);
  (!is_in_another_language
    && (0..template.quasi_count()).all(|index| template.cooked(index).is_some()))
  .then_some(parser)
}

/// Returns the text of `template` with placeholders. `counter` distinguishes them from the placeholders of an outer
/// template whose text contains this one.
fn text_with_placeholders(template: Template<'_>, counter: u32) -> String {
  let mut text = Vec::new();
  for index in 0..template.quasi_count() {
    if index > 0 {
      text.extend_from_slice(PLACEHOLDER_START);
      text.extend_from_slice((index - 1).to_string().as_bytes());
      text.push(b'_');
      text.extend_from_slice(counter.to_string().as_bytes());
      text.extend_from_slice(PLACEHOLDER_END);
    }
    text.extend_from_slice(template.cooked(index).map_or(&[][..], |cooked| cooked.bytes()));
  }
  // An escape can produce a lone surrogate, which becomes U+FFFD once Prettier's output is written as UTF-8.
  let mut from = 0;
  while let Some(at) = strings::index_of_char_pos(&text, 0xED, from) {
    if text.get(at + 1).is_some_and(|&byte| byte >= 0xA0)
      && let Some(half) = text.get_mut(at..at + 3)
    {
      half.copy_from_slice("\u{FFFD}".as_bytes());
    }
    from = at + 1;
  }
  let text = match strings::contains_char(&text, b'\r') {
    true => crate::js::strings::crlf_as_lf(&text).into_owned(),
    false => text,
  };
  String::from_utf8(text)
    .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

/// The first counter from `start` on whose placeholders aren't in the text of `template` already, as those of an outer
/// template in a script of this one are.
fn counter_of(template: Template<'_>, start: u32) -> u32 {
  let has = |counter: u32| {
    let end = [b"_", counter.to_string().as_bytes(), PLACEHOLDER_END].concat();
    (0..template.quasi_count())
      .any(|index| template.cooked(index).is_some_and(|text| strings::contains(text.bytes(), &end)))
  };
  (start..).find(|&counter| !has(counter)).unwrap_or(start)
}

/// Returns the line break between the backticks and the text, or `None` if there's none.
fn surrounding_line(text: &str, options: &FormatOptions) -> Option<LineMode> {
  let text = text.as_bytes();
  if options.html_whitespace_sensitivity == HtmlWhitespaceSensitivity::Ignore {
    return Some(LineMode::Hard);
  }
  (text::starts_with_white_space(text) && strings::trim_js_whitespace_end(text).len() < text.len())
    .then_some(LineMode::SoftOrSpace)
}

/// Returns the number of `indent`s around the line that `e` starts on, assuming the file is already indented the way
/// it's going to be.
fn indent_level_in_source<'a>(e: Expr<'a>, options: &FormatOptions, f: &Formatter<'a>) -> u32 {
  let start = e.span().start;
  // A file that's one long line has its templates visited in order, so resume from the last search instead of
  // rescanning the line.
  let (known_at, known) = f.context().line_start.get();
  let from = if known_at <= start { known_at } else { 0 };
  let text = e.file().text();
  let searched = text.get(from as usize..start as usize).unwrap_or_default();
  let line_start = match strings::last_index_of_any(searched, b"\n\r") {
    Some(at) => from + at as u32 + 1,
    None if from == 0 => 0,
    None => known,
  };
  f.context().line_start.set((start, line_start));
  let indent_width = options.indent_width.value().max(1);
  let mut columns = 0;
  for byte in text.get(line_start as usize..start as usize).unwrap_or_default() {
    match byte {
      b' ' => columns += 1,
      b'\t' => columns += indent_width,
      _ => break,
    }
  }
  columns / indent_width
}

/// The label on a template's document.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Label {
  /// `{ embed: true }`
  Embed,
  /// `{ embed: true, hug: false }`
  EmbedWithoutHug,
}

/// Returns the label on `e`'s document if `e` is a template, tagged or not, that's formatted as HTML and isn't just
/// ` `` `.
pub(crate) fn label<'a>(e: Expr<'a>, f: &Formatter<'a>) -> Option<Label> {
  let quasi = match e.kind() {
    ExprKind::TaggedTemplate(call) => call.template(),
    _ => Some(e),
  };
  let (quasi, ExprKind::Template(template)) = quasi.map(|quasi| (quasi, quasi.kind()))? else {
    return None;
  };
  let parser = parser_of(quasi, template, f).filter(|_| !is_blank(template))?;
  let text = text_with_placeholders(template, f.options().html_template_depth);
  crate::html::parses(&text, parser).then(|| match surrounding_line(&text, f.options()) {
    Some(_) => Label::Embed,
    None => Label::EmbedWithoutHug,
  })
}

/// The callback of the `mapDoc` in `printEmbedHtmlLike`.
struct Substitutions<'e> {
  /// The substitutions' documents.
  expressions: &'e [Interned],
  /// `_`, the counter and the end of a placeholder.
  placeholder_end: Vec<u8>,
  /// `__embeddedInHtml`: a script's end tag would end the script that contains the template.
  is_in_html: bool,
}

/// `/^<\/(?=script\b)/i.test(text)`
fn starts_with_end_of_script(text: &[u8]) -> bool {
  text.starts_with(b"</")
    && text.get(2..8).is_some_and(|name| name.eq_ignore_ascii_case(b"script"))
    && !text.get(8).is_some_and(|&byte| strings::is_regexp_word_byte(byte))
}

impl Substitutions<'_> {
  /// Returns the start, the end and the number of the first placeholder in `text`.
  fn find_placeholder(&self, text: &[u8]) -> Option<(usize, usize, usize)> {
    let mut from = 0;
    while let Some(start) = text::index_of_from(text, PLACEHOLDER_START, from) {
      let digits_start = start + PLACEHOLDER_START.len();
      let digits = text[digits_start..].iter().take_while(|byte| byte.is_ascii_digit()).count();
      let end = digits_start + digits;
      if digits > 0 && text[end..].starts_with(&self.placeholder_end) {
        let number = text[digits_start..end].iter().fold(0usize, |number, digit| {
          number.saturating_mul(10).saturating_add(usize::from(digit - b'0'))
        });
        return Some((start, end + self.placeholder_end.len(), number));
      }
      from = digits_start;
    }
    None
  }

  fn needs_escapes(&self, text: &[u8]) -> bool {
    strings::index_of_any(text, b"\\`$").is_some()
      || (self.is_in_html && strings::contains_char(text, b'<'))
  }

  /// The width of `text`, where a placeholder of an outer template counts as wide as its substitution.
  fn width(text: &[u8], f: &Formatter<'_>) -> u32 {
    let width = f.string_width(text);
    if !strings::contains(text, PLACEHOLDER_START) {
      return width;
    }
    OUTER.with_borrow(|outer| {
      let mut width = width;
      let mut from = 0;
      while let Some(start) = text::index_of_from(text, PLACEHOLDER_START, from) {
        from = start + PLACEHOLDER_START.len();
        let Some((length, number, counter)) = parse_placeholder(&text[start..]) else { continue };
        let substitution = outer.iter().rev().find(|(it, _)| *it == counter);
        if let Some(&Some(substitution)) = substitution.and_then(|(_, widths)| widths.get(number)) {
          width = width + substitution - length as u32;
          from = start + length;
        }
      }
      width
    })
  }

  /// Writes `text`, which is between placeholders: `uncookTemplateElementValue`.
  fn write_text(&self, text: &[u8], f: &mut Formatter<'_>) {
    if text.is_empty() {
      return;
    }
    let escaped;
    let text = match self.needs_escapes(text) {
      false => text,
      true => {
        let mut with_escapes = Vec::with_capacity(text.len() + 8);
        for (index, &byte) in text.iter().enumerate() {
          match byte {
            b'\\' | b'`' => with_escapes.push(b'\\'),
            b'$' if text.get(index + 1) == Some(&b'{') => with_escapes.push(b'\\'),
            b'/'
              if self.is_in_html && index > 0 && starts_with_end_of_script(&text[index - 1..]) =>
            {
              with_escapes.push(b'\\')
            }
            _ => {}
          }
          with_escapes.push(byte);
        }
        escaped = with_escapes;
        &escaped[..]
      }
    };
    write_string(text, Self::width(text, f), f);
  }
}

/// The length, the number and the counter of the placeholder at the start of `text`.
fn parse_placeholder(text: &[u8]) -> Option<(usize, usize, u32)> {
  let rest = text.strip_prefix(PLACEHOLDER_START)?;
  let digits = |text: &[u8]| text.iter().take_while(|byte| byte.is_ascii_digit()).count();
  let number = digits(rest);
  let after = rest[number..].strip_prefix(b"_")?;
  let counter = digits(after);
  if number == 0 || counter == 0 || !after[counter..].starts_with(PLACEHOLDER_END) {
    return None;
  }
  let length = text.len() - after.len() + counter + PLACEHOLDER_END.len();
  let number = std::str::from_utf8(&rest[..number]).ok()?.parse().ok()?;
  let counter = std::str::from_utf8(&after[..counter]).ok()?.parse().ok()?;
  Some((length, number, counter))
}

thread_local! {
  /// The counter of each template whose markup is formatted now, outermost first, and the widths of its
  /// substitutions: Prettier measures a placeholder of an outer template in the code of an inner one as the document
  /// that replaces it, once the whole document is put together.
  static OUTER: RefCell<Vec<(u32, Vec<Option<u32>>)>> = const { RefCell::new(Vec::new()) };
}

/// The width of each substitution of `template` on one line, as `${` and `}` around the expression's text.
fn substitution_widths(template: Template<'_>, f: &Formatter<'_>) -> Vec<Option<u32>> {
  (0..template.quasi_count().saturating_sub(1))
    .map(|index| {
      let span = template.quasi_span(index).between(template.quasi_span(index + 1));
      let code = f.source_text().text_for(&span);
      let code = strings::trim_js_whitespace(code);
      (!strings::contains_char(code, b'\n')).then(|| f.string_width(code) + 3)
    })
    .collect()
}

impl<'a> Texts<'a> for Substitutions<'_> {
  fn write(&mut self, text: &str, width: u32, f: &mut Formatter<'a>) {
    let mut rest = text.as_bytes();
    if !self.needs_escapes(rest) && !strings::contains(rest, PLACEHOLDER_START) {
      return write_string(rest, width, f);
    }
    while let Some((start, end, number)) = self.find_placeholder(rest) {
      self.write_text(&rest[..start], f);
      if let Some(&expression) = self.expressions.get(number) {
        f.write_element(FormatElement::Interned(expression));
      }
      rest = &rest[end..];
    }
    self.write_text(rest, f);
  }
}

/// True if `docs`, the document of `text`, still has the same number of substitutions once printed. If not, oxfmt
/// leaves the template as it is.
fn keeps_substitution_count(text: &str, docs: &Docs<'_>, options: &crate::html::Options) -> bool {
  let print = crate::doc::Options {
    width: options.width,
    tab_width: options.tab_width,
    tabs: options.tabs,
    eol: crate::doc::Eol::Lf,
  };
  let printed = crate::doc::print(docs, &print, String::new());
  text.matches("PRETTIER_HTML_PLACEHOLDER_").count()
    == printed.matches("PRETTIER_HTML_PLACEHOLDER_").count()
}

/// The options of markup in a template, `indent_level` indents deep.
fn markup_options(parser: Parser, indent_level: u32, f: &Formatter<'_>) -> crate::html::Options {
  let options = f.options();
  let tab_width = options.indent_width.value() as usize;
  crate::html::Options {
    parser,
    width: (options.line_width.value() as usize).saturating_sub(indent_level as usize * tab_width),
    tab_width,
    tabs: options.indent_style == IndentStyle::Tab,
    whitespace: match options.html_whitespace_sensitivity {
      HtmlWhitespaceSensitivity::Css => crate::Whitespace::Auto,
      HtmlWhitespaceSensitivity::Strict => crate::Whitespace::Strict,
      HtmlWhitespaceSensitivity::Ignore => crate::Whitespace::Ignore,
    },
    single_attribute_per_line: options.attribute_position == AttributePosition::Multiline,
    bracket_same_line: options.bracket_same_line.value(),
    vue_indent_script_and_style: options.vue_indent_script_and_style,
    filepath: options.is_in_html_file.then(|| ".html".into()),
    js: super::scripts(options),
    ..crate::html::Options::default()
  }
}

/// Writes the template `e` as HTML if Prettier treats it as HTML: `printEmbedHtmlLike`. Returns true if it did, and
/// false if the text can't be parsed.
pub(crate) fn write_template<'a>(
  e: Expr<'a>,
  template: Template<'a>,
  f: &mut Formatter<'a>,
) -> bool {
  let Some(parser) = parser_of(e, template, f) else {
    return false;
  };
  if is_blank(template) {
    f.write_token("``");
    return true;
  }
  let counter = counter_of(template, f.options().html_template_depth);
  let text = text_with_placeholders(template, counter);
  let line = surrounding_line(&text, f.options());
  // The printer decides where the lines start. But languages that are printed separately (JSON, YAML, ..) need to
  // know the available width up front, so assume the indentation the file would have if it had been formatted before.
  let indent_level = indent_level_in_source(e, f.options(), f) + u32::from(line.is_some());
  let options = markup_options(parser, indent_level, f);
  let bump = bumpalo::Bump::new();
  let mut docs = Docs::new(&bump);
  let widths = substitution_widths(template, f);
  OUTER.with_borrow_mut(|outer| outer.push((counter, widths)));
  let result = crate::html::template(&text, &options, &mut docs);
  OUTER.with_borrow_mut(Vec::pop);
  let Ok(top_level_count) = result else {
    return false;
  };
  // Both tools format the HTML with Prettier's options.
  if f.options().flavor.is_oxfmt() && !keeps_substitution_count(&text, &docs, &options) {
    return false;
  }
  let end = match docs.element(docs.len() - 1) {
    Element::Line(Line::Hard) => docs.len() - 1,
    _ => docs.len(),
  };
  // Written in source order, whatever the order of the placeholders, because comments are consumed in that order.
  let expressions: SmallVec<[Interned; 8]> = (0..template.quasi_count().saturating_sub(1))
    .map(|index| {
      f.capture(&format_with(|f| write_embedded_template_expression(template, index, f)))
    })
    .collect();
  let mut substitutions = Substitutions {
    expressions: &expressions,
    placeholder_end: [b"_", counter.to_string().as_bytes(), PLACEHOLDER_END].concat(),
    is_in_html: f.options().in_html.root != HtmlRoot::None,
  };

  let tag = |tag: Tag, f: &mut Formatter<'a>| f.write_element(FormatElement::Tag(tag));
  let is_indented = line.is_some() || top_level_count > 1;
  tag(Tag::StartGroup(Group::new()), f);
  f.write_token("`");
  if line.is_none() && text::starts_with_white_space(text.as_bytes()) {
    f.write_token(" ");
  }
  if is_indented {
    tag(Tag::StartIndent, f);
  }
  if let Some(mode) = line {
    f.write_element(FormatElement::Line(mode));
  }
  tag(Tag::StartGroup(Group::new()), f);
  let breaks = u32::from(line == Some(LineMode::Hard));
  Writer::new(&docs, breaks, false).write(1, end, &mut substitutions, f);
  tag(Tag::EndGroup, f);
  if is_indented {
    tag(Tag::EndIndent, f);
  }
  match line {
    Some(mode) => f.write_element(FormatElement::Line(mode)),
    None if strings::trim_js_whitespace_end(text.as_bytes()).len() < text.len() => {
      f.write_token(" ")
    }
    None => {}
  }
  f.write_token("`");
  tag(Tag::EndGroup, f);
  true
}
