//! GraphQL in JavaScript templates: Prettier's `language-js/embed/graphql.js`.

use super::doc::{Texts, Writer};
use crate::doc::Docs;
use crate::js::base::*;
use crate::js::ir::element::{LineMode, TextWidth};
use crate::js::print::template::write_embedded_template_expression;
use smallvec::SmallVec;

fn is_identifier(e: Expr<'_>, names: &[&[u8]]) -> bool {
  matches!(e.kind(), ExprKind::Ident(_)) && names.contains(&e.text())
}

/// Returns the nodes whose leading comments can specify the language of the template `e`: the template itself, and an
/// expression statement or an `as const` directly around it.
fn nodes_with_language_comment<'a>(e: Expr<'a>, parent: AstNodes<'a>) -> [Option<AstNodes<'a>>; 2] {
  let includes_parent = match parent {
    AstNodes::ExpressionStatement(_) => true,
    AstNodes::TSAsExpression(cast) => matches!(cast.kind(), ExprKind::AsConst(_)),
    _ => false,
  };
  [Some(e.as_ast_nodes()), includes_parent.then_some(parent)]
}

/// True if a comment could specify the language of the template `e`. A cheap check before trying each embedded
/// language.
pub(crate) fn may_have_language_comment<'a>(e: Expr<'a>, f: &Formatter<'a>) -> bool {
  (nodes_with_language_comment(e, e.ast_parent()).into_iter().flatten())
    .any(|node| !f.comments().of_node(node).leading.is_empty())
}

/// Prettier's `hasLanguageComment`: `/* GraphQL */ `..``. `language` is the text between the `/*` and the `*/`.
pub(crate) fn has_language_comment<'a>(
  e: Expr<'a>,
  parent: AstNodes<'a>,
  language: &[u8],
  f: &Formatter<'a>,
) -> bool {
  (nodes_with_language_comment(e, parent).into_iter().flatten()).any(|node| {
    (f.comments().of_node(node).leading.iter())
      .any(|comment| comment.is_block() && comment.content() == language)
  })
}

/// Prettier's `isEmbedGraphQL`. `e` is a template.
pub(crate) fn is_embed_graphql<'a>(e: Expr<'a>, f: &Formatter<'a>) -> bool {
  let parent = e.ast_parent();
  let is_marked = match parent {
    AstNodes::TaggedTemplateExpression(tagged) => match tagged.kind() {
      ExprKind::TaggedTemplate(call) if f.options().flavor.is_oxfmt() => {
        matches!(crate::js::embed::css::root_name_of_tag(call.callee()), Some(b"gql" | b"graphql"))
      }
      ExprKind::TaggedTemplate(call) => match call.callee().kind() {
        ExprKind::Dot { obj, name, .. } => {
          matches!(call.callee().as_ast_nodes(), AstNodes::StaticMemberExpression(_))
            && is_identifier(obj, &[b"graphql"])
            && name.bytes() == b"experimental"
        }
        _ => is_identifier(call.callee(), &[b"gql", b"graphql"]),
      },
      _ => false,
    },
    AstNodes::CallExpression(call) => {
      call.callee().is_some_and(|callee| is_identifier(callee, &[b"graphql"]))
    }
    _ => false,
  };
  is_marked || has_language_comment(e, parent, b" GraphQL ", f)
}

/// The text between two substitutions.
struct Part<'a> {
  text: &'a str,
  content: Content,
  starts_with_blank_line: bool,
  ends_with_blank_line: bool,
}

enum Content {
  Nothing,
  Comments,
  Document(usize),
}

fn lines(text: &[u8]) -> impl DoubleEndedIterator<Item = &[u8]> + Clone {
  crate::js::strings::split(text, b"\n").collect::<SmallVec<[&[u8]; 16]>>().into_iter()
}

