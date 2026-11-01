//! YAML, formatted the way Prettier does, with ports of the parsers it uses: `yaml` 2.9.0, for the lexer, the
//! concrete syntax tree and the composer, and `yaml-unist-parser` 3.2.0, for the tree that Prettier prints.

mod compose;
mod cst;
mod lexer;
mod scalar;

use crate::Error;
use crate::stack::Stack;
use crate::syntax::{Rejected, Rejection};

#[derive(Copy, Clone, Debug)]
enum Abort {
  Syntax(Rejected),
  TooDeep,
}

/// Whether Prettier's parser accepts `text`. For the oracle, until the formatter exists.
#[doc(hidden)]
pub fn validate(text: &str) -> Result<(), Error> {
  let mut lexemes = Vec::new();
  lexer::Lexer::lex(text.as_bytes(), &mut lexemes);
  let mut tree = cst::Tree::default();
  cst::Parser::parse(text.as_bytes(), &lexemes, &mut tree);
  let rejection = Rejection::default();
  let stack = Stack::new();
  let mut composition = compose::Composition::default();
  let mut composer = compose::Composer::new(text, &tree, &mut composition, &rejection, &stack);
  composer.compose().map_err(|abort| match abort {
    Abort::Syntax(_) => {
      let reason = rejection.reason();
      Error::Syntax { message: reason.0.text().to_owned(), at: reason.1 as usize }
    }
    Abort::TooDeep => Error::TooDeep,
  })
}
