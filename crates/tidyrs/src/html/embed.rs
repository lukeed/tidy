//! Prettier's `embed.js` and `embed/*`: code of other languages, through the host's [`crate::embed::Embed`].

use std::borrow::Cow;

use super::Parser;
use super::ast::{Flags, Id};
use super::print::{Printer, raw_name};
use super::utilities::{
  dedent_string, html_split, html_trim_preserve_indentation, is_script_like_tag,
  unescape_quote_entities,
};
use crate::doc::{Align, Branch};
use crate::embed::{Kind as Purpose, Request};
use crate::strings;
use crate::{front_matter, languages};

/// Code to hand to the host.
pub(crate) struct Embedded<'c> {
  code: &'c str,
  language: &'c str,
  kind: Purpose,
  /// Indents beyond the printer's own, where the code is printed in an `indent`.
  extra: usize,
  module: bool,
}

impl<'c> Embedded<'c> {
  pub(crate) fn request(&self, indents: usize, options: &super::Options<'_>) -> Request<'c> {
    Request {
      code: self.code,
      language: self.language,
      kind: self.kind,
      indent: indents + self.extra,
      width: options.width,
      module: self.module,
    }
  }
}

fn text(bytes: &[u8]) -> Cow<'_, str> {
  String::from_utf8_lossy(bytes)
}

// ───────────────────────────── parsers ─────────────────────────────

/// `inferParserByTypeAttribute`
fn parser_by_type(kind: &[u8]) -> Option<&'static str> {
  match kind {
    b"module" | b"text/javascript" | b"text/babel" | b"text/jsx" | b"application/javascript" => {
      Some("babel")
    }
    b"application/x-typescript" => Some("typescript"),
    b"text/markdown" => Some("markdown"),
    b"text/html" => Some("html"),
    b"text/x-handlebars-template" => Some("glimmer"),
    _ if kind.ends_with(b"json") || kind.ends_with(b"importmap") || kind == b"speculationrules" => {
      Some("json")
    }
    _ => None,
  }
}

fn parser_by_language(language: Option<&[u8]>) -> Option<&'static str> {
  languages::parser(std::str::from_utf8(language?).ok()?)
}

/// `inferElementParser`
fn element_parser(printer: &Printer<'_, '_>, id: Id) -> Option<&'static str> {
  let tree = printer.tree;
  let node = &tree[id];
  let attribute = |name: &[u8]| tree.attribute(id, name).map(Option::unwrap_or_default);
  if &node.name[..] == b"script" && attribute(b"src").is_none() {
    let (kind, lang) = (attribute(b"type"), attribute(b"lang"));
    if lang.is_none_or(<[u8]>::is_empty) && kind.is_none_or(<[u8]>::is_empty) {
      return Some("babel");
    }
    return parser_by_language(lang.filter(|lang| !lang.is_empty()))
      .or_else(|| parser_by_type(kind.unwrap_or_default()));
  }
  if &node.name[..] == b"style" {
    return match attribute(b"lang").filter(|lang| !lang.is_empty()) {
      Some(lang) => parser_by_language(Some(lang)),
      None => Some("css"),
    };
  }
  if &node.name[..] == b"mj-style" && printer.options.parser == Parser::Mjml {
    return Some("css");
  }
  if tree.is_vue_non_html_block(id, printer.options) && attribute(b"src").is_none() {
    let (kind, lang) = (attribute(b"type"), attribute(b"lang"));
    return parser_by_language(lang.filter(|lang| !lang.is_empty()))
      .or_else(|| parser_by_type(kind.unwrap_or_default()));
  }
  None
}

// ───────────────────────────── nodes ─────────────────────────────

/// A Vue block in another language than HTML.
pub(crate) fn element(printer: &mut Printer<'_, '_>, id: Id) -> bool {
  let tree = printer.tree;
  let node = &tree[id];
  if is_script_like_tag(node, printer.options)
    || node.has(Flags::IS_SELF_CLOSING)
    || !tree.is_vue_non_html_block(id, printer.options)
  {
    return false;
  }
  let Some(language) = element_parser(printer, id) else { return false };
  let tags = printer.tags();
  let content = tags.node_content(id);
  let mut formatted = String::new();
  if !strings::trim_js_whitespace(content).is_empty() {
    let code = text(html_trim_preserve_indentation(content));
    match printer.embed_text(Embedded {
      code: &code,
      language,
      kind: Purpose::Block,
      extra: 0,
      module: true,
    }) {
      Some(text) => formatted = text,
      None => return false,
    }
  }
  printer.marker(|tags, out| tags.opening_tag_prefix(id, out));
  let group = printer.docs.group();
  printer.print_opening_tag(id);
  printer.docs.close(group);
  if !formatted.is_empty() {
    printer.docs.hardline();
    printer.text_doc(&formatted);
    printer.docs.hardline();
  }
  printer.print_closing_tag(id);
  printer.marker(|tags, out| tags.closing_tag_suffix(id, out));
  true
}

