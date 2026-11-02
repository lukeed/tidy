//! Style sheets in JavaScript templates: Prettier's `language-js/embed/index.js` and `css.js`.

use super::doc::{Texts, Writer, each_text, write_string};
use crate::doc::Docs;
use crate::js::base::*;
use crate::js::ir::element::LineMode;
use crate::js::print::template::write_embedded_template_expression;
use crate::js::text;

const PLACEHOLDER: &[u8] = b"@prettier-placeholder-";

/// Finds the first match of `/@prettier-placeholder-(\d+)-id/` in `text`. Returns its start, its end and the number.
fn find_placeholder(text: &[u8]) -> Option<(usize, usize, usize)> {
  let mut from = 0;
  while let Some(start) = text::index_of_from(text, PLACEHOLDER, from) {
    let digits_start = start + PLACEHOLDER.len();
    let digits = text[digits_start..].iter().take_while(|b| b.is_ascii_digit()).count();
    let end = digits_start + digits;
    if digits > 0 && text[end..].starts_with(b"-id") {
      let number = text[digits_start..end].iter().fold(0usize, |number, digit| {
        number.saturating_mul(10).saturating_add(usize::from(digit - b'0'))
      });
      return Some((start, end + 3, number));
    }
    from = digits_start;
  }
  None
}

/// Counts the placeholders in the texts of `docs`. Returns `None` if a placeholder's number is `count` or more.
fn count_placeholders(docs: &Docs<'_>, count: usize) -> Option<usize> {
  let (mut found, mut is_valid) = (0, true);
  each_text(docs, |text| {
    let mut text = text.as_bytes();
    while let Some((_, end, number)) = find_placeholder(text) {
      is_valid &= number < count;
      found += 1;
      text = &text[end..];
    }
  });
  is_valid.then_some(found)
}

/// Writes the texts of a style sheet, with the substitutions of `template` in place of the placeholders.
struct Substitutions<'a> {
  template: Template<'a>,
  has_placeholders: bool,
}

impl<'a> Texts<'a> for Substitutions<'a> {
  fn write(&mut self, text: &str, width: u32, f: &mut Formatter<'a>) {
    let mut rest = text.as_bytes();
    if !self.has_placeholders || find_placeholder(rest).is_none() {
      return write_string(rest, width, f);
    }
    while let Some((start, end, number)) = find_placeholder(rest) {
      write_with_literal_lines(&rest[..start], f);
      write_embedded_template_expression(self.template, number, f);
      rest = &rest[end..];
    }
    write_with_literal_lines(rest, f);
  }
}

/// `replaceEndOfLine(text)`
fn write_with_literal_lines(text: &[u8], f: &mut Formatter<'_>) {
  if text.is_empty() {
    return;
  }
  f.write_text(text, None);
  if crate::js::strings::contains_char(text, b'\n') {
    f.write_element(FormatElement::ExpandParent);
  }
}

/// The options of a style sheet in a script.
fn sheet_options(options: &FormatOptions) -> crate::css::Options {
  crate::css::Options {
    dialect: crate::css::Dialect::Scss,
    width: options.line_width.value() as usize,
    tab_width: options.indent_width.value() as usize,
    tabs: options.indent_style == IndentStyle::Tab,
    single_quote: options.quote_style == QuoteStyle::Single,
    trailing_comma: options.trailing_commas != TrailingCommas::None,
    ..crate::css::Options::default()
  }
}

/// Prettier's `printEmbedCss`: calls `then` with the document of `template`'s text as SCSS, with placeholders for the
/// substitutions. Returns `None` if the text doesn't parse, or if some substitution isn't in the document exactly once.
fn with_sheet<'a, R>(
  template: Template<'a>,
  options: &FormatOptions,
  then: impl FnOnce(&Docs<'_>) -> R,
) -> Option<R> {
  let count = template.quasi_count().saturating_sub(1);
  let mut text = Vec::new();
  for index in 0..template.quasi_count() {
    if index > 0 {
      text.extend_from_slice(PLACEHOLDER);
      text.extend_from_slice((index - 1).to_string().as_bytes());
      text.extend_from_slice(b"-id");
    }
    text.extend_from_slice(template.raw(index));
  }
  let text = String::from_utf8_lossy(&text);
  let options = sheet_options(options);
  let sheet =
    crate::css::document(&text, &options, false, count > 0, &mut crate::embed::Verbatim).ok()?;
  let bump = bumpalo::Bump::new();
  let mut docs = Docs::new(&bump);
  sheet.write(&mut docs);
  let mut is_empty = true;
  each_text(&docs, |text| is_empty &= text.is_empty());
  if is_empty || (count > 0 && count_placeholders(&docs, count) != Some(count)) {
    return None;
  }
  Some(then(&docs))
}