/// Returns `None` if Prettier leaves the template as it is. Adds the document of the part to `docs`.
fn parse_part<'a: 'x, 'x>(
  template: Template<'a>,
  index: usize,
  options: &crate::graphql::Options,
  bump: &'x bumpalo::Bump,
  docs: &mut Vec<Docs<'x>>,
) -> Option<Part<'a>> {
  let raw = template.raw(index);
  let raw_equals_cooked = crate::js::strings::index_of_any(raw, b"\\\r").is_none();
  let bytes = if raw_equals_cooked { raw } else { template.cooked(index)?.bytes() };
  // An escape can produce a lone surrogate, which isn't a valid GraphQL character.
  let text = std::str::from_utf8(bytes).ok()?;
  let is_last = index + 1 == template.quasi_count();

  let all = lines(bytes);
  // A substitution in a comment.
  if !is_last
    && all.clone().next_back().is_some_and(|line| crate::js::strings::contains_char(line, b'#'))
  {
    return None;
  }
  let count = all.clone().count();
  let has_only_comments =
    all.clone().all(|line| matches!(crate::js::strings::trim_js_whitespace(line), [] | [b'#', ..]));
  let content = if !has_only_comments {
    let mut document = Docs::new(bump);
    crate::graphql::template(text, options, &mut document).ok()?;
    docs.push(document);
    Content::Document(docs.len() - 1)
  } else if crate::js::strings::is_all_js_whitespace(bytes) {
    Content::Nothing
  } else {
    Content::Comments
  };
  Some(Part {
    text,
    content,
    starts_with_blank_line: count > 2
      && all.clone().take(2).all(crate::js::strings::is_all_js_whitespace),
    ends_with_blank_line: count > 2
      && all.rev().take(2).all(crate::js::strings::is_all_js_whitespace),
  })
}

fn is_candidate<'a>(e: Expr<'a>, template: Template<'a>, f: &Formatter<'a>) -> bool {
  matches!(f.options().embedded_language_formatting, EmbeddedLanguageFormatting::Auto)
    && is_embed_graphql(e, f)
    && (0..template.quasi_count()).all(|index| template.cooked(index).is_some())
}

fn is_blank_template(template: Template<'_>) -> bool {
  template.quasi_count() == 1 && crate::js::strings::is_all_js_whitespace(template.raw(0))
}

/// The options of GraphQL in a script.
fn graphql_options(options: &FormatOptions) -> crate::graphql::Options {
  crate::graphql::Options {
    width: options.line_width.value() as usize,
    tab_width: options.indent_width.value() as usize,
    tabs: options.indent_style == IndentStyle::Tab,
    bracket_spacing: options.bracket_spacing.value(),
    ..crate::graphql::Options::default()
  }
}

/// Calls `then` with the parts of `template` and the documents they refer to, unless Prettier leaves it as it is.
fn with_parts<'a, R>(
  template: Template<'a>,
  options: &FormatOptions,
  then: impl FnOnce(&[Part<'a>], &[Docs<'_>]) -> R,
) -> Option<R> {
  let options = graphql_options(options);
  let bump = bumpalo::Bump::new();
  let mut docs = Vec::new();
  let parts = (0..template.quasi_count())
    .map(|index| parse_part(template, index, &options, &bump, &mut docs))
    .collect::<Option<SmallVec<[Part<'a>; 2]>>>()?;
  Some(then(&parts, &docs))
}

/// True if `e` is a template, tagged or not, that's formatted as GraphQL and isn't just ` `` `. In Prettier, its
/// document then has the label `embed`.
pub(crate) fn has_embed_label<'a>(e: Expr<'a>, f: &Formatter<'a>) -> bool {
  let quasi = match e.kind() {
    ExprKind::TaggedTemplate(call) => call.template(),
    _ => Some(e),
  };
  let Some((quasi, ExprKind::Template(template))) = quasi.map(|quasi| (quasi, quasi.kind())) else {
    return false;
  };
  is_candidate(quasi, template, f)
    && (crate::js::embed::css::tag_alone_decides_template_layout(f)
      || (!is_blank_template(template) && with_parts(template, f.options(), |_, _| ()).is_some()))
}

/// The line breaks between the parts that Prettier's `printEmbedGraphQL` joins.
#[derive(Default)]
struct Separator {
  is_needed: bool,
  has_blank_line: bool,
}

