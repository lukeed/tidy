//! Stands in for `jsdoc/` until it formats with tidy's Markdown: comments stay as they are written.

use crate::js::comments::Comment;
use crate::js::ir::formatter::Formatter;

pub(crate) fn write_comment<'a>(_: Comment<'_>, _: &mut Formatter<'a>) -> bool {
  false
}
