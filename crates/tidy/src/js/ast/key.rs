//! The name of a property, a member or a JSX attribute: bun's `Key` from `src/lint/ast/decl.rs` (MIT).

use super::{Expr, File, Name, Span, expression_id, skip_trivia};
use oxc_ast::ast::{Expression, JSXAttributeName, PropertyKey};
use oxc_span::GetSpan;

#[derive(Copy, Clone, Debug)]
pub(crate) struct Key<'a> {
  kind: KeyKind<'a>,
  /// The name without brackets.
  inner: Span,
  /// With the brackets of a computed name.
  outer: Span,
  /// True for a JSX attribute name.
  jsx: bool,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum KeyKind<'a> {
  /// `a`. In JSX also `a-b` and `a:b`.
  Ident(Name<'a>),
  /// `"a"`. The name is the string's value.
  String(Name<'a>),
  /// `0`, `1e3`, `0x10`, `1n`. The name is the number as `String(n)` formats it.
  Number(Name<'a>),
  /// `#a`. The name includes the `#`.
  Private(Name<'a>),
  /// `["a"]`, `` [`a`] ``. The name is the string's value.
  ComputedString(Name<'a>),
  /// `[0]`. The name is the number as `String(n)` formats it.
  ComputedNumber(Name<'a>),
  /// `[e]`, where `e` isn't a bare string or number.
  Computed(Expr<'a>),
}

impl<'a> Key<'a> {
  /// The key `key` of a property or a member, which is in brackets if `computed`.
  pub(crate) fn of(file: &'a File<'a>, key: &'a PropertyKey<'a>, computed: bool) -> Key<'a> {
    let inner = span(key.span());
    let kind = match key {
      PropertyKey::StaticIdentifier(it) => KeyKind::Ident(Name(it.name.as_str())),
      PropertyKey::PrivateIdentifier(_) => KeyKind::Private(Name(file.slice(inner))),
      _ => match key.as_expression().map(|it| literal(file, it)) {
        Some(Some(Literal::String(name))) if computed => KeyKind::ComputedString(name),
        Some(Some(Literal::Number(name))) if computed => KeyKind::ComputedNumber(name),
        Some(Some(Literal::String(name))) => KeyKind::String(name),
        Some(Some(Literal::Number(name))) => KeyKind::Number(name),
        _ => {
          KeyKind::Computed(Expr::new(file, key.as_expression().map_or(super::NONE, expression_id)))
        }
      },
    };
    let outer = match computed {
      true => {
        let text = file.text.as_bytes();
        let open =
          text[..inner.start as usize].iter().rposition(|&byte| byte == b'[').unwrap_or(0) as u32;
        let close = skip_trivia(text, outer_end(file, key, inner.end));
        Span::new(open, close + 1)
      }
      false => inner,
    };
    Key { kind, inner, outer, jsx: false }
  }

  /// A key that oxc doesn't store as a `PropertyKey`, without and with its brackets.
  pub(crate) fn new(kind: KeyKind<'a>, inner: Span, outer: Span) -> Key<'a> {
    Key { kind, inner, outer, jsx: false }
  }

  /// The name of a JSX attribute.
  pub(crate) fn jsx(file: &'a File<'a>, name: &'a JSXAttributeName<'a>) -> Key<'a> {
    let span = span(name.span());
    Key { kind: KeyKind::Ident(Name(file.slice(span))), inner: span, outer: span, jsx: true }
  }

  pub(crate) fn kind(self) -> KeyKind<'a> {
    self.kind
  }

  /// True if it's written in brackets.
  pub(crate) fn is_computed(self) -> bool {
    matches!(
      self.kind,
      KeyKind::Computed(_) | KeyKind::ComputedString(_) | KeyKind::ComputedNumber(_)
    )
  }

  pub(crate) fn is_private(self) -> bool {
    matches!(self.kind, KeyKind::Private(_))
  }

  pub(crate) fn is_jsx(self) -> bool {
    self.jsx
  }

  /// The property's name, or `None` if finding it would take evaluating an expression.
  pub(crate) fn name(self) -> Option<Name<'a>> {
    match self.kind {
      KeyKind::Ident(name)
      | KeyKind::String(name)
      | KeyKind::Number(name)
      | KeyKind::Private(name)
      | KeyKind::ComputedString(name)
      | KeyKind::ComputedNumber(name) => Some(name),
      KeyKind::Computed(_) => None,
    }
  }

  pub(crate) fn is(self, name: &str) -> bool {
    self.name().is_some_and(|it| it.is(name))
  }

  /// Excludes the brackets.
  pub(crate) fn inner_span(self, _: &File<'a>) -> Span {
    self.inner
  }

  /// Includes the brackets or quotes.
  pub(crate) fn span(self, _: &File<'a>) -> Span {
    self.outer
  }

  /// Where it starts, including a `[`.
  pub(crate) fn start(self) -> u32 {
    self.outer.start
  }
}

enum Literal<'a> {
  String(Name<'a>),
  Number(Name<'a>),
}

/// The value of a string or numeric literal key, or a template without substitutions.
fn literal<'a>(file: &'a File<'a>, expression: &'a Expression<'a>) -> Option<Literal<'a>> {
  match expression {
    Expression::StringLiteral(it) => {
      let text = file.slice(span(it.span));
      let quoted = text.get(1..text.len().saturating_sub(1)).unwrap_or_default();
      Some(Literal::String(Name(it.value.as_str().unwrap_or(quoted))))
    }
    Expression::TemplateLiteral(it) if it.expressions.is_empty() => {
      let cooked = it.quasis.first()?.value.cooked?.as_str()?;
      Some(Literal::String(Name(cooked)))
    }
    Expression::NumericLiteral(it) => {
      Some(Literal::Number(Name(file.keep(&number_string(it.value)))))
    }
    Expression::BigIntLiteral(it) => {
      let digits = it.value.as_str();
      Some(Literal::Number(Name(digits)))
    }
    _ => None,
  }
}

/// Where a computed key's expression ends, with the parentheses around it.
fn outer_end(file: &File, key: &PropertyKey, end: u32) -> u32 {
  match key.as_expression() {
    Some(expression) => {
      let parens = file.parens(expression_id(expression));
      parens.last().map_or(end, |it| it.1.end)
    }
    None => end,
  }
}

fn span(span: oxc_span::Span) -> Span {
  Span::new(span.start, span.end)
}

/// `String(value)` in JavaScript: the shortest digits that read back as `value`, with an exponent below `1e-6` and from
/// `1e21`.
pub(crate) fn number_string(value: f64) -> String {
  if value.is_nan() {
    return "NaN".into();
  }
  if value.is_infinite() {
    return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
  }
  if value == 0.0 || (1e-6..1e21).contains(&value.abs()) {
    return value.to_string();
  }
  let exponent = format!("{value:e}");
  match exponent.split_once('e') {
    Some((mantissa, power)) if !power.starts_with('-') => format!("{mantissa}e+{power}"),
    _ => exponent,
  }
}
