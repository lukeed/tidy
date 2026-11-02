//! Markdown in JavaScript templates: Prettier's `language-js/embed/markdown.js`.

use super::doc::{Texts, Writer};
use crate::doc::Docs;
use crate::embed::{Kind, Request};
use crate::js::base::*;
use crate::js::ir::element::{DedentMode, LineMode, TextWidth};

/// Prettier's `isEmbedMarkdown`, plus the checks `embed` runs on every template. `e` is a template.
fn is_candidate<'a>(e: Expr<'a>, template: Template<'a>, f: &Formatter<'a>) -> bool {
  matches!(f.options().embedded_language_formatting, EmbeddedLanguageFormatting::Auto)
    && template.quasi_count() == 1
    && matches!(e.ast_parent(), AstNodes::TaggedTemplateExpression(tagged)
            if matches!(tagged.kind(), ExprKind::TaggedTemplate(call)
                if matches!(call.callee().kind(), ExprKind::Ident(_)) && matches!(call.callee().text(), b"md" | b"markdown")))
    && template.cooked(0).is_some()
}

fn is_blank(template: Template<'_>) -> bool {
  crate::js::strings::trim_js_whitespace_start(template.raw(0)).is_empty()
}

/// True if `e` is a template with the tag `md` or `markdown` that's more than ` `` `. In Prettier its document then has
/// the label `embed`.
pub(crate) fn has_embed_label<'a>(e: Expr<'a>, f: &Formatter<'a>) -> bool {
  let ExprKind::TaggedTemplate(call) = e.kind() else {
    return false;
  };
  let Some((quasi, ExprKind::Template(template))) =
    call.template().map(|quasi| (quasi, quasi.kind()))
  else {
    return false;
  };
  is_candidate(quasi, template, f) && !is_blank(template)
}

/// Writes each text with Prettier's `escapeTemplateCharacters(doc, true)`: a backtick gets a backslash, and the
/// backslashes before it are doubled.
struct Escaped;

impl<'a> Texts<'a> for Escaped {
  fn write(&mut self, text: &str, width: u32, f: &mut Formatter<'a>) {
    if !text.contains('`') {
      return super::doc::write_string(text.as_bytes(), width, f);
    }
    let mut escaped = Vec::with_capacity(text.len() + 8);
    let mut backslashes = 0;
    for &byte in text.as_bytes() {
      if byte == b'`' {
        escaped.resize(escaped.len() + backslashes + 1, b'\\');
      }
      backslashes = if byte == b'\\' { backslashes + 1 } else { 0 };
      escaped.push(byte);
    }
    let width = TextWidth::single(f.string_width(&escaped));
    f.write_text(&escaped, Some(width));
  }
}

/// Writes the template `e` as Markdown, if Prettier treats it as Markdown. Returns true if it did.
pub(crate) fn write_template<'a>(
  e: Expr<'a>,
  template: Template<'a>,
  f: &mut Formatter<'a>,
) -> bool {
  if !is_candidate(e, template, f) {
    return false;
  }
  if is_blank(template) {
    f.write_token("``");
    return true;
  }

  // `\`` is a backtick, and the backslashes before it are escaped.
  let raw = template.raw(0);
  let mut text = Vec::with_capacity(raw.len());
  let mut backslashes = 0usize;
  for (index, &byte) in raw.iter().enumerate() {
    let byte = match byte {
      b'`' => {
        text.truncate(text.len() - backslashes.div_ceil(2));
        byte
      }
      b'\r' if raw.get(index + 1) == Some(&b'\n') => continue,
      b'\r' => b'\n',
      _ => byte,
    };
    backslashes = if byte == b'\\' { backslashes + 1 } else { 0 };
    text.push(byte);
  }

  // The indentation of the first non-blank line is stripped from all lines.
  let is_space_but_not_line_break = |byte: &&u8| byte.is_ascii_whitespace() && **byte != b'\n';
  let indentation = crate::js::strings::split(&text, b"\n")
    .find(|line| !line.iter().all(u8::is_ascii_whitespace))
    .map(|line| line[..line.iter().take_while(is_space_but_not_line_break).count()].to_vec())
    .unwrap_or_default();
  if !indentation.is_empty() {
    let mut dedented = Vec::with_capacity(text.len());
    for (index, line) in crate::js::strings::split(&text, b"\n").enumerate() {
      if index > 0 {
        dedented.push(b'\n');
      }
      dedented.extend_from_slice(line.strip_prefix(&indentation[..]).unwrap_or(line));
    }
    text = dedented;
  }

  let formatted = format(&String::from_utf8_lossy(&text), f.options());
  let bump = bumpalo::Bump::new();
  let mut docs = Docs::new(&bump);
  for (index, line) in formatted.trim_end_matches('\n').split('\n').enumerate() {
    if index > 0 {
      docs.hardline();
    }
    docs.text(line);
  }
  f.write_token("`");
  let mut writer = Writer::new(&docs, 1, true);
  if indentation.is_empty() {
    f.write_text(b"\n", Some(TextWidth::multiline(0)));
    f.write_element(FormatElement::Tag(Tag::StartDedent(DedentMode::Root)));
    writer.write(1, docs.len(), &mut Escaped, f);
    f.write_element(FormatElement::Tag(Tag::EndDedent(DedentMode::Root)));
  } else {
    f.write_element(FormatElement::Tag(Tag::StartIndent));
    f.write_element(FormatElement::Line(LineMode::Soft));
    writer.write(1, docs.len(), &mut Escaped, f);
    f.write_element(FormatElement::Tag(Tag::EndIndent));
  }
  f.write_element(FormatElement::Line(LineMode::Soft));
  f.write_token("`");
  true
}

/// Formats `text` as Markdown in a template, whose code blocks are fenced with `~`, and formats the code in them.
fn format(text: &str, options: &FormatOptions) -> String {
  let wrap = match options.prose_wrap {
    ProseWrap::Always => crate::Wrap::Always,
    ProseWrap::Never => crate::Wrap::Never,
    ProseWrap::Preserve => crate::Wrap::Preserve,
  };
  let markdown = crate::markdown::Options {
    width: options.line_width.value() as usize,
    wrap,
    fence: crate::markdown::Fence::Tilde,
    ..crate::markdown::Options::default()
  };
  let scripts = super::scripts(options);
  crate::markdown::format_with(text, &markdown, |code| {
    let language = crate::languages::parser(code.lang).or(crate::embed::own(code.lang))?;
    let request = Request {
      code: code.text,
      language,
      kind: Kind::Block,
      indent: 0,
      prefix: 0,
      suffix: 0,
      width: code.width,
      module: true,
    };
    let dialect = crate::css::Dialect::of_path(&format!("code.{language}"));
    match dialect {
      Some(dialect) => {
        let css = crate::css::Options {
          dialect,
          width: code.width,
          tab_width: scripts.tab_width,
          tabs: scripts.tabs,
          single_quote: scripts.single_quote,
          ..crate::css::Options::default()
        };
        crate::css::format(code.text, &css, &mut crate::embed::Verbatim).ok()
      }
      None => {
        crate::embed::builtin_in_markdown(&request, scripts.tab_width, scripts.tabs, &scripts)
      }
    }
  })
}