/// The text in a `<script>` or a `<style>`.
pub(crate) fn script(printer: &mut Printer<'_, '_>, id: Id, parent: Id) -> bool {
  let Some(language) = element_parser(printer, parent) else { return false };
  let tree = printer.tree;
  let value = &tree[id].value[..];
  let code = if language == "markdown" {
    let first = value
      .iter()
      .position(|&byte| byte == b'\n')
      .filter(|&at| value[..at].iter().all(|byte| byte.is_ascii_whitespace()));
    let value = first.map_or(value, |at| &value[at + 1..]);
    text(&dedent_string(value)).into_owned()
  } else {
    text(value).into_owned()
  };
  let module = is_module(printer, parent, language);
  let embedded = Embedded { code: &code, language, kind: Purpose::Block, extra: 0, module };
  // `</script` would end the element. Prettier only prints one in a template of HTML in JavaScript, where it escapes it.
  let formatted = printer.embed_text(embedded).map(|formatted| match &tree[parent].name[..] {
    b"script" => escape_script_end(formatted),
    _ => formatted,
  });
  // Without the host's answer, a style sheet is formatted here.
  let document = match &formatted {
    Some(_) => None,
    None => match style_sheet(printer, language, &code, false) {
      Some(document) => Some(document),
      None => return false,
    },
  };
  printer.docs.break_parent();
  printer.marker(|tags, out| tags.opening_tag_prefix(id, out));
  match (formatted, document) {
    (Some(formatted), _) => printer.text_doc(&formatted),
    (None, Some(document)) => document.write(printer.docs),
    (None, None) => {}
  }
  printer.marker(|tags, out| tags.closing_tag_suffix(id, out));
  true
}

/// Whether the code of a `<script>` is a module. In HTML, Prettier parses JavaScript as a classic script, unless `type`
/// makes it a module.
fn is_module(printer: &Printer<'_, '_>, id: Id, language: &str) -> bool {
  if printer.options.parser != Parser::Html || language != "babel" {
    return true;
  }
  let attribute = |name: &[u8]| printer.tree.attribute(id, name).flatten().unwrap_or_default();
  match attribute(b"type") {
    b"module" => true,
    b"text/babel" | b"text/jsx" => attribute(b"data-type") == b"module",
    _ => false,
  }
}

/// `text` with each `</script`, in any case, as `<\/script`.
fn escape_script_end(text: String) -> String {
  let lower = text.to_ascii_lowercase();
  if !lower.contains("</script") {
    return text;
  }
  let mut escaped = String::with_capacity(text.len() + 8);
  let mut last = 0;
  for (at, _) in lower.match_indices("</script") {
    escaped.push_str(&text[last..at + 1]);
    escaped.push('\\');
    last = at + 1;
  }
  escaped.push_str(&text[last..]);
  escaped
}

/// The document of a style sheet in `language`, formatted by `crate::css`, or `None` if it isn't one or doesn't parse.
fn style_sheet(
  printer: &mut Printer<'_, '_>,
  language: &str,
  code: &str,
  is_style_attribute: bool,
) -> Option<crate::css::Document> {
  if !printer.options.formats_embedded {
    return None;
  }
  let dialect = match language {
    "css" => crate::css::Dialect::Css,
    "scss" => crate::css::Dialect::Scss,
    "less" => crate::css::Dialect::Less,
    _ => return None,
  };
  let settings = crate::css::Settings {
    dialect,
    width: printer.options.width,
    tab_width: printer.options.tab_width,
    ..crate::css::Settings::default()
  };
  crate::css::document(code, &settings, is_style_attribute, true, printer.embed).ok()
}