impl Separator {
  fn write(&mut self, f: &mut Formatter<'_>) {
    if self.is_needed {
      let mode = if self.has_blank_line { LineMode::Empty } else { LineMode::Hard };
      f.write_element(FormatElement::Line(mode));
    }
    *self = Separator { is_needed: true, has_blank_line: false };
  }
}

/// Writes the template `e` as GraphQL if Prettier treats it as GraphQL. Returns true if it did.
pub(crate) fn write_template<'a>(
  e: Expr<'a>,
  template: Template<'a>,
  f: &mut Formatter<'a>,
) -> bool {
  if !is_candidate(e, template, f) {
    return false;
  }
  if is_blank_template(template) {
    f.write_token("``");
    return true;
  }
  let options = f.options().clone();
  let written = with_parts(template, &options, |parts, docs| write_parts(template, parts, docs, f));
  written.is_some()
}

/// Prettier's `printEmbedGraphQL`, once the parts parse.
fn write_parts<'a>(
  template: Template<'a>,
  parts: &[Part<'a>],
  docs: &[Docs<'_>],
  f: &mut Formatter<'a>,
) {
  f.write_token("`");
  f.write_element(FormatElement::Tag(Tag::StartIndent));
  f.write_element(FormatElement::Line(LineMode::Hard));
  let mut separator = Separator::default();
  for (index, part) in parts.iter().enumerate() {
    let (is_first, is_last) = (index == 0, index + 1 == parts.len());
    if matches!(part.content, Content::Nothing) {
      separator.has_blank_line |= !is_first && !is_last && part.starts_with_blank_line;
    } else {
      separator.has_blank_line |= !is_first && part.starts_with_blank_line;
      separator.write(f);
      match part.content {
        Content::Document(index) => {
          let docs = &docs[index];
          Writer::new(docs, 1, true).write(1, docs.len(), &mut Escaped, f)
        }
        _ => write_comment_lines(part.text, f),
      }
      separator.has_blank_line |= !is_last && part.ends_with_blank_line;
    }
    if !is_last {
      separator.write(f);
      write_embedded_template_expression(template, index, f);
    }
  }
  f.write_element(FormatElement::Tag(Tag::EndIndent));
  f.write_element(FormatElement::Line(LineMode::Hard));
  f.write_token("`");
}

/// Prettier's `printGraphqlComments`, for a text that's only comments and white space.
fn write_comment_lines(text: &str, f: &mut Formatter<'_>) {
  let (mut has_comment, mut is_after_blank_line) = (false, false);
  for line in text.split('\n') {
    let comment = line.trim_matches(|c: char| c.is_ascii_whitespace());
    if !comment.is_empty() {
      match (has_comment, is_after_blank_line) {
        (false, _) => {}
        (true, true) => f.write_element(FormatElement::Line(LineMode::Empty)),
        (true, false) => f.write_element(FormatElement::Line(LineMode::Hard)),
      }
      has_comment = true;
      let width = f.string_width(comment.as_bytes());
      Escaped.write(comment, width, f);
    }
    is_after_blank_line = comment.is_empty();
  }
}

/// Writes texts with Prettier's `uncookTemplateElementValue`: `\\`, `` ` `` and `${` get a backslash.
struct Escaped;

impl<'a> Texts<'a> for Escaped {
  fn write(&mut self, text: &str, width: u32, f: &mut Formatter<'a>) {
    let bytes = text.as_bytes();
    if crate::js::strings::index_of_any(bytes, b"\\`").is_none() && !text.contains("${") {
      return super::doc::write_string(bytes, width, f);
    }
    let mut escaped = Vec::with_capacity(bytes.len() + 8);
    for (index, &byte) in bytes.iter().enumerate() {
      if matches!(byte, b'\\' | b'`') || (byte == b'$' && bytes.get(index + 1) == Some(&b'{')) {
        escaped.push(b'\\');
      }
      escaped.push(byte);
    }
    let width = TextWidth::single(f.string_width(&escaped));
    f.write_text(&escaped, Some(width));
  }
}
