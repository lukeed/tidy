//! Stands in for `embed/` until it formats with tidy's printers: Prettier with `embeddedLanguageFormatting: "off"`,
//! where no template is embedded code.

pub(crate) mod css {
  use crate::js::ast::{Expr, Template};
  use crate::js::ir::formatter::Formatter;

  pub(crate) fn write_template<'a>(_: Expr<'a>, _: Template<'a>, _: &mut Formatter<'a>) -> bool {
    false
  }

  pub(crate) fn has_embed_label<'a>(_: Expr<'a>, _: &Formatter<'a>) -> bool {
    false
  }
}

pub(crate) mod graphql {
  pub(crate) use super::css::{has_embed_label, write_template};
  use crate::js::ast::Expr;
  use crate::js::ir::formatter::Formatter;

  pub(crate) fn may_have_language_comment<'a>(_: Expr<'a>, _: &Formatter<'a>) -> bool {
    false
  }
}

pub(crate) mod markdown {
  pub(crate) use super::css::{has_embed_label, write_template};
}

pub(crate) mod html {
  pub(crate) use super::css::write_template;
  use crate::js::ast::Expr;
  use crate::js::ir::formatter::Formatter;

  #[derive(Copy, Clone, PartialEq, Eq, Debug)]
  pub(crate) enum Label {
    Embed,
    EmbedWithoutHug,
  }

  pub(crate) fn label<'a>(_: Expr<'a>, _: &Formatter<'a>) -> Option<Label> {
    None
  }

  pub(crate) fn is_placeholder_in_js(_: &[u8]) -> bool {
    false
  }
}
