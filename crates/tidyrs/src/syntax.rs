//! Syntax errors of the parsers: a message and an offset. Ported from bun's `syntax_error.rs` (MIT).

use std::cell::Cell;

/// A syntax error: its message, and the byte offset in the text.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct SyntaxError(pub(crate) Message, pub(crate) u32);

/// Where a parser records its syntax error. The error that it propagates, [`Rejected`], is zero-sized, so the success
/// path doesn't pay for carrying the reason.
pub(crate) struct Rejection(Cell<SyntaxError>);

impl Default for Rejection {
  fn default() -> Self {
    Rejection(Cell::new(SyntaxError(Message::UnexpectedToken, 0)))
  }
}

/// The input was rejected, and the reason was recorded in a [`Rejection`].
#[derive(Debug, Copy, Clone)]
pub(crate) struct Rejected(());

impl Rejection {
  #[cold]
  pub(crate) fn note(&self, message: Message, offset: u32) -> Rejected {
    self.0.set(SyntaxError(message, offset));
    Rejected(())
  }

  /// The most recently recorded error.
  #[cold]
  pub(crate) fn reason(&self) -> SyntaxError {
    self.0.get()
  }
}

macro_rules! messages {
  ($($name:ident = $text:literal,)*) => {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub(crate) enum Message {
      $($name,)*
    }

    impl Message {
      pub(crate) fn text(self) -> &'static str {
        match self {
          $(Message::$name => $text,)*
        }
      }
    }
  };
}

messages! {
  // Style sheets
  AtRuleWithoutName = "Expected a name after \"@\"",
  ExpectedColon = "Expected \":\"",
  NeitherDeclarationNorRule = "Expected a declaration or a rule",
  PropertyWithoutName = "Expected a property name",
  RuleWithoutSelector = "Expected a selector",
  UnclosedInterpolation = "Unclosed interpolation",
  UnexpectedClosingBrace = "Unexpected \"}\"",
  UnexpectedColon = "Unexpected \":\"",
  UnreadableAtRule = "Invalid at-rule parameters",
  UnreadableValue = "Invalid value",
  // Handlebars
  AttributeInEndTag = "An end tag cannot have attributes",
  EndTagOfVoidElement = "This element cannot have an end tag",
  EndTagWithoutStartTag = "This end tag has no start tag",
  ExpectedClosingParenthesis = "Expected \")\"",
  ExpectedEndOfMustache = "Expected \"}}\"",
  ExpectedName = "Expected a name",
  InvalidBlockParameters = "Invalid block parameters",
  InvalidDoctype = "Invalid doctype",
  InvalidPath = "\"..\", \".\" and \"this\" can only be at the start of a path",
  InvalidTagName = "Invalid element name",
  MisplacedBlock = "A block can only be inside an element or another block",
  MisplacedComment = "A comment is not allowed here",
  MisplacedBranch = "\"elif\" and \"else\" can only be in an \"if\" block, before its \"else\"",
  MisplacedMustache = "A mustache is not allowed here",
  SelfClosingEndTag = "An end tag cannot be self-closing",
  TooLarge = "The file is too large",
  UnclosedBlock = "Unclosed block",
  UnclosedBracket = "Unclosed bracket",
  UnclosedComment = "Unclosed comment",
  UnclosedElement = "Unclosed element",
  UnclosedString = "Unclosed string",
  UnexpectedCharacter = "Unexpected character",
  UnexpectedClose = "This closing tag has no open block",
  UnexpectedEnd = "Unexpected end of file",
  UnexpectedToken = "Unexpected token",
  UnquotedValueWithMustache = "An attribute value that mixes text and a mustache needs quotes",
  UnsupportedMustache = "Partials, decorators and raw blocks are not supported",
  WrongEndTag = "This end tag does not match the open element",
  WrongNameAtEndOfBlock = "This name does not match the open block",
}
