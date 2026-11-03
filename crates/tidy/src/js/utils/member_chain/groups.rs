use super::chain_member::{CallExpressionPosition, ChainMember};
use crate::js::base::*;
use crate::js::print::program::is_next_line_empty;
use crate::js::write;

/// Prettier's `shouldInsertEmptyLineAfter`: true if the line after `e`, or after the `)` following it, is empty.
pub(super) fn should_insert_empty_line_after<'a>(e: Expr<'a>, f: &Formatter<'a>) -> bool {
  let (source_text, end) = (f.source_text(), e.span().end);
  let next = crate::js::ast::skip_trivia(source_text.as_bytes(), end);
  match source_text.byte_at(next) {
    Some(b')') => is_next_line_empty(source_text, next + 1),
    _ => is_next_line_empty(source_text, end),
  }
}

/// A group's links.
pub(super) struct FormatMemberChainGroup<'a, 'b>(pub(super) &'b [ChainMember<'a>]);

impl<'a> Format<'a> for FormatMemberChainGroup<'a, '_> {
  fn fmt(&self, f: &mut Formatter<'a>) {
    let mut members = self.0.iter().peekable();
    let mut is_at_line_start = true;
    while let Some(member) = members.next() {
      // `a.b⏎// comment⏎.c`: the comment comes directly after `.b`, where the next formatting finds it trailing `.b`
      // and prints a space before it. So does this.
      if !is_at_line_start
        && !f.is_quiet()
        && matches!(member, ChainMember::StaticMember(_) | ChainMember::ComputedMember(_))
        && member.leading_comments(f).first().is_some_and(|comment| comment.has_newline_before())
      {
        write!(f, space());
      }
      write!(f, member);
      is_at_line_start = false;
      // Prettier prints a line break after a call that's followed by an empty line, which makes an empty line at
      // the end of a group. It does that in the middle of a group too. oxfmt doesn't.
      if let ChainMember::CallExpression { expression, position: CallExpressionPosition::Middle } =
        *member
        && members.peek().is_some()
        && !f.options().flavor.is_oxfmt()
        && should_insert_empty_line_after(expression, f)
      {
        write!(f, hard_line_break());
        is_at_line_start = true;
      }
    }
  }
}
