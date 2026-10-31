//! Prettier's `embed.js` and `embed/*`: code of other languages, through the host's [`crate::embed::Embed`].

use std::borrow::Cow;

use super::Parser;
use super::ast::{Flags, Id, Kind};
use super::print::{Printer, raw_name, replace};
use super::strings;
use super::utilities::{
  dedent_string, html_split, html_trim_preserve_indentation, is_script_like_tag,
  unescape_quote_entities,
};
use crate::doc::{Align, Doc, Docs};
use crate::embed::{Kind as Purpose, Request};
use crate::{front_matter, languages};

/// Code to hand to the host.
pub(crate) struct Embedded<'c> {
  code: &'c str,
  language: &'c str,
  kind: Purpose,
  /// Indents beyond the printer's own, where the code is printed in an `indent`.
  extra: usize,
}

impl<'c> Embedded<'c> {
  pub(crate) fn request(&self, indents: usize, options: &super::Options<'_>) -> Request<'c> {
    Request {
      code: self.code,
      language: self.language,
      kind: self.kind,
      indent: indents + self.extra,
      width: options.width,
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
pub(crate) fn element(printer: &mut Printer<'_, '_>, id: Id) -> Option<Doc> {
  let tree = printer.tree;
  let node = &tree[id];
  if is_script_like_tag(node, printer.options)
    || node.has(Flags::IS_SELF_CLOSING)
    || !tree.is_vue_non_html_block(id, printer.options)
  {
    return None;
  }
  let language = element_parser(printer, id)?;
  let tags = super::tag::Tags { tree, options: printer.options };
  let content = tags.node_content(id);
  let mut empty = strings::trim_js_whitespace(content).is_empty();
  let mut doc = Docs::EMPTY;
  if !empty {
    let code = text(html_trim_preserve_indentation(content));
    let formatted =
      printer.embedded(Embedded { code: &code, language, kind: Purpose::Block, extra: 0 })?;
    empty = formatted == Docs::EMPTY;
    doc = formatted;
  }
  let prefix = printer.bytes(&marker(&tags, |tags, out| tags.opening_tag_prefix(id, out)));
  let opening = printer.print_opening_tag(id);
  let opening = printer.docs.group(opening);
  let line = if empty { Docs::EMPTY } else { printer.docs.hardline() };
  let line_after = if empty { Docs::EMPTY } else { printer.docs.hardline() };
  let closing = printer.print_closing_tag(id);
  let suffix = printer.bytes(&marker(&tags, |tags, out| tags.closing_tag_suffix(id, out)));
  Some(printer.docs.concat(&[prefix, opening, line, doc, line_after, closing, suffix]))
}

/// The text in a `<script>` or a `<style>`.
pub(crate) fn script(printer: &mut Printer<'_, '_>, id: Id, parent: Id) -> Option<Doc> {
  let language = element_parser(printer, parent)?;
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
  let doc = printer.embedded(Embedded { code: &code, language, kind: Purpose::Block, extra: 0 })?;
  let tags = super::tag::Tags { tree, options: printer.options };
  let breaks = printer.docs.break_parent();
  let prefix = printer.bytes(&marker(&tags, |tags, out| tags.opening_tag_prefix(id, out)));
  let suffix = printer.bytes(&marker(&tags, |tags, out| tags.closing_tag_suffix(id, out)));
  Some(printer.docs.concat(&[breaks, prefix, doc, suffix]))
}

/// The expression in `{{ }}`.
pub(crate) fn interpolation(printer: &mut Printer<'_, '_>, id: Id, parent: Id) -> Option<Doc> {
  let tree = printer.tree;
  let language = match printer.options.parser {
    Parser::Vue if printer.options.typescript => "__vue_ts_expression",
    Parser::Vue => "__vue_expression",
    _ => "__js_expression",
  };
  let code = text(&tree[id].value).into_owned();
  let doc =
    printer.embedded(Embedded { code: &code, language, kind: Purpose::Expression, extra: 1 })?;
  let line = printer.docs.line();
  let inner = printer.docs.concat(&[line, doc]);
  let indented = printer.docs.indent(inner);
  let tags = super::tag::Tags { tree, options: printer.options };
  let after =
    if tree.next(parent).is_some_and(|next| tags.needs_to_borrow_prev_closing_tag_end_marker(next))
    {
      printer.docs.text(" ")
    } else {
      printer.docs.line()
    };
  Some(printer.docs.concat(&[indented, after]))
}

/// `printEmbedFrontMatter`
pub(crate) fn front_matter(printer: &mut Printer<'_, '_>, raw: &[u8]) -> Option<Doc> {
  let raw = std::str::from_utf8(raw).ok()?;
  let matter = front_matter::parse(raw)?;
  let language = match matter.language {
    "yaml" => "yaml",
    "toml" => return None,
    other => languages::parser(other)?,
  };
  let value = matter.value.trim();
  let doc = if value.is_empty() {
    Docs::EMPTY
  } else {
    let saved = std::mem::replace(&mut printer.indents, 0);
    let doc =
      printer.embedded(Embedded { code: value, language, kind: Purpose::FrontMatter, extra: 0 });
    printer.indents = saved;
    doc?
  };
  let start = printer.docs.owned(matter.start);
  let explicit = printer.docs.owned(matter.explicit.unwrap_or_default());
  let hardline = printer.docs.hardline();
  let after = if doc == Docs::EMPTY { Docs::EMPTY } else { printer.docs.hardline() };
  let end = printer.docs.owned(matter.end);
  let inner = printer.docs.concat(&[start, explicit, hardline, doc, after, end]);
  Some(printer.docs.align(Align::Mark, inner))
}

fn marker(
  tags: &super::tag::Tags<'_, '_, '_>,
  write: impl FnOnce(super::tag::Tags<'_, '_, '_>, &mut Vec<u8>),
) -> Vec<u8> {
  let mut out = Vec::new();
  write(*tags, &mut out);
  out
}

// ───────────────────────────── attributes ─────────────────────────────

/// `printAttribute` of `embed/attribute.js`.
pub(crate) fn attribute(printer: &mut Printer<'_, '_>, id: Id, index: usize) -> Option<Doc> {
  let tree = printer.tree;
  let attr = &tree.attrs(id)[index];
  let value = attr.value.filter(|value| !value.is_empty())?;
  let raw = raw_name(attr.raw_name());
  if super::utilities::should_unquote_attribute_value(attr, printer.options) {
    let name = printer.bytes(&raw);
    let equals = printer.docs.text("=");
    let value = printer.bytes(value);
    return Some(printer.docs.concat(&[name, equals, value]));
  }
  let full = attr.full_name();
  let is_candidate = matches!(&full[..], b"class" | b"srcset" | b"allow" | b"style")
    || is_event_handler(&full)
    || printer.options.parser == Parser::Vue;
  if !is_candidate {
    return None;
  }
  let embedded = !printer.options.embedded && !strings::contains(value, b"{{");
  let unescaped = unescape_quote_entities(value);
  // `createAttributePrinter` escapes the double quotes in every text of the value.
  printer.quotes = true;
  let doc = value_doc(printer, id, &full, &unescaped, embedded);
  printer.quotes = false;
  let doc = doc?;
  let name = printer.bytes(&raw);
  let open = printer.docs.text("=\"");
  let group = printer.docs.group(doc);
  let close = printer.docs.text("\"");
  Some(printer.docs.concat(&[name, open, group, close]))
}

/// The printers of `embed/attribute.js` for one value, in their order.
fn value_doc(
  printer: &mut Printer<'_, '_>,
  id: Id,
  full: &[u8],
  unescaped: &[u8],
  embedded: bool,
) -> Option<Doc> {
  let tree = printer.tree;
  if full == b"class" && embedded {
    let joined = strings::collapse_js_whitespace(strings::trim_js_whitespace(unescaped));
    if joined.is_empty() { None } else { Some(printer.bytes(&joined)) }
  } else if full == b"srcset" && (tree[id].is_full_name(b"img") || tree[id].is_full_name(b"source"))
  {
    srcset(printer, unescaped)
  } else if full == b"allow" && embedded && tree[id].is_full_name(b"iframe") {
    Some(permissions_policy(printer, unescaped))
  } else if full == b"style" && embedded {
    let code = text(unescaped);
    let doc = style(printer, &code)?;
    Some(expand(printer, doc))
  } else if embedded && is_event_handler(full) {
    let code = text(unescaped);
    let doc = printer.embedded(Embedded {
      code: &code,
      language: "babel",
      kind: Purpose::Attribute,
      extra: 2,
    })?;
    Some(expand(printer, doc))
  } else if printer.options.parser == Parser::Vue {
    let code = text(unescaped);
    vue(printer, id, full, &code)
  } else {
    None
  }
}

/// The declarations of a `style` attribute, as Prettier's CSS printer writes them for one: joined by `line`, the
/// last one's semicolon only when the attribute breaks.
fn style(printer: &mut Printer<'_, '_>, code: &str) -> Option<Doc> {
  let width = printer.options.width;
  let request = Request { code, language: "css", kind: Purpose::Attribute, indent: 0, width };
  let formatted = printer.embed.format(request)?;
  let formatted = formatted.trim_end();
  let mut declarations: Vec<Vec<&str>> = Vec::new();
  for line in formatted.split('\n') {
    match declarations.last_mut() {
      Some(lines) if line.starts_with([' ', '\t']) || line.is_empty() => lines.push(line),
      _ => declarations.push(vec![line]),
    }
  }
  let count = declarations.len();
  let mut parts = Vec::with_capacity(count * 2);
  for (index, lines) in declarations.into_iter().enumerate() {
    if index > 0 {
      parts.push(printer.docs.line());
    }
    let last = index + 1 == count;
    for (at, line) in lines.iter().enumerate() {
      if at > 0 {
        parts.push(printer.docs.hardline());
      }
      let end = last && at + 1 == lines.len();
      match line.strip_suffix(';').filter(|_| end) {
        Some(line) => {
          parts.push(printer.bytes(line.as_bytes()));
          let semicolon = printer.docs.text(";");
          parts.push(printer.docs.if_break(semicolon, Docs::EMPTY, None));
        }
        None => parts.push(printer.bytes(line.as_bytes())),
      }
    }
  }
  Some(printer.docs.concat(&parts))
}

/// `printExpand`
fn expand(printer: &mut Printer<'_, '_>, doc: Doc) -> Doc {
  let soft = printer.docs.softline();
  let inner = printer.docs.concat(&[soft, doc]);
  let indented = printer.docs.indent(inner);
  let after = printer.docs.softline();
  printer.docs.concat(&[indented, after])
}

/// `@prettier/html-event-attributes`
fn is_event_handler(name: &[u8]) -> bool {
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
fn srcset(printer: &mut Printer<'_, '_>, value: &[u8]) -> Option<Doc> {
  let candidates = parse_srcset(value)?;
  type Get = fn(&Candidate<'_>) -> Option<f64>;
  let kinds: [(Get, &str); 3] =
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
  let left = |descriptor: &str| descriptor.find('.').unwrap_or(descriptor.len());
  let max_url =
    candidates.iter().map(|it| strings::wtf8_len_utf16(it.url) as usize).max().unwrap_or(0);
  let max_left = descriptors.iter().map(|it| left(it)).max().unwrap_or(0);
  let mut parts = Vec::new();
  for (index, (candidate, descriptor)) in candidates.iter().zip(&descriptors).enumerate() {
    if index > 0 {
      parts.push(printer.docs.text(","));
      parts.push(printer.docs.line());
    }
    parts.push(printer.bytes(candidate.url));
    if descriptor.is_empty() {
      continue;
    }
    let padding =
      max_url - strings::wtf8_len_utf16(candidate.url) as usize + 1 + max_left - left(descriptor);
    let aligned = printer.docs.owned(&" ".repeat(padding));
    let space = printer.docs.text(" ");
    parts.push(printer.docs.if_break(aligned, space, None));
    let unit = kind.map_or("", |(_, unit)| *unit);
    parts.push(printer.docs.owned(&format!("{descriptor}{unit}")));
  }
  let doc = printer.docs.concat(&parts);
  Some(expand(printer, doc))
}

/// `printPermissionsPolicy`
fn permissions_policy(printer: &mut Printer<'_, '_>, value: &[u8]) -> Doc {
  let directives: Vec<&[u8]> = strings::split(value, b";")
    .map(strings::trim_html_whitespace)
    .filter(|token| !token.is_empty())
    .collect();
  if directives.is_empty() {
    return Docs::EMPTY;
  }
  let count = directives.len();
  let mut parts = Vec::new();
  for (index, directive) in directives.into_iter().enumerate() {
    let words: Vec<&[u8]> = html_split(directive).collect();
    parts.push(printer.bytes(&words.join(&b' ')));
    if index + 1 == count {
      let semicolon = printer.docs.text(";");
      parts.push(printer.docs.if_break(semicolon, Docs::EMPTY, None));
    } else {
      parts.push(printer.docs.text(";"));
      parts.push(printer.docs.line());
    }
  }
  let doc = printer.docs.concat(&parts);
  expand(printer, doc)
}

// ───────────────────────────── Vue ─────────────────────────────

/// `embed/vue-attributes.js`: the value of a directive or binding of a Vue file.
fn vue(printer: &mut Printer<'_, '_>, id: Id, name: &[u8], code: &str) -> Option<Doc> {
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
fn expression(printer: &mut Printer<'_, '_>, code: &str, language: &str, hug: Hug) -> Option<Doc> {
  if hug == Hug::Always {
    let doc = printer.embedded(Embedded { code, language, kind: Purpose::Attribute, extra: 1 })?;
    return Some(printer.docs.group(doc));
  }
  let request = |extra| Embedded { code, language, kind: Purpose::Attribute, extra };
  let expanded = printer.embed_text(request(2))?;
  let strings = matches!(language, "__vue_expression" | "__vue_ts_expression");
  if !is_literal(expanded.trim(), strings) {
    let doc = printer.text_doc(&expanded);
    return Some(expand(printer, doc));
  }
  let doc = printer.embedded(request(1))?;
  Some(printer.docs.group(doc))
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
        at += 2 + super::strings::index_of(&bytes[at + 2..], b"*/")? + 1;
      }
      _ => {}
    }
    at += 1;
  }
  None
}

/// `printVueVForDirective`
fn v_for(printer: &mut Printer<'_, '_>, code: &str) -> Option<Doc> {
  let (left, operator, right) = parse_v_for(code)?;
  let ts = printer.options.typescript;
  let language = if ts { "__vue_ts_for_left" } else { "__vue_for_left" };
  let left = expression(printer, &left, language, Hug::Always)?;
  let left = printer.docs.group(left);
  let script = if ts { "__ts_expression" } else { "__js_expression" };
  let right = expression(printer, right, script, Hug::Always)?;
  let space = printer.docs.text(" ");
  let operator = printer.docs.text(operator);
  Some(printer.docs.concat(&[left, space, operator, space, right]))
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
