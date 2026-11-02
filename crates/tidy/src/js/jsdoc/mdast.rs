//! A description's Markdown syntax tree, in the shape of [mdast](https://github.com/syntax-tree/mdast), which
//! prettier-plugin-jsdoc reads with mdast-util-from-markdown. It's built from pulldown-cmark's events, with CommonMark,
//! `~~strikethrough~~` and footnotes, as bun's `markdown::parse_plain` builds it.

use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};
use std::ops::Range;

pub(super) type NodeId = u32;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Kind {
  Root,
  Paragraph,
  Heading,
  ThematicBreak,
  Blockquote,
  List,
  ListItem,
  Html,
  Code,
  Definition,
  FootnoteDefinition,
  Text,
  Emphasis,
  Strong,
  Delete,
  InlineCode,
  Break,
  Link,
  Image,
  LinkReference,
  ImageReference,
  FootnoteReference,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum ReferenceType {
  Shortcut,
  Collapsed,
  Full,
}

/// A range of [`Tree::strings`], or `null`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) struct Str {
  start: u32,
  end: u32,
  null: bool,
}

impl Str {
  const NULL: Str = Str { start: 0, end: 0, null: true };

  pub(super) fn is_null(self) -> bool {
    self.null
  }
}

#[derive(Debug)]
pub(super) struct Node {
  pub(super) kind: Kind,
  /// A list's `ordered`.
  pub(super) ordered: bool,
  pub(super) reference_type: ReferenceType,
  /// A heading's `depth`, an ordered list's `start`.
  pub(super) number: u32,
  pub(super) start: u32,
  pub(super) end: u32,
  /// `value`, `url`, or a reference's `label`.
  pub(super) value: Str,
  /// `lang`, or `title`.
  pub(super) second: Str,
  /// `alt`, or a definition's `label`.
  pub(super) third: Str,
  pub(super) identifier: Str,
  children: Vec<NodeId>,
}

#[derive(Default)]
pub(super) struct Tree {
  nodes: Vec<Node>,
  strings: Vec<u8>,
}

impl Tree {
  pub(super) fn get(&self, id: NodeId) -> Option<&Node> {
    self.nodes.get(id as usize)
  }

  pub(super) fn kind(&self, id: NodeId) -> Option<Kind> {
    self.get(id).map(|node| node.kind)
  }

  pub(super) fn children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    self.get(id).map_or(&[][..], |node| &node.children).iter().copied()
  }

  pub(super) fn str(&self, string: Str) -> &[u8] {
    self.strings.get(string.start as usize..string.end as usize).unwrap_or_default()
  }

  fn keep(&mut self, text: &[u8]) -> Str {
    let start = self.strings.len() as u32;
    self.strings.extend_from_slice(text);
    Str { start, end: self.strings.len() as u32, null: false }
  }

  fn add(&mut self, kind: Kind, range: Range<usize>) -> NodeId {
    self.nodes.push(Node {
      kind,
      ordered: false,
      reference_type: ReferenceType::Shortcut,
      number: 0,
      start: range.start as u32,
      end: range.end as u32,
      value: Str::NULL,
      second: Str::NULL,
      third: Str::NULL,
      identifier: Str::NULL,
      children: Vec::new(),
    });
    (self.nodes.len() - 1) as NodeId
  }

  fn node(&mut self, id: NodeId) -> &mut Node {
    &mut self.nodes[id as usize]
  }
}

/// Fills `tree` with the syntax tree of `text`, a description. Returns the root, or `None` if `text` isn't UTF-8.
pub(super) fn parse(text: &[u8], tree: &mut Tree) -> Option<NodeId> {
  let text = std::str::from_utf8(text).ok()?;
  let options =
    Options::ENABLE_STRIKETHROUGH | Options::ENABLE_SUBSCRIPT | Options::ENABLE_FOOTNOTES;
  let events = Parser::new_ext(text, options).into_offset_iter();
  let mut definitions: Vec<Definition> = events
    .reference_definitions()
    .iter()
    .map(|(key, definition)| Definition {
      key: key.to_owned(),
      url: definition.dest.to_string(),
      span: definition.span.clone(),
    })
    .collect();
  definitions.sort_by_key(|definition| definition.span.start);
  let root = tree.add(Kind::Root, 0..text.len());
  let mut builder = Builder {
    text,
    tree,
    open: vec![root],
    paragraph: None,
    image: None,
    definitions: definitions.into_iter().peekable(),
  };
  for (event, range) in events {
    builder.event(event, range);
  }
  builder.close_paragraph();
  builder.flush_definitions(text.len());
  Some(root)
}

/// A link reference definition: its normalized label, its URL, and where it is.
struct Definition {
  key: String,
  url: String,
  span: Range<usize>,
}

struct Builder<'t> {
  text: &'t str,
  tree: &'t mut Tree,
  /// The open nodes, innermost last.
  open: Vec<NodeId>,
  /// The paragraph that a tight list item's text is put in, which pulldown-cmark leaves out.
  paragraph: Option<NodeId>,
  /// The image whose `alt` the text goes to.
  image: Option<NodeId>,
  /// The definitions not added yet, which pulldown-cmark has no events for.
  definitions: std::iter::Peekable<std::vec::IntoIter<Definition>>,
}