/// The expression in `{{ }}`.
pub(crate) fn interpolation(printer: &mut Printer<'_, '_>, id: Id, parent: Id) -> bool {
  let tree = printer.tree;
  let language = match printer.options.parser {
    Parser::Vue if printer.options.typescript => "__vue_ts_expression",
    Parser::Vue => "__vue_expression",
    _ => "__js_expression",
  };
  let code = text(&tree[id].value).into_owned();
  let Some(formatted) = printer.embed_text(Embedded {
    code: &code,
    language,
    kind: Purpose::Expression,
    extra: 1,
    module: true,
  }) else {
    return false;
  };
  let indent = printer.docs.indent();
  printer.docs.line();
  printer.text_doc(&formatted);
  printer.docs.close(indent);
  let tags = printer.tags();
  if tree.next(parent).is_some_and(|next| tags.needs_to_borrow_prev_closing_tag_end_marker(next)) {
    printer.docs.text(" ");
  } else {
    printer.docs.line();
  }
  true
}

/// `printEmbedFrontMatter`
pub(crate) fn front_matter(printer: &mut Printer<'_, '_>, raw: &[u8]) -> bool {
  let Some(matter) = std::str::from_utf8(raw).ok().and_then(front_matter::parse) else {
    return false;
  };
  let language = match matter.language {
    "yaml" => "yaml",
    "toml" => return false,
    other => match languages::parser(other) {
      Some(language) => language,
      None => return false,
    },
  };
  let value = matter.value.trim();
  let mut formatted = String::new();
  if !value.is_empty() {
    let saved = std::mem::replace(&mut printer.indents, 0);
    let result = printer.embed_text(Embedded {
      code: value,
      language,
      kind: Purpose::FrontMatter,
      extra: 0,
      module: true,
    });
    printer.indents = saved;
    match result {
      Some(text) => formatted = text,
      None => return false,
    }
  }
  let root = printer.docs.align(Align::Mark);
  printer.docs.owned(matter.start);
  printer.docs.owned(matter.explicit.unwrap_or_default());
  printer.docs.hardline();
  if !formatted.is_empty() {
    printer.text_doc(&formatted);
    printer.docs.hardline();
  }
  printer.docs.owned(matter.end);
  printer.docs.close(root);
  true
}

// ───────────────────────────── attributes ─────────────────────────────

/// What an attribute's value prints, worked out before anything is emitted.
enum Value<'v> {
  /// A text, as for `class`.
  Text(Cow<'v, [u8]>),
  Srcset {
    candidates: Vec<Candidate<'v>>,
    descriptors: Vec<String>,
    unit: &'static str,
  },
  /// The directives of `allow`.
  Policy(Vec<&'v [u8]>),
  /// The declarations of `style`, formatted.
  Style(String),
  /// The declarations of `style`, as `crate::css` formats them.
  Css(crate::css::Document),
  /// `printExpand` of formatted code.
  Expand(String),
  /// `group` of formatted code.
  Hug(String),
  /// `v-for`: the formatted aliases, `in` or `of`, and the formatted expression.
  For(String, &'static str, String),
}

/// `printAttribute` of `embed/attribute.js`. Emits nothing and returns false if no printer applies.
pub(crate) fn attribute(printer: &mut Printer<'_, '_>, id: Id, index: usize) -> bool {
  if !printer.options.formats_embedded {
    return false;
  }
  let tree = printer.tree;
  let attr = &tree.attrs(id)[index];
  let Some(value) = attr.value.filter(|value| !value.is_empty()) else { return false };
  let raw = raw_name(attr.raw_name());
  if super::utilities::should_unquote_attribute_value(attr, printer.options) {
    printer.bytes(&raw);
    printer.docs.text("=");
    printer.bytes(value);
    return true;
  }
  let full = attr.full_name();
  let is_candidate = matches!(&full[..], b"class" | b"srcset" | b"allow" | b"style")
    || is_event_handler(&full)
    || printer.options.parser == Parser::Vue;
  if !is_candidate {
    return false;
  }
  let embedded = !printer.options.embedded && !strings::contains(value, b"{{");
  let unescaped = unescape_quote_entities(value);
  let Some(plan) = value_of(printer, id, &full, &unescaped, embedded) else { return false };
  printer.bytes(&raw);
  printer.docs.text("=\"");
  let group = printer.docs.group();
  // `createAttributePrinter` escapes the double quotes in every text of the value.
  printer.quotes = true;
  emit_value(printer, plan);
  printer.quotes = false;
  printer.docs.close(group);
  printer.docs.text("\"");
  true
}

/// The printers of `embed/attribute.js` for one value, in their order.
fn value_of<'v>(
  printer: &mut Printer<'_, '_>,
  id: Id,
  full: &[u8],
  unescaped: &'v [u8],
  embedded: bool,
) -> Option<Value<'v>> {
  let tree = printer.tree;
  if full == b"class" && embedded {
    let joined = strings::collapse_js_whitespace(strings::trim_js_whitespace(unescaped));
    (!joined.is_empty()).then_some(Value::Text(joined))
  } else if full == b"srcset" && (tree[id].is_full_name(b"img") || tree[id].is_full_name(b"source"))
  {
    srcset(unescaped)
  } else if full == b"allow" && embedded && tree[id].is_full_name(b"iframe") {
    let directives = strings::split(unescaped, b";")
      .map(strings::trim_html_whitespace)
      .filter(|token| !token.is_empty())
      .collect();
    Some(Value::Policy(directives))
  } else if full == b"style" && embedded {
    let code = text(unescaped);
    let width = printer.options.width;
    let request = Request {
      code: &code,
      language: "css",
      kind: Purpose::Attribute,
      indent: 0,
      width,
      module: true,
    };
    match printer.embed.format(request) {
      Some(formatted) => Some(Value::Style(formatted)),
      None => style_sheet(printer, "css", &code, true).map(Value::Css),
    }
  } else if embedded && is_event_handler(full) {
    let code = text(unescaped);
    let embedded =
      Embedded { code: &code, language: "babel", kind: Purpose::Attribute, extra: 2, module: true };
    printer.embed_text(embedded).map(Value::Expand)
  } else if printer.options.parser == Parser::Vue {
    let code = text(unescaped);
    vue(printer, id, full, &code)
  } else {
    None
  }
}

