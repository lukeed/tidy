//! Hooks that format the code of other languages in a template: scripts, style sheets, expressions.

/// What a piece of embedded code is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  /// The contents of a `<script>`, a `<style>`, or a block of a Vue file.
  Block,
  /// An expression in a template, as in `{{ value }}`.
  Expression,
  /// The value of an attribute that holds code, as in `style="..."` or `onclick="..."`.
  Attribute,
  /// The front matter of a file.
  FrontMatter,
}

/// A piece of code for the host to format.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
  pub code: &'a str,
  /// Prettier's name for the parser of the code: `babel`, `typescript`, `css`, `scss`, `json`, `yaml`, ...
  pub language: &'a str,
  pub kind: Kind,
  /// How many indents the code starts at. Lines after the first are indented by the host.
  pub indent: usize,
  /// The line width.
  pub width: usize,
  /// JavaScript that is a module, not a classic script: Prettier's `__babelSourceType`. Only a `<script>` in HTML is
  /// a classic script, unless its `type` makes it a module.
  pub module: bool,
}

/// Formats the code of another language. `None` leaves it as it is written.
pub trait Embed {
  fn format(&mut self, request: Request<'_>) -> Option<String>;
}

impl<F: FnMut(Request<'_>) -> Option<String>> Embed for F {
  fn format(&mut self, request: Request<'_>) -> Option<String> {
    self(request)
  }
}

/// Leaves all embedded code as it is written.
pub struct Verbatim;

impl Embed for Verbatim {
  fn format(&mut self, _: Request<'_>) -> Option<String> {
    None
  }
}
