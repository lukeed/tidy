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
  let embedded = !printer.options.embedded && !strings::contains(value, b"{{");
  let unescaped = unescape_quote_entities(value);
  let full = attr.full_name();
  let doc = if &full[..] == b"class" && embedded {
    let words: Vec<&[u8]> = html_split(strings::trim_js_whitespace(&unescaped)).collect();
    let joined = words.join(&b' ');
    Some(printer.bytes(&joined))
  } else if &full[..] == b"style" && embedded {
    let code = text(&unescaped).into_owned();
    let doc = printer.embedded(Embedded {
      code: &code,
      language: "css",
      kind: Purpose::Attribute,
      extra: 1,
    })?;
    Some(expand(printer, doc))
  } else if embedded && is_event_handler(&full) {
    let code = text(&unescaped).into_owned();
    let doc = printer.embedded(Embedded {
      code: &code,
      language: "babel",
      kind: Purpose::Attribute,
      extra: 1,
    })?;
    Some(expand(printer, doc))
  } else {
    None
  }?;
  let doc = escape_quotes(printer, doc);
  let name = printer.bytes(&raw);
  let open = printer.docs.text("=\"");
  let group = printer.docs.group(doc);
  let close = printer.docs.text("\"");
  Some(printer.docs.concat(&[name, open, group, close]))
}

/// `printExpand`
fn expand(printer: &mut Printer<'_, '_>, doc: Doc) -> Doc {
  let soft = printer.docs.softline();
  let inner = printer.docs.concat(&[soft, doc]);
  let indented = printer.docs.indent(inner);
  let after = printer.docs.softline();
  printer.docs.concat(&[indented, after])
}

/// `mapDoc(valueDoc, doc => doc.replaceAll('"', "&quot;"))`
fn escape_quotes(printer: &mut Printer<'_, '_>, doc: Doc) -> Doc {
  printer.docs.map_text(doc, |text| match replace(text.as_bytes(), b"\"", b"&quot;") {
    Cow::Borrowed(_) => None,
    Cow::Owned(bytes) => String::from_utf8(bytes).ok(),
  })
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
