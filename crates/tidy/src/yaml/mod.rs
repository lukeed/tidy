//! YAML, formatted the way Prettier does, with ports of the parsers it uses: `yaml` 2.9.0, for the lexer, the
//! concrete syntax tree and the composer, and `yaml-unist-parser` 3.2.0, for the tree that Prettier prints.
//!
//! ```text
//! text ─ lexer ─→ lexemes ─ cst::Parser ─→ cst::Tree ─ compose::Composer ─→ Composition
//!      ─ transform::Transformer ─→ Root ─ attach ─→ Root with comments
//! ```

mod ast;
mod attach;
mod compose;
mod cst;
mod lexer;
mod scalar;
mod transform;

pub use ast::{Chomping, Id, Kind, Node, Point, Position, Root};

use crate::Error;
use crate::stack::Stack;
use crate::syntax::{Rejected, Rejection};

#[derive(Copy, Clone, Debug)]
enum Abort {
  Syntax(Rejected),
  TooDeep,
}

/// Parses `text` into the tree that Prettier prints, with each comment attached to a node.
pub fn parse(text: &str) -> Result<Root, Error> {
  let mut parser = Parser::default();
  parser.parse(text)?;
  Ok(parser.root)
}

/// The buffers of parsing, reused from one text to the next.
#[derive(Default)]
struct Parser {
  lexemes: Vec<lexer::Lexeme>,
  tree: cst::Tree,
  composition: compose::Composition,
  root: Root,
}

impl Parser {
  fn parse(&mut self, text: &str) -> Result<(), Error> {
    if text.len() > u32::MAX as usize {
      return Err(Error::Syntax { message: "The file is too large".into(), at: 0 });
    }
    self.lexemes.clear();
    self.tree.clear();
    self.composition.clear();
    self.root.clear();
    lexer::Lexer::lex(text.as_bytes(), &mut self.lexemes);
    cst::Parser::parse(text.as_bytes(), &self.lexemes, &mut self.tree);
    let rejection = Rejection::default();
    let stack = Stack::new();
    let mut composer =
      compose::Composer::new(text, &self.tree, &mut self.composition, &rejection, &stack);
    composer.compose().map_err(|abort| match abort {
      Abort::Syntax(_) => {
        let reason = rejection.reason();
        Error::Syntax { message: reason.0.text().to_owned(), at: reason.1 as usize }
      }
      Abort::TooDeep => Error::TooDeep,
    })?;
    let mut transformer = transform::Transformer {
      text,
      tree: &self.tree,
      composition: &self.composition,
      lines: ast::Lines::new(text),
      root: &mut self.root,
    };
    transformer.transform();
    attach::attach(&mut self.root);
    let root = self.root.root;
    attach::update_positions(&mut self.root, root);
    Ok(())
  }
}
