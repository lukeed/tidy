//! The concrete syntax tree of `yaml` 2.9.0 (`src/parse/cst.ts`, `parser.ts`), built from the lexemes the way its
//! `Parser` builds it, in an arena. Its nodes keep every source token, so the text can be printed back from them.

use super::lexer::Lexeme;

/// The type of a source token: a lexeme that the parser keeps as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
  ByteOrderMark,
  DocMode,
  DocStart,
  DocEnd,
  Space,
  Comment,
  Newline,
  DirectiveLine,
  Anchor,
  Tag,
  SeqItemInd,
  ExplicitKeyInd,
  MapValueInd,
  FlowMapStart,
  FlowMapEnd,
  FlowSeqStart,
  FlowSeqEnd,
  FlowErrorEnd,
  Comma,
  BlockScalarHeader,
  Alias,
  Scalar,
  SingleQuotedScalar,
  DoubleQuotedScalar,
}

/// A lexeme the parser keeps: its type, where it is, and the indent of its line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Source {
  pub(crate) kind: Kind,
  pub(crate) offset: u32,
  pub(crate) end: u32,
  pub(crate) indent: u32,
}

pub(crate) type Id = u32;

/// The key of an item: none, the empty key of `: value`, or a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Key {
  #[default]
  Absent,
  Null,
  Token(Id),
}

/// An item of a collection.
#[derive(Clone, Debug, Default)]
pub(crate) struct Item {
  pub(crate) start: Vec<Source>,
  pub(crate) explicit_key: bool,
  pub(crate) key: Key,
  pub(crate) sep: Option<Vec<Source>>,
  pub(crate) value: Option<Id>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScalarKind {
  Alias,
  Plain,
  Single,
  Double,
}

/// A node of the tree: `yaml`'s `Token`.
#[derive(Clone, Debug)]
pub(crate) enum Node {
  /// A byte order mark, space, comment or line break outside documents.
  Source(Source),
  Error {
    offset: u32,
    end: u32,
    message: &'static str,
  },
  Directive {
    offset: u32,
    end: u32,
  },
  Document {
    offset: u32,
    start: Vec<Source>,
    value: Option<Id>,
    end: Option<Vec<Source>>,
  },
  DocumentEnd {
    offset: u32,
    source_end: u32,
    end: Option<Vec<Source>>,
  },
  FlowScalar {
    kind: ScalarKind,
    offset: u32,
    source_end: u32,
    indent: u32,
    end: Option<Vec<Source>>,
  },
  BlockScalar {
    offset: u32,
    indent: u32,
    props: Vec<Prop>,
    source: (u32, u32),
  },
  BlockMap {
    offset: u32,
    indent: u32,
    items: Vec<Item>,
  },
  BlockSeq {
    offset: u32,
    indent: u32,
    items: Vec<Item>,
  },
  FlowCollection {
    offset: u32,
    indent: u32,
    start: Source,
    items: Vec<Item>,
    end: Vec<Source>,
  },
}

/// What a block scalar's `props` hold: its header and the tokens after it, or an error token.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Prop {
  Source(Source),
  Token(Id),
}

impl Node {
  pub(crate) fn offset(&self) -> u32 {
    match *self {
      Node::Source(source) => source.offset,
      Node::Error { offset, .. }
      | Node::Directive { offset, .. }
      | Node::Document { offset, .. }
      | Node::DocumentEnd { offset, .. }
      | Node::FlowScalar { offset, .. }
      | Node::BlockScalar { offset, .. }
      | Node::BlockMap { offset, .. }
      | Node::BlockSeq { offset, .. }
      | Node::FlowCollection { offset, .. } => offset,
    }
  }

  fn indent(&self) -> Option<u32> {
    match *self {
      Node::FlowScalar { indent, .. }
      | Node::BlockScalar { indent, .. }
      | Node::BlockMap { indent, .. }
      | Node::BlockSeq { indent, .. }
      | Node::FlowCollection { indent, .. } => Some(indent),
      _ => None,
    }
  }

  fn set_indent(&mut self, value: u32) {
    if let Node::FlowScalar { indent, .. }
    | Node::BlockScalar { indent, .. }
    | Node::BlockMap { indent, .. }
    | Node::BlockSeq { indent, .. }
    | Node::FlowCollection { indent, .. } = self
    {
      *indent = value;
    }
  }

  fn items(&self) -> Option<&Vec<Item>> {
    match self {
      Node::BlockMap { items, .. }
      | Node::BlockSeq { items, .. }
      | Node::FlowCollection { items, .. } => Some(items),
      _ => None,
    }
  }

  fn items_mut(&mut self) -> Option<&mut Vec<Item>> {
    match self {
      Node::BlockMap { items, .. }
      | Node::BlockSeq { items, .. }
      | Node::FlowCollection { items, .. } => Some(items),
      _ => None,
    }
  }

  /// `end`, of the tokens that have one: documents, flow scalars and flow collections.
  fn end_mut(&mut self) -> Option<&mut Option<Vec<Source>>> {
    match self {
      Node::Document { end, .. } | Node::DocumentEnd { end, .. } | Node::FlowScalar { end, .. } => {
        Some(end)
      }
      _ => None,
    }
  }
}

