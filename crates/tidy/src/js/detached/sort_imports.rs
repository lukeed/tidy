//! Stands in for `sort_imports/` until it builds on tidy: imports stay in their order.

use crate::js::ast::Stmt;
use crate::js::ir::formatter::Formatter;

pub(crate) struct ImportRun;

impl ImportRun {
  pub(crate) fn new(_: &Formatter<'_>) -> Self {
    ImportRun
  }

  pub(crate) fn before_separator<'a>(&mut self, _: Stmt<'a>, _: bool, _: &mut Formatter<'a>) {}

  pub(crate) fn before_statement<'a>(&mut self, _: Stmt<'a>, _: &mut Formatter<'a>) {}

  pub(crate) fn finish(&mut self, _: &mut Formatter<'_>) {}
}