impl Builder<'_> {
  fn parent(&self) -> NodeId {
    self.open.last().copied().unwrap_or(0)
  }

  fn append(&mut self, kind: Kind, range: Range<usize>) -> NodeId {
    let parent = self.parent();
    let id = self.tree.add(kind, range);
    self.tree.node(parent).children.push(id);
    id
  }

  fn open(&mut self, kind: Kind, range: Range<usize>) -> NodeId {
    let id = self.append(kind, range);
    self.open.push(id);
    id
  }

  fn close_paragraph(&mut self) {
    if let Some(paragraph) = self.paragraph.take()
      && self.open.last() == Some(&paragraph)
    {
      self.open.pop();
    }
  }

  /// Adds the definitions that start before `end` to the open block.
  fn flush_definitions(&mut self, end: usize) {
    while let Some(definition) = self.definitions.next_if(|it| it.span.start < end) {
      let span = definition.span;
      let id = self.append(Kind::Definition, span.clone());
      let label = label_of(&self.text[span]);
      let (label, key, url) = (
        self.tree.keep(label.as_bytes()),
        self.tree.keep(definition.key.as_bytes()),
        self.tree.keep(definition.url.as_bytes()),
      );
      let node = self.tree.node(id);
      (node.third, node.identifier, node.value) = (label, key, url);
    }
  }

  /// Opens a paragraph for the inline content of a tight list item.
  fn inline(&mut self, range: &Range<usize>) {
    if let Some(paragraph) = self.paragraph {
      self.tree.node(paragraph).end = range.end as u32;
      return;
    }
    let parent = self.tree.kind(self.parent());
    if matches!(
      parent,
      Some(Kind::ListItem | Kind::Root | Kind::Blockquote | Kind::FootnoteDefinition)
    ) {
      self.paragraph = Some(self.open(Kind::Paragraph, range.clone()));
    }
  }

  fn leaf(&mut self, kind: Kind, range: Range<usize>, value: &[u8]) {
    if let Some(image) = self.image {
      let start = self.tree.node(image).third;
      let added = self.tree.keep(value);
      self.tree.node(image).third = Str { start: start.start, end: added.end, null: false };
      return;
    }
    self.inline(&range);
    let id = self.append(kind, range);
    let value = self.tree.keep(value);
    self.tree.node(id).value = value;
  }

  fn event(&mut self, event: Event<'_>, range: Range<usize>) {
    match event {
      Event::Start(tag) => self.start(tag, range),
      Event::End(tag) => self.end(tag, range),
      Event::Text(text) => match self.tree.kind(self.parent()) {
        Some(Kind::Code | Kind::Html) => {
          let id = self.parent();
          let value = self.tree.node(id).value;
          let added = self.tree.keep(text.as_bytes());
          let start = if value.is_null() { added.start } else { value.start };
          self.tree.node(id).value = Str { start, end: added.end, null: false };
        }
        _ => self.leaf(Kind::Text, range, text.as_bytes()),
      },
      Event::Code(code) => self.leaf(Kind::InlineCode, range, code.as_bytes()),
      Event::Html(html) => self.event(Event::Text(html), range),
      Event::InlineHtml(html) => self.leaf(Kind::Html, range, html.as_bytes()),
      Event::SoftBreak => self.leaf(Kind::Text, range, b"\n"),
      Event::HardBreak => self.leaf(Kind::Break, range, b""),
      Event::FootnoteReference(label) => {
        self.leaf(Kind::FootnoteReference, range, b"");
        if let Some(&id) = self.tree.node(self.parent()).children.last() {
          let label = self.tree.keep(label.as_bytes());
          self.tree.node(id).identifier = label;
        }
      }
      Event::Rule => {
        self.block(range.start);
        self.append(Kind::ThematicBreak, range);
      }
      Event::InlineMath(_) | Event::DisplayMath(_) | Event::TaskListMarker(_) => {}
    }
  }

  /// Ends the inline content of a tight list item before a block that starts at `start`.
  fn block(&mut self, start: usize) {
    self.close_paragraph();
    self.flush_definitions(start);
  }

  fn start(&mut self, tag: Tag<'_>, range: Range<usize>) {
    match tag {
      Tag::Paragraph => {
        self.block(range.start);
        self.open(Kind::Paragraph, range);
      }
      Tag::Heading { level, .. } => {
        self.block(range.start);
        let id = self.open(Kind::Heading, range);
        self.tree.node(id).number = level as u32;
      }
      Tag::BlockQuote(_) => {
        self.block(range.start);
        self.open(Kind::Blockquote, range);
      }
      Tag::CodeBlock(kind) => {
        self.block(range.start);
        let id = self.open(Kind::Code, range);
        if let CodeBlockKind::Fenced(info) = kind
          && let Some(lang) = info.split_whitespace().next()
        {
          let lang = self.tree.keep(lang.as_bytes());
          self.tree.node(id).second = lang;
        }
      }
      Tag::HtmlBlock => {
        self.block(range.start);
        self.open(Kind::Html, range);
      }
      Tag::List(start) => {
        self.block(range.start);
        let id = self.open(Kind::List, range);
        let node = self.tree.node(id);
        node.ordered = start.is_some();
        node.number = start.map_or(1, |start| u32::try_from(start).unwrap_or(u32::MAX));
      }
      Tag::Item => {
        self.block(range.start);
        self.open(Kind::ListItem, range);
      }
      Tag::FootnoteDefinition(label) => {
        self.block(range.start);
        let id = self.open(Kind::FootnoteDefinition, range);
        let label = self.tree.keep(label.as_bytes());
        self.tree.node(id).identifier = label;
      }
      Tag::Emphasis => self.open_inline(Kind::Emphasis, range),
      Tag::Strong => self.open_inline(Kind::Strong, range),
      // `~a~` isn't strikethrough for mdast-util-from-markdown.
      Tag::Subscript => self.leaf(Kind::Text, range, b"~"),
      Tag::Strikethrough => self.open_inline(Kind::Delete, range),
      Tag::Link { link_type, dest_url, title, id } => {
        let kind = reference_type(link_type).map_or(Kind::Link, |_| Kind::LinkReference);
        self.open_inline(kind, range);
        self.link(link_type, &dest_url, &title, &id);
      }
      Tag::Image { link_type, dest_url, title, id } => {
        let reference = reference_type(link_type);
        let kind = reference.map_or(Kind::Image, |_| Kind::ImageReference);
        self.inline(&range);
        let image = self.append(kind, range);
        let alt = self.tree.keep(b"");
        self.tree.node(image).third = alt;
        self.open.push(image);
        self.link(link_type, &dest_url, &title, &id);
        self.image = Some(image);
      }
      Tag::Table(_)
      | Tag::TableHead
      | Tag::TableRow
      | Tag::TableCell
      | Tag::Superscript
      | Tag::MetadataBlock(_)
      | Tag::DefinitionList
      | Tag::DefinitionListTitle
      | Tag::DefinitionListDefinition => {}
    }
  }

  fn open_inline(&mut self, kind: Kind, range: Range<usize>) {
    if self.image.is_some() {
      return;
    }
    self.inline(&range);
    self.open(kind, range);
  }

  /// Sets the `url` and `title`, or the reference's `label`, of the link or image just opened.
  fn link(&mut self, link_type: LinkType, url: &str, title: &str, label: &str) {
    let id = self.parent();
    match reference_type(link_type) {
      Some(reference) => {
        let label = self.tree.keep(label.as_bytes());
        let node = self.tree.node(id);
        node.reference_type = reference;
        (node.value, node.identifier) = (label, label);
      }
      None => {
        let url = match link_type {
          LinkType::Email => self.tree.keep(format!("mailto:{url}").as_bytes()),
          _ => self.tree.keep(url.as_bytes()),
        };
        let title = match title.is_empty() {
          true => Str::NULL,
          false => self.tree.keep(title.as_bytes()),
        };
        let node = self.tree.node(id);
        (node.value, node.second) = (url, title);
      }
    }
  }

  fn end(&mut self, tag: TagEnd, range: Range<usize>) {
    match tag {
      TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link
        if self.image.is_some() => {}
      TagEnd::Subscript => self.leaf(Kind::Text, range, b"~"),
      TagEnd::Image => {
        self.image = None;
        self.open.pop();
      }
      TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
        self.open.pop();
      }
      TagEnd::Paragraph | TagEnd::Heading(_) => {
        self.open.pop();
      }
      TagEnd::CodeBlock | TagEnd::HtmlBlock => {
        let id = self.parent();
        let value = self.tree.node(id).value;
        let trimmed = match value.is_null() {
          true => self.tree.keep(b""),
          false => {
            let text = self.tree.str(value);
            let end = value.end - u32::from(text.ends_with(b"\n"));
            Str { end, ..value }
          }
        };
        self.tree.node(id).value = trimmed;
        self.open.pop();
      }
      TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item | TagEnd::FootnoteDefinition => {
        self.block(range.end);
        self.open.pop();
      }
      _ => {}
    }
  }
}

fn reference_type(link_type: LinkType) -> Option<ReferenceType> {
  match link_type {
    LinkType::Reference | LinkType::ReferenceUnknown => Some(ReferenceType::Full),
    LinkType::Collapsed | LinkType::CollapsedUnknown => Some(ReferenceType::Collapsed),
    LinkType::Shortcut | LinkType::ShortcutUnknown => Some(ReferenceType::Shortcut),
    LinkType::Inline | LinkType::Autolink | LinkType::Email | LinkType::WikiLink { .. } => None,
  }
}

/// The label of the definition `[label]: url`, as written.
fn label_of(definition: &str) -> &str {
  let Some(open) = definition.find('[') else {
    return "";
  };
  let rest = &definition[open + 1..];
  let mut escaped = false;
  for (at, char) in rest.char_indices() {
    match char {
      ']' if !escaped => return &rest[..at],
      '\\' => escaped = !escaped,
      _ => escaped = false,
    }
  }
  rest
}
