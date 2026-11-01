//! The tree of a document: the plugin's simplified CST, in flat lists. The content of an element is a range of
//! `Tree::items`, in source order.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
  pub(crate) start: u32,
  pub(crate) end: u32,
}

impl Span {
  pub(crate) fn of(self, text: &str) -> &str {
    &text[self.start as usize..self.end as usize]
  }
}

/// A range of one of the lists of the tree.
pub(crate) type Range = (u32, u32);

/// A part of the content of an element, or of the document.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Item {
  Element(u32),
  /// `chardata` with a `TEXT`.
  Text(Span),
  /// `chardata` with a `SEA_WS`.
  Space(Span),
  Reference(Span),
  CData(Span),
  Comment(Span),
  Instruction(Span),
}

impl Item {
  pub(crate) fn start(self, tree: &Tree) -> u32 {
    self.span(tree).start
  }

  pub(crate) fn span(self, tree: &Tree) -> Span {
    match self {
      Item::Element(id) => tree.elements[id as usize].span,
      Item::Text(span)
      | Item::Space(span)
      | Item::Reference(span)
      | Item::CData(span)
      | Item::Comment(span)
      | Item::Instruction(span) => span,
    }
  }

  pub(crate) fn is_chardata(self) -> bool {
    matches!(self, Item::Text(_) | Item::Space(_))
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Element {
  pub(crate) span: Span,
  pub(crate) name: Span,
  pub(crate) attributes: Range,
  /// `None` for `<a/>`.
  pub(crate) content: Option<Range>,
  /// The name in the end tag, which may not be the same.
  pub(crate) end_name: Span,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Attribute {
  pub(crate) name: Span,
  /// With its quotes.
  pub(crate) value: Span,
}

/// `<?xml ... ?>`
#[derive(Clone, Copy, Debug)]
pub(crate) struct Prolog {
  /// `<?xml` and the whitespace character after it.
  pub(crate) open: Span,
  pub(crate) attributes: Range,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Doctype {
  pub(crate) name: Span,
  pub(crate) external: Option<External>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum External {
  System { keyword: Span, system: Span },
  Public { keyword: Span, public: Span, system: Span },
}

/// A part of the document, outside of the root element.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Top {
  Prolog(Prolog),
  Doctype(Doctype),
  Item(Item),
}

#[derive(Default)]
pub(crate) struct Tree {
  pub(crate) items: Vec<Item>,
  pub(crate) elements: Vec<Element>,
  pub(crate) attributes: Vec<Attribute>,
  pub(crate) top: Vec<Top>,
  /// The offset of each `\n`.
  pub(crate) breaks: Vec<u32>,
}

impl Tree {
  pub(crate) fn clear(&mut self) {
    self.items.clear();
    self.elements.clear();
    self.attributes.clear();
    self.top.clear();
    self.breaks.clear();
  }

  pub(crate) fn content(&self, range: Range) -> &[Item] {
    &self.items[range.0 as usize..range.1 as usize]
  }

  pub(crate) fn attributes(&self, range: Range) -> &[Attribute] {
    &self.attributes[range.0 as usize..range.1 as usize]
  }

  /// The line that the character at `offset` is on, from 1, as chevrotain counts it: a `\n` is on the line it ends.
  pub(crate) fn line(&self, offset: u32) -> usize {
    self.breaks.partition_point(|&at| at < offset) + 1
  }

  /// chevrotain's `startLine` and `endLine` of a span: the lines of its first and last characters.
  pub(crate) fn lines(&self, span: Span) -> (usize, usize) {
    (self.line(span.start), self.line(span.end.saturating_sub(1).max(span.start)))
  }
}