fn is_identifier(e: Expr<'_>, name: &[u8]) -> bool {
  matches!(e.kind(), ExprKind::Ident(_)) && e.text() == name
}

/// Returns `e`'s object if `e` is a `MemberExpression`.
fn member_object(e: Expr<'_>) -> Option<Expr<'_>> {
  match e.as_ast_nodes() {
    AstNodes::StaticMemberExpression(_) | AstNodes::ComputedMemberExpression(_) => e.object(),
    _ => None,
  }
}

/// Returns `e`'s callee if `e` is a `CallExpression`.
fn call_callee(e: Expr<'_>) -> Option<Expr<'_>> {
  match e.as_ast_nodes() {
    AstNodes::CallExpression(_) => e.callee(),
    _ => None,
  }
}

/// `Component.extend`
fn is_styled_extend(e: Expr<'_>) -> bool {
  matches!(e.kind(), ExprKind::Dot { obj, name, .. }
        if member_object(e).is_some()
            && name.bytes() == b"extend"
            && matches!(obj.kind(), ExprKind::Ident(_))
            && obj.text().first().is_some_and(u8::is_ascii_uppercase))
}

fn is_styled_object(e: Expr<'_>) -> bool {
  member_object(e).is_some_and(|object| is_identifier(object, b"styled")) || is_styled_extend(e)
}

/// `isStyledComponents` and the first half of `isStyledJsx`. `tag` is the template's tag.
fn is_styled_tag(tag: Expr<'_>) -> bool {
  // css``, css.global``, css.resolve``
  if is_identifier(tag, b"css") {
    return true;
  }
  if let ExprKind::Dot { obj, name, .. } = tag.kind()
    && member_object(tag).is_some()
    && is_identifier(obj, b"css")
    && matches!(name.bytes(), b"global" | b"resolve")
  {
    return true;
  }
  // styled.foo``, Component.extend``
  if is_styled_object(tag) {
    return true;
  }
  let Some(callee) = call_callee(tag) else {
    return false;
  };
  // styled(Component)``
  is_identifier(callee, b"styled")
    || member_object(callee).is_some_and(|object| {
      // styled.foo.attrs({})``, Component.extend.attrs({})``, styled(Component).attrs({})``
      is_styled_object(object)
        || call_callee(object).is_some_and(|callee| is_identifier(callee, b"styled"))
    })
}

fn has_name(prop: Prop<'_>, name: &[u8]) -> bool {
  matches!(prop.key().map(|key| key.kind()), Some(KeyKind::Ident(key)) if key.bytes() == name)
}

/// `isAngularComponentStyles`: `@Component({ styles: [`..`] })`. `parent` is the template's parent.
fn is_angular_component_styles(parent: AstNodes<'_>) -> bool {
  let property = match parent {
    AstNodes::ArrayExpression(_) => parent.parent(),
    _ => parent,
  };
  is_angular_component_property(property, b"styles")
}

/// True if `property` is the property `name` of the object in `@Component({ .. })`.
pub(crate) fn is_angular_component_property(property: AstNodes<'_>, name: &[u8]) -> bool {
  let AstNodes::ObjectProperty(prop) = property else {
    return false;
  };
  let AstNodes::ObjectExpression(object) = property.parent() else {
    return false;
  };
  let call = property.parent().parent();
  // Compare the name last. The cheaper checks have already rejected nearly every property.
  matches!(call, AstNodes::CallExpression(call)
        if call.callee().is_some_and(|callee| callee != object && is_identifier(callee, b"Component")))
    && matches!(call.parent(), AstNodes::Decorator(_))
    && prop.kind() == PropKind::Init
    && has_name(prop, name)
}

/// oxc's `get_tag_name`: the identifier a tag starts with, like `a` in `a.b(c)[d]`, looking through TypeScript
/// wrappers. oxfmt picks the template's language from that name only.
pub(crate) fn root_name_of_tag(mut tag: Expr<'_>) -> Option<&[u8]> {
  loop {
    tag = match tag.kind() {
      ExprKind::Ident(_) => return Some(tag.text()),
      ExprKind::Dot { obj, .. } | ExprKind::Index { obj, .. } => obj,
      ExprKind::Call(call) => call.callee(),
      ExprKind::As { expr, .. } | ExprKind::Satisfies { expr, .. } => expr,
      ExprKind::AsConst(expr) | ExprKind::NonNull(expr) => expr,
      _ => return None,
    };
  }
}

/// Prettier's `isEmbedCss`. `e` is a template.
pub(crate) fn is_embed_css(e: Expr<'_>) -> bool {
  is_embed_css_for(e, Flavor::Prettier)
}

fn is_embed_css_for(e: Expr<'_>, flavor: Flavor) -> bool {
  let parent = e.ast_parent();
  match parent {
    AstNodes::TaggedTemplateExpression(tagged) => {
      matches!(tagged.kind(), ExprKind::TaggedTemplate(call) if call.callee() != e && match flavor.is_oxfmt() {
          true => matches!(root_name_of_tag(call.callee()), Some(b"css" | b"styled")),
          false => is_styled_tag(call.callee()),
      })
    }
    AstNodes::JSXExpressionContainer(_) => match parent.parent() {
      // <style jsx>{`div{color:red}`}</style>
      AstNodes::JSXElement(element) => matches!(element.kind(), ExprKind::Jsx(jsx)
                if jsx.tag().is_some_and(|tag| is_identifier(tag, b"style"))
                    && jsx.attrs().iter().any(|attribute| has_name(attribute, b"jsx"))),
      // <div css={`color: red`} />
      AstNodes::JSXAttribute(attribute) => has_name(attribute, b"css"),
      _ => false,
    },
    _ => is_angular_component_styles(parent),
  }
}

/// True if Prettier's `embed` would try to format the template `e`.
fn is_candidate<'a>(e: Expr<'a>, template: Template<'a>, options: &FormatOptions) -> bool {
  matches!(options.embedded_language_formatting, EmbeddedLanguageFormatting::Auto)
    && (0..template.quasi_count()).all(|index| template.cooked(index).is_some())
    && is_embed_css_for(e, options.flavor)
}

/// True if `template` is printed as ` `` `.
pub(crate) fn is_blank(template: Template<'_>) -> bool {
  template.quasi_count() == 1 && crate::js::strings::trim_js_whitespace(template.raw(0)).is_empty()
}

/// Prettier's `embed`, for the languages that have a formatter here. Returns true if it wrote the template `e`.
pub(crate) fn write_template<'a>(
  e: Expr<'a>,
  template: Template<'a>,
  f: &mut Formatter<'a>,
) -> bool {
  if !is_candidate(e, template, f.options()) {
    return false;
  }
  if is_blank(template) {
    f.write_token("``");
    return true;
  }
  let options = f.options().clone();
  let written = with_sheet(template, &options, |docs| {
    f.write_token("`");
    f.write_element(FormatElement::Tag(Tag::StartIndent));
    f.write_element(FormatElement::Line(LineMode::Hard));
    let has_placeholders = template.quasi_count() > 1;
    let mut texts = Substitutions { template, has_placeholders };
    Writer::new(docs, 1, true).write(1, docs.len(), &mut texts, f);
    f.write_element(FormatElement::Tag(Tag::EndIndent));
    f.write_element(FormatElement::Line(LineMode::Soft));
    f.write_token("`");
  });
  written.is_some()
}

/// True if `e`'s document has the label `embed`, which means `e` is a template, tagged or not, that's formatted as its
/// embedded language.
pub(crate) fn has_embed_label<'a>(e: Expr<'a>, f: &Formatter<'a>) -> bool {
  let quasi = match e.kind() {
    ExprKind::TaggedTemplate(call) => call.template(),
    _ => Some(e),
  };
  let Some((quasi, ExprKind::Template(template))) = quasi.map(|quasi| (quasi, quasi.kind())) else {
    return false;
  };
  is_candidate(quasi, template, f.options())
    && (tag_alone_decides_template_layout(f)
      || (!is_blank(template) && with_sheet(template, f.options(), |_| ()).is_some()))
}

/// oxc's `embed_hug`: oxfmt lays out a template with a language's tag in a call or after `=>` as if it were formatted
/// as that language, even when it doesn't parse and is printed verbatim.
pub(crate) fn tag_alone_decides_template_layout(f: &Formatter<'_>) -> bool {
  f.options().flavor.is_oxfmt()
}
