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
  // JSON
  CommentInJson = "Comments are not allowed in JSON",
  DuplicateProto = "Redefinition of __proto__ property",
  ArrayHole = "An array can't have holes",
  InvalidEscape = "Invalid escape sequence",
  InvalidNumber = "Invalid number",
  NotJson = "This is not allowed in JSON",
  // TOML
  ExpectedBrace = "Expected \"{\" or \"}\"",
  ExpectedBracket = "Expected \"[\" or \"]\"",
  ExpectedComma = "Expected \",\"",
  ExpectedLineBreak = "Expected a line break",
  ExpectedValue = "Expected a value",
  InvalidCharacter = "Invalid character",
  LineBreakInInlineTable = "An inline table cannot have line breaks",
  // YAML
  AliasWithProperties = "An alias cannot have a tag or an anchor",
  BadAnchor = "An anchor or alias needs a name",
  BadDirective = "Invalid directive",
  BadIndent = "Bad indentation",
  BadScalarStart = "A plain scalar cannot start with this character",
  BadTag = "Invalid tag",
  BlockAfterDocumentStart = "A block collection cannot start on the line of ---",
  BlockInFlow = "A block collection cannot be in a flow collection",
  CommentWithoutSpace = "A comment needs white space before it",
  ExpectedDocumentStart = "Expected ---",
  KeyTooLong = "An implicit key must be shorter than 1024 characters",
  MissingNewlineAfterProps = "Expected a line break after the tag or anchor of a block sequence",
  MultilineImplicitKey = "An implicit key must be on one line",
  MultipleAnchors = "A node can have one anchor",
  MultipleTags = "A node can have one tag",
  PropertyWithoutSpace = "A tag or anchor needs white space after it",
  TabIndent = "Tabs are not allowed as indentation",
  // GraphQL
  EmptyExtension = "This extension is empty",
  ExpectedClosingBracket = "Expected \"]\"",
  ExpectedDefinition = "Expected a definition",
  ExpectedDigit = "Expected a digit",
  ExpectedDirectiveLocation = "Expected a directive location",
  ExpectedOpeningBrace = "Expected \"{\"",
  // XML
  ExpectedClose = "Expected \">\"",
  ExpectedDeclarationEnd = "Expected \"?>\"",
  ExpectedElement = "Expected an element",
  ExpectedEndOfInput = "Expected the end of the file",
  ExpectedEndTag = "Expected an end tag",
  ExpectedEquals = "Expected \"=\"",
  ExpectedExternalId = "Expected SYSTEM or PUBLIC",
  ExpectedString = "Expected a quoted value",
  ExpectedTagEnd = "Expected \">\" or \"/>\"",
}