fn emit_value(printer: &mut Printer<'_, '_>, value: Value<'_>) {
  match value {
    Value::Text(text) => printer.bytes(&text),
    Value::Srcset { candidates, descriptors, unit } => {
      let left = |descriptor: &str| descriptor.find('.').unwrap_or(descriptor.len());
      let max_url =
        candidates.iter().map(|it| strings::wtf8_len_utf16(it.url) as usize).max().unwrap_or(0);
      let max_left = descriptors.iter().map(|it| left(it)).max().unwrap_or(0);
      expand(printer, |printer| {
        for (index, (candidate, descriptor)) in candidates.iter().zip(&descriptors).enumerate() {
          if index > 0 {
            printer.docs.text(",");
            printer.docs.line();
          }
          printer.bytes(candidate.url);
          if descriptor.is_empty() {
            continue;
          }
          let padding = max_url - strings::wtf8_len_utf16(candidate.url) as usize + 1 + max_left
            - left(descriptor);
          let broken = printer.docs.if_break(Branch::Break, None);
          printer.docs.owned(&" ".repeat(padding));
          printer.docs.close(broken);
          let flat = printer.docs.if_break(Branch::Flat, None);
          printer.docs.text(" ");
          printer.docs.close(flat);
          printer.docs.owned(&format!("{descriptor}{unit}"));
        }
      });
    }
    Value::Policy(directives) => {
      if directives.is_empty() {
        return;
      }
      let count = directives.len();
      expand(printer, |printer| {
        for (index, directive) in directives.iter().enumerate() {
          let words: Vec<&[u8]> = html_split(directive).collect();
          printer.bytes(&words.join(&b' '));
          if index + 1 == count {
            let broken = printer.docs.if_break(Branch::Break, None);
            printer.docs.text(";");
            printer.docs.close(broken);
          } else {
            printer.docs.text(";");
            printer.docs.line();
          }
        }
      });
    }
    Value::Style(formatted) => expand(printer, |printer| style(printer, &formatted)),
    Value::Css(document) => expand(printer, |printer| document.write(printer.docs)),
    Value::Expand(formatted) => expand(printer, |printer| printer.text_doc(&formatted)),
    Value::Hug(formatted) => {
      let group = printer.docs.group();
      printer.text_doc(&formatted);
      printer.docs.close(group);
    }
    Value::For(left, operator, right) => {
      let outer = printer.docs.group();
      let inner = printer.docs.group();
      printer.text_doc(&left);
      printer.docs.close(inner);
      printer.docs.close(outer);
      printer.docs.text(" ");
      printer.docs.text(operator);
      printer.docs.text(" ");
      let group = printer.docs.group();
      printer.text_doc(&right);
      printer.docs.close(group);
    }
  }
}