/// The parsed stream: the nodes, and the top-level tokens in order.
#[derive(Clone, Debug, Default)]
pub(crate) struct Tree {
  pub(crate) nodes: Vec<Node>,
  pub(crate) tokens: Vec<Id>,
}

impl Tree {
  pub(crate) fn clear(&mut self) {
    self.nodes.clear();
    self.tokens.clear();
  }

  fn add(&mut self, node: Node) -> Id {
    self.nodes.push(node);
    self.nodes.len() as Id - 1
  }

  pub(crate) fn node(&self, id: Id) -> &Node {
    &self.nodes[id as usize]
  }
}

/// `tokenType`
fn token_type(text: &[u8]) -> Option<Kind> {
  Some(match text {
    [0xEF, 0xBB, 0xBF] => Kind::ByteOrderMark,
    b"---" => Kind::DocStart,
    b"..." => Kind::DocEnd,
    b"" | b"\n" | b"\r\n" => Kind::Newline,
    b"-" => Kind::SeqItemInd,
    b"?" => Kind::ExplicitKeyInd,
    b":" => Kind::MapValueInd,
    b"{" => Kind::FlowMapStart,
    b"}" => Kind::FlowMapEnd,
    b"[" => Kind::FlowSeqStart,
    b"]" => Kind::FlowSeqEnd,
    b"," => Kind::Comma,
    _ => match text.first()? {
      b' ' | b'\t' => Kind::Space,
      b'#' => Kind::Comment,
      b'%' => Kind::DirectiveLine,
      b'*' => Kind::Alias,
      b'&' => Kind::Anchor,
      b'!' => Kind::Tag,
      b'\'' => Kind::SingleQuotedScalar,
      b'"' => Kind::DoubleQuotedScalar,
      b'|' | b'>' => Kind::BlockScalarHeader,
      _ => return None,
    },
  })
}

fn includes(list: &[Source], kind: Kind) -> bool {
  list.iter().any(|it| it.kind == kind)
}

/// `findNonEmptyIndex`
fn find_non_empty(list: &[Source]) -> Option<usize> {
  list.iter().position(|it| !matches!(it.kind, Kind::Space | Kind::Comment | Kind::Newline))
}

/// `getFirstKeyStartProps`: removes the props of the key at the end of `prev`, and returns them.
fn first_key_start_props(prev: &mut Vec<Source>) -> Vec<Source> {
  if prev.is_empty() {
    return Vec::new();
  }
  let mut i = prev.len() as isize;
  loop {
    i -= 1;
    if i < 0 {
      break;
    }
    if matches!(
      prev[i as usize].kind,
      Kind::DocStart | Kind::ExplicitKeyInd | Kind::MapValueInd | Kind::SeqItemInd | Kind::Newline
    ) {
      break;
    }
  }
  let mut i = (i + 1) as usize;
  while prev.get(i).is_some_and(|it| it.kind == Kind::Space) {
    i += 1;
  }
  prev.split_off(i.min(prev.len()))
}

pub(crate) struct Parser<'p> {
  pub(crate) text: &'p [u8],
  pub(crate) tree: &'p mut Tree,
  stack: Vec<Id>,
  at_new_line: bool,
  at_scalar: bool,
  indent: u32,
  offset: u32,
  on_key_line: bool,
  /// The current lexeme, and its type.
  source: (u32, u32),
  kind: Kind,
}

impl<'p> Parser<'p> {
  pub(crate) fn parse(text: &'p [u8], lexemes: &[Lexeme], tree: &'p mut Tree) {
    let mut parser = Parser {
      text,
      tree,
      stack: Vec::new(),
      at_new_line: true,
      at_scalar: false,
      indent: 0,
      offset: 0,
      on_key_line: false,
      source: (0, 0),
      kind: Kind::Newline,
    };
    for lexeme in lexemes {
      parser.next(*lexeme);
    }
    while !parser.stack.is_empty() {
      parser.pop(None);
    }
  }

  fn source_token(&self) -> Source {
    Source {
      kind: self.kind,
      offset: self.offset,
      end: self.offset + self.len(),
      indent: self.indent,
    }
  }

  fn len(&self) -> u32 {
    self.source.1 - self.source.0
  }