/// The declarations of a `style` attribute, as Prettier's CSS printer writes them for one: joined by `line`, the
/// last one's semicolon only when the attribute breaks.
fn style(printer: &mut Printer<'_, '_>, formatted: &str) {
  let formatted = formatted.trim_end();
  let mut declarations: Vec<Vec<&str>> = Vec::new();
  for line in formatted.split('\n') {
    match declarations.last_mut() {
      Some(lines) if line.starts_with([' ', '\t']) || line.is_empty() => lines.push(line),
      _ => declarations.push(vec![line]),
    }
  }
  let count = declarations.len();
  for (index, lines) in declarations.into_iter().enumerate() {
    if index > 0 {
      printer.docs.line();
    }
    let last = index + 1 == count;
    for (at, line) in lines.iter().enumerate() {
      if at > 0 {
        printer.docs.hardline();
      }
      let end = last && at + 1 == lines.len();
      match line.strip_suffix(';').filter(|_| end) {
        Some(line) => {
          printer.bytes(line.as_bytes());
          let broken = printer.docs.if_break(Branch::Break, None);
          printer.docs.text(";");
          printer.docs.close(broken);
        }
        None => printer.bytes(line.as_bytes()),
      }
    }
  }
}

/// `printExpand`: `[indent([softline, doc]), softline]`
fn expand(printer: &mut Printer<'_, '_>, doc: impl FnOnce(&mut Printer<'_, '_>)) {
  let indent = printer.docs.indent();
  printer.docs.softline();
  doc(printer);
  printer.docs.close(indent);
  printer.docs.softline();
}

/// `@prettier/html-event-attributes`
pub(crate) fn is_event_handler(name: &[u8]) -> bool {
  name.len() > 2 && name.starts_with(b"on") && EVENTS.binary_search(&&name[2..]).is_ok()
}

/// The names of event attributes without `on`, from `@prettier/html-event-attributes` 0.0.2.
const EVENTS: &[&[u8]] = &[
  b"abort",
  b"afterprint",
  b"auxclick",
  b"beforeinput",
  b"beforematch",
  b"beforeprint",
  b"beforetoggle",
  b"beforeunload",
  b"blur",
  b"cancel",
  b"canplay",
  b"canplaythrough",
  b"change",
  b"click",
  b"close",
  b"command",
  b"contextlost",
  b"contextmenu",
  b"contextrestored",
  b"copy",
  b"cuechange",
  b"cut",
  b"dblclick",
  b"drag",
  b"dragend",
  b"dragenter",
  b"dragleave",
  b"dragover",
  b"dragstart",
  b"drop",
  b"durationchange",
  b"emptied",
  b"ended",
  b"error",
  b"focus",
  b"formdata",
  b"hashchange",
  b"input",
  b"invalid",
  b"keydown",
  b"keypress",
  b"keyup",
  b"languagechange",
  b"load",
  b"loadeddata",
  b"loadedmetadata",
  b"loadstart",
  b"message",
  b"messageerror",
  b"mousedown",
  b"mouseenter",
  b"mouseleave",
  b"mousemove",
  b"mouseout",
  b"mouseover",
  b"mouseup",
  b"offline",
  b"online",
  b"pagehide",
  b"pagereveal",
  b"pageshow",
  b"pageswap",
  b"paste",
  b"pause",
  b"play",
  b"playing",
  b"popstate",
  b"progress",
  b"ratechange",
  b"rejectionhandled",
  b"reset",
  b"resize",
  b"scroll",
  b"scrollend",
  b"securitypolicyviolation",
  b"seeked",
  b"seeking",
  b"select",
  b"slotchange",
  b"stalled",
  b"storage",
  b"submit",
  b"suspend",
  b"timeupdate",
  b"toggle",
  b"unhandledrejection",
  b"unload",
  b"volumechange",
  b"waiting",
  b"wheel",
];

// ───────────────────────────── `srcset` and `allow` ─────────────────────────────

/// A `srcset` candidate: the URL and its descriptors.
struct Candidate<'v> {
  url: &'v [u8],
  width: Option<f64>,
  density: Option<f64>,
  height: Option<f64>,
}

/// `@prettier/parse-srcset`. `None` where it throws.
fn parse_srcset(input: &[u8]) -> Option<Vec<Candidate<'_>>> {
  let is_space = |byte: u8| matches!(byte, b'\t' | b'\n' | 0x0C | b'\r' | b' ');
  let mut candidates = Vec::new();
  let mut position = 0;
  loop {
    position +=
      input[position..].iter().take_while(|&&byte| byte == b',' || is_space(byte)).count();
    if position >= input.len() {
      return Some(candidates).filter(|candidates| !candidates.is_empty());
    }
    let len = input[position..].iter().take_while(|&&byte| !is_space(byte)).count();
    let mut url = &input[position..position + len];
    position += len;
    let mut descriptors: Vec<&[u8]> = Vec::new();
    if url.ends_with(b",") {
      while let Some(shorter) = url.strip_suffix(b",") {
        url = shorter;
      }
    } else {
      position += input[position..].iter().take_while(|&&byte| is_space(byte)).count();
      let mut start = position;
      let mut in_parens = false;
      loop {
        let c = input.get(position).copied();
        match c {
          Some(b')') if in_parens => in_parens = false,
          Some(_) if in_parens => {}
          Some(b'(') => in_parens = true,
          Some(byte) if byte != b',' && !is_space(byte) => {}
          _ => {
            if position > start {
              descriptors.push(&input[start..position]);
            }
            if !c.is_some_and(is_space) {
              position += usize::from(c.is_some());
              break;
            }
            position += input[position..].iter().take_while(|&&byte| is_space(byte)).count();
            start = position;
            continue;
          }
        }
        position += 1;
      }
    }
    let mut candidate = Candidate { url, width: None, density: None, height: None };
    for descriptor in descriptors {
      let (&unit, value) = descriptor.split_last()?;
      let number = std::str::from_utf8(value).ok().and_then(|value| value.parse::<f64>().ok());
      let has = |value: Option<f64>| value.is_some_and(|value| value != 0.0);
      match unit {
        b'w' if is_non_negative_integer(value) => {
          if has(candidate.width) || has(candidate.density) {
            return None;
          }
          candidate.width = Some(number.filter(|&number| number != 0.0)?);
        }
        b'x' if is_floating_point(value) => {
          if has(candidate.width) || has(candidate.density) || has(candidate.height) {
            return None;
          }
          candidate.density = Some(number.filter(|&number| number >= 0.0)?);
        }
        b'h' if is_non_negative_integer(value) => {
          if has(candidate.height) || has(candidate.density) {
            return None;
          }
          candidate.height = Some(number.filter(|&number| number != 0.0)?);
        }
        _ => return None,
      }
    }
    candidate.density = candidate.density.filter(|&density| density != 0.0);
    candidates.push(candidate);
  }
}

/// `String(value)` for a non-negative number, or `None` where JavaScript writes an exponent.
fn number_to_string(value: f64) -> Option<String> {
  (value == 0.0 || (1e-6..1e21).contains(&value)).then(|| value.to_string())
}

/// `/^\d+$/`
fn is_non_negative_integer(text: &[u8]) -> bool {
  !text.is_empty() && text.iter().all(u8::is_ascii_digit)
}

/// `/^-?(?:[0-9]+|[0-9]*\.[0-9]+)(?:[eE][+-]?[0-9]+)?$/`
fn is_floating_point(text: &[u8]) -> bool {
  let digits = |text: &[u8]| text.iter().take_while(|byte| byte.is_ascii_digit()).count();
  let rest = text.strip_prefix(b"-").unwrap_or(text);
  let whole = digits(rest);
  let mut rest = match rest[whole..].strip_prefix(b".") {
    Some(fraction) if digits(fraction) > 0 => &fraction[digits(fraction)..],
    Some(_) => return false,
    None if whole == 0 => return false,
    None => &rest[whole..],
  };
  if let [b'e' | b'E', exponent @ ..] = rest {
    let exponent =
      exponent.strip_prefix(b"+").or_else(|| exponent.strip_prefix(b"-")).unwrap_or(exponent);
    if digits(exponent) == 0 {
      return false;
    }
    rest = &exponent[digits(exponent)..];
  }
  rest.is_empty()
}