  fn text(&self) -> &'p [u8] {
    &self.text[self.source.0 as usize..self.source.1 as usize]
  }

  fn emit(&mut self, node: Node) {
    let id = self.tree.add(node);
    self.tree.tokens.push(id);
  }

  fn next(&mut self, lexeme: Lexeme) {
    let (start, end, marker) = match lexeme {
      Lexeme::Text(start, end) => (start, end, None),
      Lexeme::Document => (self.offset, self.offset, Some(Kind::DocMode)),
      Lexeme::FlowEnd => (self.offset, self.offset, Some(Kind::FlowErrorEnd)),
      Lexeme::Scalar => (self.offset, self.offset, Some(Kind::Scalar)),
    };
    self.source = (start, end);
    if self.at_scalar {
      self.at_scalar = false;
      self.step();
      self.offset += self.len();
      return;
    }
    let kind = match marker {
      Some(kind) => Some(kind),
      None => token_type(self.text()),
    };
    match kind {
      None => {
        let error = Node::Error {
          offset: self.offset,
          end: self.offset + self.len(),
          message: "Not a YAML token",
        };
        let id = self.tree.add(error);
        self.pop(Some(id));
        self.offset += self.len();
      }
      Some(Kind::Scalar) => {
        self.at_new_line = false;
        self.at_scalar = true;
        self.kind = Kind::Scalar;
      }
      Some(kind) => {
        self.kind = kind;
        self.step();
        match kind {
          Kind::Newline => {
            self.at_new_line = true;
            self.indent = 0;
          }
          Kind::Space => {
            if self.at_new_line && self.text().first() == Some(&b' ') {
              self.indent += self.len();
            }
          }
          Kind::ExplicitKeyInd | Kind::MapValueInd | Kind::SeqItemInd => {
            if self.at_new_line {
              self.indent += self.len();
            }
          }
          Kind::DocMode | Kind::FlowErrorEnd => return,
          _ => self.at_new_line = false,
        }
        self.offset += self.len();
      }
    }
  }

  fn peek(&self, n: usize) -> Option<Id> {
    self.stack.len().checked_sub(n).map(|at| self.stack[at])
  }

  fn node(&self, id: Id) -> &Node {
    &self.tree.nodes[id as usize]
  }

  fn node_mut(&mut self, id: Id) -> &mut Node {
    &mut self.tree.nodes[id as usize]
  }

  fn step(&mut self) {
    let top = self.peek(1);
    if self.kind == Kind::DocEnd
      && !top.is_some_and(|top| matches!(self.node(top), Node::DocumentEnd { .. }))
    {
      while !self.stack.is_empty() {
        self.pop(None);
      }
      let end =
        Node::DocumentEnd { offset: self.offset, source_end: self.offset + self.len(), end: None };
      let id = self.tree.add(end);
      self.stack.push(id);
      return;
    }
    let Some(top) = top else { return self.stream() };
    match self.node(top) {
      Node::Document { .. } => self.document(top),
      Node::FlowScalar { .. } => self.scalar(top),
      Node::BlockScalar { .. } => self.block_scalar(top),
      Node::BlockMap { .. } => self.block_map(top),
      Node::BlockSeq { .. } => self.block_sequence(top),
      Node::FlowCollection { .. } => self.flow_collection(top),
      Node::DocumentEnd { .. } => self.document_end(top),
      _ => self.pop(None),
    }
  }

  /// `pop`: takes the node off the stack, or `error`, and puts it in the node under it.
  fn pop(&mut self, error: Option<Id>) {
    let token = match error {
      Some(error) => Some(error),
      None => self.stack.pop(),
    };
    let Some(token) = token else {
      return self.emit(Node::Error {
        offset: self.offset,
        end: self.offset,
        message: "Tried to pop an empty stack",
      });
    };
    let Some(top) = self.peek(1) else { return self.tree.tokens.push(token) };
    match self.node(token) {
      Node::BlockScalar { .. } => {
        let indent = self.node(top).indent().unwrap_or(0);
        self.node_mut(token).set_indent(indent);
      }
      Node::FlowCollection { .. } if matches!(self.node(top), Node::Document { .. }) => {
        self.node_mut(token).set_indent(0);
      }
      _ => {}
    }
    if matches!(self.node(token), Node::FlowCollection { .. }) {
      self.fix_flow_seq_items(token);
    }
    match self.node(top) {
      Node::Document { .. } => {
        if let Node::Document { value, .. } = self.node_mut(top) {
          *value = Some(token);
        }
      }
      Node::BlockScalar { .. } => {
        if let Node::BlockScalar { props, .. } = self.node_mut(top) {
          props.push(Prop::Token(token));
        }
      }
      Node::BlockMap { .. } => {
        let items = self.node_mut(top).items_mut().expect("a map has items");
        let it = items.last_mut().expect("a map has an item");
        if it.value.is_some() {
          items.push(Item { key: Key::Token(token), sep: Some(Vec::new()), ..Item::default() });
          self.on_key_line = true;
          return;
        } else if it.sep.is_some() {
          it.value = Some(token);
        } else {
          it.key = Key::Token(token);
          it.sep = Some(Vec::new());
          self.on_key_line = !it.explicit_key;
          return;
        }
      }
      Node::BlockSeq { .. } => {
        let items = self.node_mut(top).items_mut().expect("a sequence has items");
        let it = items.last_mut().expect("a sequence has an item");
        if it.value.is_some() {
          items.push(Item { value: Some(token), ..Item::default() });
        } else {
          it.value = Some(token);
        }
      }
      Node::FlowCollection { .. } => {
        let items = self.node_mut(top).items_mut().expect("a flow collection has items");
        match items.last_mut() {
          Some(it) if it.value.is_none() && it.sep.is_some() => it.value = Some(token),
          Some(it) if it.value.is_none() => {
            it.key = Key::Token(token);
            it.sep = Some(Vec::new());
          }
          _ => {
            items.push(Item { key: Key::Token(token), sep: Some(Vec::new()), ..Item::default() })
          }
        }
        return;
      }
      _ => {
        self.pop(None);
        self.pop(Some(token));
        return;
      }
    }
    let top_takes = matches!(
      self.node(top),
      Node::Document { .. } | Node::BlockMap { .. } | Node::BlockSeq { .. }
    );
    let token_is_block = matches!(self.node(token), Node::BlockMap { .. } | Node::BlockSeq { .. });
    if !(top_takes && token_is_block) {
      return;
    }
    let indent = self.node(token).indent().unwrap_or(0);
    let items = self.node(token).items().expect("a block collection has items");
    let Some(last) = items.last() else { return };
    let moves = last.sep.is_none()
      && last.value.is_none()
      && !last.start.is_empty()
      && find_non_empty(&last.start).is_none()
      && (indent == 0
        || last.start.iter().all(|st| st.kind != Kind::Comment || st.indent < indent));
    if !moves {
      return;
    }
    let last = self.node_mut(token).items_mut().expect("items").pop().expect("an item");
    match self.node_mut(top) {
      Node::Document { end, .. } => *end = Some(last.start),
      node => node.items_mut().expect("items").push(Item { start: last.start, ..Item::default() }),
    }
  }

  /// `fixFlowSeqItems`: in a flow sequence, an item with a key but no `:` is a value.
  fn fix_flow_seq_items(&mut self, id: Id) {
    let Node::FlowCollection { start, items, .. } = &self.tree.nodes[id as usize] else { return };
    if start.kind != Kind::FlowSeqStart {
      return;
    }
    let mut items = items.clone();
    for it in &mut items {
      let Some(sep) = it.sep.take() else { continue };
      if it.value.is_some()
        || includes(&it.start, Kind::ExplicitKeyInd)
        || includes(&sep, Kind::MapValueInd)
      {
        it.sep = Some(sep);
        continue;
      }
      if let Key::Token(key) = it.key {
        it.value = Some(key);
      }
      it.key = Key::Absent;
      let flow = it.value.is_some_and(|value| is_flow_token(&self.tree.nodes[value as usize]));
      match it.value {
        Some(value) if flow => {
          let node = &mut self.tree.nodes[value as usize];
          match node {
            Node::FlowCollection { end, .. } => end.extend(sep),
            node => match node.end_mut() {
              Some(Some(end)) => end.extend(sep),
              Some(end) => *end = Some(sep),
              None => {}
            },
          }
        }
        _ => it.start.extend(sep),
      }
    }
    if let Node::FlowCollection { items: target, .. } = &mut self.tree.nodes[id as usize] {
      *target = items;
    }
  }

  fn stream(&mut self) {
    match self.kind {
      Kind::DirectiveLine => {
        let (offset, end) = (self.offset, self.offset + self.len());
        self.emit(Node::Directive { offset, end });
      }
      Kind::ByteOrderMark | Kind::Space | Kind::Comment | Kind::Newline => {
        let source = self.source_token();
        self.emit(Node::Source(source));
      }
      Kind::DocMode | Kind::DocStart => {
        let start =
          if self.kind == Kind::DocStart { vec![self.source_token()] } else { Vec::new() };
        let id =
          self.tree.add(Node::Document { offset: self.offset, start, value: None, end: None });
        self.stack.push(id);
      }
      _ => {
        let (offset, end) = (self.offset, self.offset + self.len());
        self.emit(Node::Error { offset, end, message: "Unexpected token in YAML stream" });
      }
    }
  }

  fn document(&mut self, doc: Id) {
    let Node::Document { start, value, .. } = self.node(doc) else { return };
    if value.is_some() {
      return self.line_end(doc);
    }
    match self.kind {
      Kind::DocStart => {
        if find_non_empty(start).is_some() {
          self.pop(None);
          self.step();
        } else {
          let token = self.source_token();
          if let Node::Document { start, .. } = self.node_mut(doc) {
            start.push(token);
          }
        }
        return;
      }
      Kind::Anchor | Kind::Tag | Kind::Space | Kind::Comment | Kind::Newline => {
        let token = self.source_token();
        if let Node::Document { start, .. } = self.node_mut(doc) {
          start.push(token);
        }
        return;
      }
      _ => {}
    }
    match self.start_block_value(doc) {
      Some(value) => {
        let id = self.tree.add(value);
        self.stack.push(id);
      }
      None => {
        let (offset, end) = (self.offset, self.offset + self.len());
        self.emit(Node::Error { offset, end, message: "Unexpected token in YAML document" });
      }
    }
  }

  fn scalar(&mut self, scalar: Id) {
    if self.kind != Kind::MapValueInd {
      return self.line_end(scalar);
    }
    let parent = self.peek(2).expect("a scalar has a parent");
    let start = self.prev_props_split(parent);
    let token = self.source_token();
    let Node::FlowScalar { offset, indent, end, .. } = self.node_mut(scalar) else { return };
    let (offset, indent) = (*offset, *indent);
    let sep = match end.take() {
      Some(mut sep) => {
        sep.push(token);
        sep
      }
      None => vec![token],
    };
    let item = Item { start, key: Key::Token(scalar), sep: Some(sep), ..Item::default() };
    let map = self.tree.add(Node::BlockMap { offset, indent, items: vec![item] });
    self.on_key_line = true;
    *self.stack.last_mut().expect("the scalar is on the stack") = map;
  }

  /// `getFirstKeyStartProps(getPrevProps(parent))`
  fn prev_props_split(&mut self, parent: Id) -> Vec<Source> {
    match self.node_mut(parent) {
      Node::Document { start, .. } => first_key_start_props(start),
      Node::BlockMap { items, .. } => {
        let it = items.last_mut().expect("a map has an item");
        match &mut it.sep {
          Some(sep) => first_key_start_props(sep),
          None => first_key_start_props(&mut it.start),
        }
      }
      Node::BlockSeq { items, .. } => {
        first_key_start_props(&mut items.last_mut().expect("a sequence has an item").start)
      }
      _ => Vec::new(),
    }
  }

  fn block_scalar(&mut self, scalar: Id) {
    match self.kind {
      Kind::Space | Kind::Comment | Kind::Newline => {
        let token = self.source_token();
        if let Node::BlockScalar { props, .. } = self.node_mut(scalar) {
          props.push(Prop::Source(token));
        }
      }
      Kind::Scalar => {
        let source = self.source;
        if let Node::BlockScalar { source: target, .. } = self.node_mut(scalar) {
          *target = source;
        }
        self.at_new_line = true;
        self.indent = 0;
        self.pop(None);
      }
      _ => {
        self.pop(None);
        self.step();
      }
    }
  }

  fn block_map(&mut self, map: Id) {
    let token = self.source_token();
    let map_indent = self.node(map).indent().unwrap_or(0);
    match self.kind {
      Kind::Newline => {
        self.on_key_line = false;
        let items = self.node(map).items().expect("items");
        let it = items.last().expect("an item");
        if let Some(value) = it.value {
          if self.ends_with_comment(value) {
            self.push_end(value, token);
          } else {
            self.push_item(map, Item { start: vec![token], ..Item::default() });
          }
        } else {
          self.with_last(map, |it| match &mut it.sep {
            Some(sep) => sep.push(token),
            None => it.start.push(token),
          });
        }
        return;
      }
      Kind::Space | Kind::Comment => {
        let items = self.node(map).items().expect("items");
        let it = items.last().expect("an item");
        if it.value.is_some() {
          self.push_item(map, Item { start: vec![token], ..Item::default() });
        } else if it.sep.is_some() {
          self.with_last(map, |it| it.sep.as_mut().expect("sep").push(token));
        } else {
          if self.at_indented_comment(&it.start, map_indent)
            && self.absorb_indented_comment(map, token)
          {
            return;
          }
          self.with_last(map, |it| it.start.push(token));
        }
        return;
      }
      _ => {}
    }
    if self.indent >= map_indent {
      let items = self.node(map).items().expect("items");
      let it = items.last().expect("an item");
      let at_map_indent = !self.on_key_line && self.indent == map_indent;
      let at_next_item =
        at_map_indent && (it.sep.is_some() || it.explicit_key) && self.kind != Kind::SeqItemInd;
      // For empty nodes, newline-separated tokens that are not indented go to the next node.
      let mut start: Vec<Source> = Vec::new();
      if at_next_item
        && it.value.is_none()
        && let Some(sep) = &it.sep
      {
        let mut nl: Vec<usize> = Vec::new();
        for (i, st) in sep.iter().enumerate() {
          match st.kind {
            Kind::Newline => nl.push(i),
            Kind::Space => {}
            Kind::Comment if st.indent > map_indent => nl.clear(),
            Kind::Comment => {}
            _ => nl.clear(),
          }
        }
        if nl.len() >= 2 {
          let at = nl[1];
          start = self.with_last(map, |it| it.sep.as_mut().expect("sep").split_off(at));
        }
      }
      let items = self.node(map).items().expect("items");
      let it = items.last().expect("an item").clone();
      match self.kind {
        Kind::Anchor | Kind::Tag => {
          if at_next_item || it.value.is_some() {
            start.push(token);
            self.push_item(map, Item { start, ..Item::default() });
            self.on_key_line = true;
          } else if it.sep.is_some() {
            self.with_last(map, |it| it.sep.as_mut().expect("sep").push(token));
          } else {
            self.with_last(map, |it| it.start.push(token));
          }
          return;
        }
        Kind::ExplicitKeyInd => {
          if it.sep.is_none() && !it.explicit_key {
            self.with_last(map, |it| {
              it.start.push(token);
              it.explicit_key = true;
            });
          } else if at_next_item || it.value.is_some() {
            start.push(token);
            self.push_item(map, Item { start, explicit_key: true, ..Item::default() });
          } else {
            let item = Item { start: vec![token], explicit_key: true, ..Item::default() };
            self.push_block_map(item);
          }
          self.on_key_line = true;
          return;
        }
        Kind::MapValueInd => {
          if it.explicit_key {
            if it.sep.is_none() {
              if includes(&it.start, Kind::Newline) {
                self.with_last(map, |it| {
                  it.key = Key::Null;
                  it.sep = Some(vec![token]);
                });
              } else {
                let start = self.with_last(map, |it| first_key_start_props(&mut it.start));
                self.push_block_map(Item {
                  start,
                  key: Key::Null,
                  sep: Some(vec![token]),
                  ..Item::default()
                });
              }
            } else if it.value.is_some() {
              self
                .push_item(map, Item { key: Key::Null, sep: Some(vec![token]), ..Item::default() });
            } else if includes(it.sep.as_ref().expect("sep"), Kind::MapValueInd) {
              self.push_block_map(Item {
                start,
                key: Key::Null,
                sep: Some(vec![token]),
                ..Item::default()
              });
            } else if matches!(it.key, Key::Token(key) if is_flow_token(self.node(key)))
              && !includes(it.sep.as_ref().expect("sep"), Kind::Newline)
            {
              let (start, key, mut sep) = self.with_last(map, |it| {
                let start = first_key_start_props(&mut it.start);
                (start, std::mem::take(&mut it.key), it.sep.take().expect("sep"))
              });
              sep.push(token);
              self.push_block_map(Item { start, key, sep: Some(sep), ..Item::default() });
            } else if !start.is_empty() {
              // Not actually at the next item.
              self.with_last(map, |it| {
                let sep = it.sep.as_mut().expect("sep");
                sep.extend(start);
                sep.push(token);
              });
            } else {
              self.with_last(map, |it| it.sep.as_mut().expect("sep").push(token));
            }
          } else if it.sep.is_none() {
            self.with_last(map, |it| {
              it.key = Key::Null;
              it.sep = Some(vec![token]);
            });
          } else if it.value.is_some() || at_next_item {
            self.push_item(
              map,
              Item { start, key: Key::Null, sep: Some(vec![token]), ..Item::default() },
            );
          } else if includes(it.sep.as_ref().expect("sep"), Kind::MapValueInd) {
            self.push_block_map(Item { key: Key::Null, sep: Some(vec![token]), ..Item::default() });
          } else {
            self.with_last(map, |it| it.sep.as_mut().expect("sep").push(token));
          }
          self.on_key_line = true;
          return;
        }
        Kind::Alias | Kind::Scalar | Kind::SingleQuotedScalar | Kind::DoubleQuotedScalar => {
          let fs = self.flow_scalar();
          if at_next_item || it.value.is_some() {
            let fs = self.tree.add(fs);
            self.push_item(
              map,
              Item { start, key: Key::Token(fs), sep: Some(Vec::new()), ..Item::default() },
            );
            self.on_key_line = true;
          } else if it.sep.is_some() {
            let fs = self.tree.add(fs);
            self.stack.push(fs);
          } else {
            let fs = self.tree.add(fs);
            self.with_last(map, |it| {
              it.key = Key::Token(fs);
              it.sep = Some(Vec::new());
            });
            self.on_key_line = true;
          }
          return;
        }
        _ => {
          if let Some(value) = self.start_block_value(map) {
            if matches!(value, Node::BlockSeq { .. }) {
              if !it.explicit_key
                && it.sep.as_ref().is_some_and(|sep| !includes(sep, Kind::Newline))
              {
                let (offset, end) = (self.offset, self.offset + self.len());
                let error = self.tree.add(Node::Error {
                  offset,
                  end,
                  message: "Unexpected block-seq-ind on same line with key",
                });
                return self.pop(Some(error));
              }
            } else if at_map_indent {
              self.push_item(map, Item { start, ..Item::default() });
            }
            let id = self.tree.add(value);
            self.stack.push(id);
            return;
          }
        }
      }
    }
    self.pop(None);
    self.step();
  }

  fn block_sequence(&mut self, seq: Id) {
    let token = self.source_token();
    let seq_indent = self.node(seq).indent().unwrap_or(0);
    let items = self.node(seq).items().expect("items");
    let it = items.last().expect("an item").clone();
    match self.kind {
      Kind::Newline => {
        match it.value {
          Some(value) if self.ends_with_comment(value) => self.push_end(value, token),
          Some(_) => self.push_item(seq, Item { start: vec![token], ..Item::default() }),
          None => self.with_last(seq, |it| it.start.push(token)),
        }
        return;
      }
      Kind::Space | Kind::Comment => {
        if it.value.is_some() {
          self.push_item(seq, Item { start: vec![token], ..Item::default() });
        } else {
          if self.at_indented_comment(&it.start, seq_indent)
            && self.absorb_indented_comment(seq, token)
          {
            return;
          }
          self.with_last(seq, |it| it.start.push(token));
        }
        return;
      }
      Kind::Anchor | Kind::Tag if !(it.value.is_some() || self.indent <= seq_indent) => {
        return self.with_last(seq, |it| it.start.push(token));
      }
      Kind::SeqItemInd if self.indent == seq_indent => {
        if it.value.is_some() || includes(&it.start, Kind::SeqItemInd) {
          self.push_item(seq, Item { start: vec![token], ..Item::default() });
        } else {
          self.with_last(seq, |it| it.start.push(token));
        }
        return;
      }
      _ => {}
    }
    if self.indent > seq_indent
      && let Some(value) = self.start_block_value(seq)
    {
      let id = self.tree.add(value);
      self.stack.push(id);
      return;
    }
    self.pop(None);
    self.step();
  }

  fn flow_collection(&mut self, fc: Id) {
    let token = self.source_token();
    if self.kind == Kind::FlowErrorEnd {
      loop {
        self.pop(None);
        match self.peek(1) {
          Some(top) if matches!(self.node(top), Node::FlowCollection { .. }) => {}
          _ => return,
        }
      }
    }
    let Node::FlowCollection { end, items, .. } = self.node(fc) else { return };
    if end.is_empty() {
      let it = items.last().cloned();
      match self.kind {
        Kind::Comma | Kind::ExplicitKeyInd => {
          match it {
            Some(it) if it.sep.is_none() => self.with_last(fc, |it| it.start.push(token)),
            _ => self.push_item(fc, Item { start: vec![token], ..Item::default() }),
          }
          return;
        }
        Kind::MapValueInd => {
          match it {
            Some(it) if it.value.is_none() && it.sep.is_some() => {
              self.with_last(fc, |it| it.sep.as_mut().expect("sep").push(token))
            }
            Some(it) if it.value.is_none() => self.with_last(fc, |it| {
              it.key = Key::Null;
              it.sep = Some(vec![token]);
            }),
            _ => {
              self.push_item(fc, Item { key: Key::Null, sep: Some(vec![token]), ..Item::default() })
            }
          }
          return;
        }
        Kind::Space | Kind::Comment | Kind::Newline | Kind::Anchor | Kind::Tag => {
          match it {
            Some(it) if it.value.is_none() && it.sep.is_some() => {
              self.with_last(fc, |it| it.sep.as_mut().expect("sep").push(token))
            }
            Some(it) if it.value.is_none() => self.with_last(fc, |it| it.start.push(token)),
            _ => self.push_item(fc, Item { start: vec![token], ..Item::default() }),
          }
          return;
        }
        Kind::Alias | Kind::Scalar | Kind::SingleQuotedScalar | Kind::DoubleQuotedScalar => {
          let fs = self.flow_scalar();
          let fs = self.tree.add(fs);
          match it {
            Some(it) if it.value.is_none() && it.sep.is_some() => self.stack.push(fs),
            Some(it) if it.value.is_none() => self.with_last(fc, |it| {
              it.key = Key::Token(fs);
              it.sep = Some(Vec::new());
            }),
            _ => self.push_item(
              fc,
              Item { key: Key::Token(fs), sep: Some(Vec::new()), ..Item::default() },
            ),
          }
          return;
        }
        Kind::FlowMapEnd | Kind::FlowSeqEnd => {
          if let Node::FlowCollection { end, .. } = self.node_mut(fc) {
            end.push(token);
          }
          return;
        }
        _ => {}
      }
      match self.start_block_value(fc) {
        Some(value) => {
          let id = self.tree.add(value);
          self.stack.push(id);
        }
        None => {
          self.pop(None);
          self.step();
        }
      }
      return;
    }
    let parent = self.peek(2).expect("a flow collection has a parent");
    let fc_indent = self.node(fc).indent().unwrap_or(0);
    let parent_node = self.node(parent);
    let parent_is_map = matches!(parent_node, Node::BlockMap { .. });
    let parent_indent = parent_node.indent().unwrap_or(0);
    let parent_last_sep =
      parent_node.items().and_then(|items| items.last()).is_some_and(|it| it.sep.is_some());
    if parent_is_map
      && ((self.kind == Kind::MapValueInd && parent_indent == fc_indent)
        || (self.kind == Kind::Newline && !parent_last_sep))
    {
      self.pop(None);
      self.step();
    } else if self.kind == Kind::MapValueInd && !matches!(parent_node, Node::FlowCollection { .. })
    {
      let start = self.prev_props_split(parent);
      self.fix_flow_seq_items(fc);
      let Node::FlowCollection { offset, indent, end, .. } = self.node_mut(fc) else { return };
      let (offset, indent) = (*offset, *indent);
      let mut sep = end.split_off(1.min(end.len()));
      sep.push(token);
      let item = Item { start, key: Key::Token(fc), sep: Some(sep), ..Item::default() };
      let map = self.tree.add(Node::BlockMap { offset, indent, items: vec![item] });
      self.on_key_line = true;
      *self.stack.last_mut().expect("the collection is on the stack") = map;
    } else {
      self.line_end(fc);
    }
  }

  fn flow_scalar(&self) -> Node {
    let kind = match self.kind {
      Kind::Alias => ScalarKind::Alias,
      Kind::SingleQuotedScalar => ScalarKind::Single,
      Kind::DoubleQuotedScalar => ScalarKind::Double,
      _ => ScalarKind::Plain,
    };
    Node::FlowScalar {
      kind,
      offset: self.offset,
      source_end: self.offset + self.len(),
      indent: self.indent,
      end: None,
    }
  }

  fn start_block_value(&mut self, parent: Id) -> Option<Node> {
    let (offset, indent) = (self.offset, self.indent);
    let token = self.source_token();
    Some(match self.kind {
      Kind::Alias | Kind::Scalar | Kind::SingleQuotedScalar | Kind::DoubleQuotedScalar => {
        self.flow_scalar()
      }
      Kind::BlockScalarHeader => Node::BlockScalar {
        offset,
        indent,
        props: vec![Prop::Source(token)],
        source: (offset, offset),
      },
      Kind::FlowMapStart | Kind::FlowSeqStart => {
        Node::FlowCollection { offset, indent, start: token, items: Vec::new(), end: Vec::new() }
      }
      Kind::SeqItemInd => Node::BlockSeq {
        offset,
        indent,
        items: vec![Item { start: vec![token], ..Item::default() }],
      },
      Kind::ExplicitKeyInd => {
        self.on_key_line = true;
        let mut start = self.prev_props_split(parent);
        start.push(token);
        Node::BlockMap {
          offset,
          indent,
          items: vec![Item { start, explicit_key: true, ..Item::default() }],
        }
      }
      Kind::MapValueInd => {
        self.on_key_line = true;
        let start = self.prev_props_split(parent);
        let item = Item { start, key: Key::Null, sep: Some(vec![token]), ..Item::default() };
        Node::BlockMap { offset, indent, items: vec![item] }
      }
      _ => return None,
    })
  }

  fn at_indented_comment(&self, start: &[Source], indent: u32) -> bool {
    self.kind == Kind::Comment
      && self.indent > indent
      && start.iter().all(|st| matches!(st.kind, Kind::Newline | Kind::Space))
  }

  /// The comment is more indented than the collection, after an item whose value has an `end`: it moves there, with
  /// the tokens before it, and the empty item goes.
  fn absorb_indented_comment(&mut self, collection: Id, token: Source) -> bool {
    let items = self.node(collection).items().expect("items");
    let Some(prev) = items.len().checked_sub(2).map(|at| &items[at]) else { return false };
    let Some(value) = prev.value else { return false };
    if !self.has_end_array(value) {
      return false;
    }
    let last = self.node_mut(collection).items_mut().expect("items").pop().expect("an item");
    let mut moved = last.start;
    moved.push(token);
    self.extend_end(value, moved);
    true
  }

  /// `Array.isArray(value.end)`
  fn has_end_array(&self, id: Id) -> bool {
    match self.node(id) {
      Node::FlowCollection { .. } => true,
      Node::Document { end, .. } | Node::DocumentEnd { end, .. } | Node::FlowScalar { end, .. } => {
        end.is_some()
      }
      _ => false,
    }
  }

  fn ends_with_comment(&self, id: Id) -> bool {
    let last = match self.node(id) {
      Node::FlowCollection { end, .. } => end.last(),
      Node::Document { end, .. } | Node::DocumentEnd { end, .. } | Node::FlowScalar { end, .. } => {
        end.as_ref().and_then(|end| end.last())
      }
      _ => None,
    };
    last.is_some_and(|it| it.kind == Kind::Comment)
  }

  fn push_end(&mut self, id: Id, token: Source) {
    self.extend_end(id, vec![token]);
  }

  fn extend_end(&mut self, id: Id, tokens: Vec<Source>) {
    match self.node_mut(id) {
      Node::FlowCollection { end, .. } => end.extend(tokens),
      node => {
        if let Some(Some(end)) = node.end_mut() {
          end.extend(tokens);
        }
      }
    }
  }

  fn push_item(&mut self, collection: Id, item: Item) {
    self.node_mut(collection).items_mut().expect("items").push(item);
  }

  fn push_block_map(&mut self, item: Item) {
    let map = Node::BlockMap { offset: self.offset, indent: self.indent, items: vec![item] };
    let id = self.tree.add(map);
    self.stack.push(id);
  }

  fn with_last<T>(&mut self, collection: Id, change: impl FnOnce(&mut Item) -> T) -> T {
    let items = self.node_mut(collection).items_mut().expect("items");
    change(items.last_mut().expect("an item"))
  }

  fn document_end(&mut self, doc_end: Id) {
    if self.kind == Kind::DocMode {
      return;
    }
    let token = self.source_token();
    if let Node::DocumentEnd { end, .. } = self.node_mut(doc_end) {
      end.get_or_insert_with(Vec::new).push(token);
    }
    if self.kind == Kind::Newline {
      self.pop(None);
    }
  }

  /// `lineEnd`
  fn line_end(&mut self, id: Id) {
    match self.kind {
      Kind::Comma
      | Kind::DocStart
      | Kind::DocEnd
      | Kind::FlowSeqEnd
      | Kind::FlowMapEnd
      | Kind::MapValueInd => {
        self.pop(None);
        self.step();
      }
      _ => {
        if self.kind == Kind::Newline {
          self.on_key_line = false;
        }
        let token = self.source_token();
        match self.node_mut(id) {
          Node::FlowCollection { end, .. } => end.push(token),
          node => {
            if let Some(end) = node.end_mut() {
              end.get_or_insert_with(Vec::new).push(token);
            }
          }
        }
        if self.kind == Kind::Newline {
          self.pop(None);
        }
      }
    }
  }
}

/// `isFlowToken`
fn is_flow_token(node: &Node) -> bool {
  matches!(node, Node::FlowScalar { .. } | Node::FlowCollection { .. })
}