/// `printSrcset`
fn srcset(value: &[u8]) -> Option<Value<'_>> {
  let candidates = parse_srcset(value)?;
  type Get = fn(&Candidate<'_>) -> Option<f64>;
  let kinds: [(Get, &'static str); 3] =
    [(|it| it.width, "w"), (|it| it.height, "h"), (|it| it.density, "x")];
  let mut used = kinds.iter().filter(|(get, _)| candidates.iter().any(|it| get(it).is_some()));
  let (kind, None) = (used.next(), used.next()) else { return None };
  let mut descriptors = Vec::with_capacity(candidates.len());
  for candidate in &candidates {
    descriptors.push(match kind.and_then(|(get, _)| get(candidate)) {
      Some(number) => number_to_string(number)?,
      None => String::new(),
    });
  }
  let unit = kind.map_or("", |(_, unit)| *unit);
  Some(Value::Srcset { candidates, descriptors, unit })
}

// ───────────────────────────── Vue ─────────────────────────────

/// `embed/vue-attributes.js`: the value of a directive or binding of a Vue file.
fn vue<'v>(printer: &mut Printer<'_, '_>, id: Id, name: &[u8], code: &str) -> Option<Value<'v>> {
  let ts = printer.options.typescript;
  let tree = printer.tree;
  if name == b"v-for" {
    return v_for(printer, code);
  }
  if name == b"generic" && tree.is_vue_script_tag(id, printer.options) {
    return expression(printer, code, "__vue_generic", Hug::Literal);
  }
  let is_slot = name.starts_with(b"#")
    || name == b"slot-scope"
    || name == b"v-slot"
    || name.starts_with(b"v-slot:");
  let is_bindings = tree.is_vue_sfc_block(id, printer.options)
    && ((tree[id].is_full_name(b"script") && name == b"setup")
      || (tree[id].is_full_name(b"style") && name == b"vars"));
  if is_slot || is_bindings {
    let language = if ts { "__vue_ts_bindings" } else { "__vue_bindings" };
    return expression(printer, code, language, Hug::Always);
  }
  let script = if ts { "__ts_expression" } else { "__js_expression" };
  if name.starts_with(b"@") || name.starts_with(b"v-on:") {
    let binding = if ts { "__vue_ts_event_binding" } else { "__vue_event_binding" };
    return expression(printer, code, script, Hug::Literal)
      .or_else(|| expression(printer, code, binding, Hug::Literal));
  }
  if name.starts_with(b":") || name.starts_with(b".") || name.starts_with(b"v-bind:") {
    let language = if ts { "__vue_ts_expression" } else { "__vue_expression" };
    return expression(printer, code, language, Hug::Literal);
  }
  if name.starts_with(b"v-") {
    return expression(printer, code, script, Hug::Literal);
  }
  None
}

/// When `formatAttributeValue` hugs the formatted value instead of expanding it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hug {
  /// Without `shouldHugJsExpression`: always.
  Always,
  /// `shouldHugJsExpression`: an object, an array, or, for Vue's own parsers, a string or a template literal.
  Literal,
}

/// `formatAttributeValue`
fn expression<'v>(
  printer: &mut Printer<'_, '_>,
  code: &str,
  language: &str,
  hug: Hug,
) -> Option<Value<'v>> {
  let request = |extra| Embedded { code, language, kind: Purpose::Attribute, extra, module: true };
  if hug == Hug::Always {
    return printer.embed_text(request(1)).map(Value::Hug);
  }
  let expanded = printer.embed_text(request(2))?;
  let strings = matches!(language, "__vue_expression" | "__vue_ts_expression");
  if !is_literal(expanded.trim(), strings) {
    return Some(Value::Expand(expanded));
  }
  printer.embed_text(request(1)).map(Value::Hug)
}

/// Whether `text`, a formatted expression, is one object or array literal, or with `strings`, one string or template
/// literal: what `shouldHugJsExpression` looks for in the tree.
fn is_literal(text: &str, strings: bool) -> bool {
  let bytes = text.as_bytes();
  let Some(&first) = bytes.first() else { return false };
  let close = match first {
    b'{' => b'}',
    b'[' => b']',
    b'"' | b'\'' | b'`' if strings => first,
    _ => return false,
  };
  match end_of_literal(bytes, close) {
    Some(end) => end + 1 == bytes.len(),
    None => false,
  }
}

/// The index of the bracket or quote that closes the one at the start of `bytes`.
fn end_of_literal(bytes: &[u8], close: u8) -> Option<usize> {
  if matches!(close, b'"' | b'\'' | b'`') {
    let mut at = 1;
    while at < bytes.len() {
      match bytes[at] {
        b'\\' => at += 2,
        byte if byte == close => return Some(at),
        _ => at += 1,
      }
    }
    return None;
  }
  let mut depth = 0usize;
  let mut at = 0;
  while at < bytes.len() {
    match bytes[at] {
      b'{' | b'[' | b'(' => depth += 1,
      b'}' | b']' | b')' => {
        depth -= 1;
        if depth == 0 {
          return (bytes[at] == close).then_some(at);
        }
      }
      quote @ (b'"' | b'\'' | b'`') => at += end_of_literal(&bytes[at..], quote)?,
      b'/' if bytes.get(at + 1) == Some(&b'/') => {
        at += bytes[at..].iter().position(|&byte| byte == b'\n')?;
      }
      b'/' if bytes.get(at + 1) == Some(&b'*') => {
        at += 2 + crate::strings::index_of(&bytes[at + 2..], b"*/")? + 1;
      }
      _ => {}
    }
    at += 1;
  }
  None
}

/// `printVueVForDirective`
fn v_for<'v>(printer: &mut Printer<'_, '_>, code: &str) -> Option<Value<'v>> {
  let (left, operator, right) = parse_v_for(code)?;
  let ts = printer.options.typescript;
  let language = if ts { "__vue_ts_for_left" } else { "__vue_for_left" };
  let left = printer.embed_text(Embedded {
    code: &left,
    language,
    kind: Purpose::Attribute,
    extra: 1,
    module: true,
  })?;
  let script = if ts { "__ts_expression" } else { "__js_expression" };
  let right = printer.embed_text(Embedded {
    code: right,
    language: script,
    kind: Purpose::Attribute,
    extra: 1,
    module: true,
  })?;
  Some(Value::For(left, operator, right))
}

/// `parseVueVForDirective`: the aliases, `in` or `of`, and the expression.
fn parse_v_for(value: &str) -> Option<(String, &'static str, &str)> {
  let (alias, operator, right) = split_v_for(value)?;
  let right = right.trim();
  if right.is_empty() {
    return None;
  }
  let alias = alias.trim();
  let alias = alias.strip_prefix('(').unwrap_or(alias);
  let alias = alias.strip_suffix(')').unwrap_or(alias);
  let parts: Vec<&str> = match iterators(alias) {
    Some((alias, first, second)) => {
      [Some(alias), Some(first.trim()), second.map(str::trim)].into_iter().flatten().collect()
    }
    None => vec![alias],
  };
  let mut slots = parts.clone();
  slots.resize(3, "");
  if slots.iter().enumerate().any(|(index, part)| {
    part.is_empty() && (index == 0 || slots[index + 1..].iter().any(|part| !part.is_empty()))
  }) {
    return None;
  }
  let left = parts.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(",");
  Some((left, operator, right))
}

/// `/(.*?)\s+(in|of)\s+(.*)/s`
fn split_v_for(value: &str) -> Option<(&str, &'static str, &str)> {
  let bytes = value.as_bytes();
  for at in 0..bytes.len() {
    if !bytes[at].is_ascii_whitespace() && !value[at..].starts_with(char::is_whitespace) {
      continue;
    }
    let before = &value[..at];
    let rest = value[at..].trim_start();
    for operator in ["in", "of"] {
      if let Some(after) = rest.strip_prefix(operator)
        && after.starts_with(char::is_whitespace)
      {
        return Some((before, operator, after.trim_start()));
      }
    }
  }
  None
}

/// `/,([^,\]}]*)(?:,([^,\]}]*))?$/` on the aliases.
fn iterators(alias: &str) -> Option<(&str, &str, Option<&str>)> {
  let allowed = |text: &str| !text.contains([',', ']', '}']);
  let last = alias.rfind(',')?;
  let tail = &alias[last + 1..];
  if !allowed(tail) {
    return None;
  }
  let head = &alias[..last];
  if let Some(second) = head.rfind(',')
    && allowed(&head[second + 1..])
  {
    return Some((&alias[..second], &head[second + 1..], Some(tail)));
  }
  Some((head, tail, None))
}
