//! `yaml-unist-parser`'s transforms (`src/transforms/*`): the tree that Prettier prints, from the composed documents
//! and the tokens of the syntax tree they come from.

use super::ast::{Chomping, Entry, Extra, Id, Kind, Lines, Position, Root};
use super::compose::{Cid, Composed, Composition, Content, Pair, ScalarType};
use super::cst::{self, Item, Key, Node as Token, Prop, Source, Tree};

pub(crate) struct Transformer<'t> {
  pub(crate) text: &'t str,
  pub(crate) tree: &'t Tree,
  pub(crate) composition: &'t Composition,
  pub(crate) lines: Lines<'t>,
  pub(crate) root: &'t mut Root,
  /// The children of the nodes being built, innermost last.
  pub(crate) pending: Vec<Id>,
}

/// A document's tokens, sorted: `DocumentData`.
struct DocumentData {
  before: Vec<Before>,
  cst: Option<cst::Id>,
  node: usize,
  after: Vec<Source>,
  end: Option<cst::Id>,
}

#[derive(Clone, Copy)]
enum Before {
  Comment(Source),
  Directive(u32, u32),
}

impl<'t> Transformer<'t> {
  fn src(&self, source: &Source) -> &'t str {
    &self.text[source.offset as usize..source.end as usize]
  }

  fn cst(&self, id: cst::Id) -> &'t Token {
    let tree: &'t Tree = self.tree;
    tree.node(id)
  }

  fn composed(&self, id: Cid) -> &'t Composed {
    let composition: &'t Composition = self.composition;
    composition.node(id)
  }

  fn position(&self, start: u32, end: u32) -> Position {
    self.lines.position(start, end)
  }

  fn add(&mut self, entry: Entry, children: &[Id]) -> Id {
    self.root.add(entry, children)
  }

  /// Adds a node with the children pending from `mark`.
  fn add_pending(&mut self, entry: Entry, mark: usize) -> Id {
    let id = self.root.add(entry, &self.pending[mark..]);
    self.pending.truncate(mark);
    id
  }

  /// Gives a node what few nodes hold, if it holds any.
  fn extra(&mut self, id: Id, extra: Extra) {
    if !extra.leading.is_empty()
      || !extra.end.is_empty()
      || !extra.middle.is_empty()
      || !extra.parameters.is_empty()
    {
      self.root.set_extra(id, extra);
    }
  }

  /// `parse`, after composing: the root, its documents, and every comment, sorted.
  pub(crate) fn transform(&mut self) {
    let documents = self.documents();
    let position = self.position(0, self.text.len() as u32);
    let id = self.add(Entry::new(Kind::Root, position), &documents);
    self.root.root = id;
    let mut comments = std::mem::take(&mut self.root.comments);
    comments.sort_by_key(|id| self.root.node(*id).position.start.offset);
    self.root.comments = comments;
  }

  /// `transformDocuments`
  fn documents(&mut self) -> Vec<Id> {
    let mut documents: Vec<DocumentData> = Vec::new();
    let mut buffer: Vec<Source> = Vec::new();
    let mut before: Vec<Before> = Vec::new();
    let mut current: Option<usize> = None;
    for &token in &self.tree.tokens {
      match self.cst(token) {
        Token::Document { .. } => {
          let mut data_before = std::mem::take(&mut before);
          data_before.extend(buffer.drain(..).map(Before::Comment));
          let node = documents.len();
          documents.push(DocumentData {
            before: data_before,
            cst: Some(token),
            node,
            after: Vec::new(),
            end: None,
          });
          current = Some(documents.len() - 1);
        }
        Token::Source(source) if source.kind == cst::Kind::Comment => buffer.push(*source),
        Token::Directive { offset, end } => {
          before.extend(buffer.drain(..).map(Before::Comment));
          before.push(Before::Directive(*offset, *end));
        }
        Token::DocumentEnd { .. } => {
          if let Some(index) = current {
            documents[index].after = std::mem::take(&mut buffer);
            documents[index].end = Some(token);
          }
        }
        _ => {}
      }
    }
    if !buffer.is_empty() {
      match current {
        Some(index) => documents[index].after.append(&mut buffer),
        // Without a document, the comments are before the body of an empty one.
        None => {
          let mut data_before = std::mem::take(&mut before);
          data_before.extend(buffer.drain(..).map(Before::Comment));
          let node = documents.len();
          documents.push(DocumentData {
            before: data_before,
            cst: None,
            node,
            after: Vec::new(),
            end: None,
          });
        }
      }
    }
    documents.into_iter().map(|data| self.document(data)).collect()
  }

  fn comment(&mut self, source: &Source) -> Id {
    let id = self.add(Entry::new(Kind::Comment, self.position(source.offset, source.end)), &[]);
    self.root.set_value(id, &self.src(source)[1..]);
    self.root.comments.push(id);
    id
  }

  /// `extractComments`, where the other tokens aren't needed: registers the comments.
  fn comments(&mut self, tokens: &[Source]) {
    for token in tokens.iter().filter(|it| it.kind == cst::Kind::Comment) {
      self.comment(token);
    }
  }

  /// `extractComments`: registers the comments, and returns the other tokens that aren't space.
  fn extract_comments(&mut self, tokens: &[Source]) -> Vec<Source> {
    let mut rest = Vec::new();
    for token in tokens.iter().filter(|it| !is_space(it)) {
      match token.kind {
        cst::Kind::Comment => drop(self.comment(token)),
        _ => rest.push(*token),
      }
    }
    rest
  }

  /// `transformDocument`
  fn document(&mut self, data: DocumentData) -> Id {
    let composed = &self.composition.documents[data.node];
    let (head, before_body, doc_start) = self.document_head(&data);
    let (body, end_point, trailing) =
      self.document_body(&data, doc_start, before_body, composed.range);
    let start = self.root.node(head).position.start;
    let mut node = Entry::new(Kind::Document, Position { start, end: end_point });
    node.directives_end_marker = doc_start.is_some();
    node.document_end_marker = data.end.is_some();
    node.trailing_comment = trailing;
    self.add(node, &[head, body])
  }

  /// `transformDocumentHead`: the head, the tokens after `---`, and `---`.
  fn document_head(&mut self, data: &DocumentData) -> (Id, Vec<Source>, Option<Source>) {
    let mut directives: Vec<Id> = Vec::new();
    let mut candidates: Vec<Id> = Vec::new();
    let mut last_directive: Option<Id> = None;
    for token in &data.before {
      match *token {
        Before::Comment(source) => {
          let comment = self.comment(&source);
          let line = self.root.node(comment).position.start.line;
          let attaches = last_directive.is_some_and(|directive| {
            let node = self.root.node(directive);
            node.position.end.line == line && node.trailing_comment.is_none()
          });
          match (attaches, last_directive) {
            (true, Some(directive)) => {
              let end = self.root.node(comment).position.end;
              let node = self.root.entry_mut(directive);
              node.trailing_comment = Some(comment);
              node.position.end = end;
              self.root.entry_mut(comment).parent = Some(directive);
            }
            _ => candidates.push(comment),
          }
        }
        Before::Directive(offset, end) => {
          let id = self.directive(offset, end);
          directives.push(id);
          last_directive = Some(id);
          candidates.clear();
        }
      }
    }
    let mut between: Vec<Source> = Vec::new();
    let mut doc_start: Option<Source> = None;
    if let Some(cst) = data.cst {
      let Token::Document { start, .. } = self.cst(cst) else { unreachable!() };
      for token in start.iter().filter(|it| !is_space(it)) {
        between.push(*token);
        if doc_start.is_none() && token.kind == cst::Kind::DocStart {
          for t in std::mem::take(&mut between) {
            if t.kind == cst::Kind::Comment {
              let comment = self.comment(&t);
              candidates.push(comment);
            }
          }
          doc_start = Some(*token);
        }
      }
    }
    let composed = &self.composition.documents[data.node];
    let mut range = match (doc_start, composed.contents) {
      (Some(start), _) => (start.offset, start.end),
      (None, Some(contents)) => {
        let start = self.composed(contents).range[0];
        (start, start)
      }
      (None, None) => (composed.range[0], composed.range[0]),
    };
    if let Some(first) = directives.first() {
      range.0 = self.root.node(*first).position.start.offset;
    }
    let position = self.position(range.0, range.1);
    let mut trailing = None;
    if doc_start.is_some()
      && let Some(first) = between.first().copied()
      && first.kind == cst::Kind::Comment
      && self.lines.point(first.offset).line == position.end.line
    {
      trailing = Some(self.comment(&first));
      between.remove(0);
    }
    let end_comments = if doc_start.is_some() { candidates } else { Vec::new() };
    let mut head = Entry::new(Kind::DocumentHead, position);
    head.trailing_comment = trailing;
    let head = self.add(head, &directives);
    self.extra(head, Extra { end: end_comments, ..Extra::default() });
    (head, between, doc_start)
  }

  /// `transformDirective`
  fn directive(&mut self, offset: u32, end: u32) -> Id {
    let source = &self.text[offset as usize..end as usize];
    let mut parts = source.trim().split([' ', '\t']).filter(|it| !it.is_empty());
    let name = parts.next().unwrap_or_default().trim_start_matches('%');
    let parameters = parts.map(str::to_owned).collect();
    let id = self.add(Entry::new(Kind::Directive, self.position(offset, end)), &[]);
    self.root.set_value(id, name);
    self.extra(id, Extra { parameters, ..Extra::default() });
    id
  }

  /// `transformDocumentBody`: the body, where the document ends, and its trailing comment.
  fn document_body(
    &mut self,
    data: &DocumentData,
    doc_start: Option<Source>,
    before_body: Vec<Source>,
    range: [u32; 3],
  ) -> (Id, super::ast::Point, Option<Id>) {
    let props: Vec<Source> = before_body;
    self.extract_comments(&data.after);
    let doc_end = data.end.map(|id| match self.cst(id) {
      Token::DocumentEnd { offset, source_end, end } => (*offset, *source_end, end.clone()),
      _ => unreachable!(),
    });
    let doc_end_point = doc_end.as_ref().map(|(offset, ..)| self.lines.point(*offset));
    let mut end_comments: Vec<Id> = Vec::new();
    let mut trailing: Option<Id> = None;
    if let Some(cst) = data.cst {
      let Token::Document { end, .. } = self.cst(cst) else { unreachable!() };
      let mut tokens: Vec<Source> = end.clone().unwrap_or_default();
      if let Some((_, _, Some(end))) = &doc_end {
        tokens.extend(end.iter().copied());
      }
      for token in tokens.iter().filter(|it| !is_space(it)) {
        if token.kind != cst::Kind::Comment {
          continue;
        }
        let comment = self.comment(token);
        let line = self.root.node(comment).position.start.line;
        match doc_end_point {
          Some(point) if point.line == line => trailing = Some(comment),
          Some(point) if line < point.line => end_comments.push(comment),
          Some(_) => {}
          None => end_comments.push(comment),
        }
      }
    }
    let composed = &self.composition.documents[data.node];
    let has_content = composed.contents.is_some_and(|contents| {
      let range = self.composed(contents).range;
      range[0] < range[1]
        || props.iter().any(|it| matches!(it.kind, cst::Kind::Tag | cst::Kind::Anchor))
    });
    let content = match (has_content, composed.contents) {
      (true, Some(contents)) => Some(self.node(contents, &props)),
      _ => {
        self.extract_comments(&props);
        None
      }
    };
    let text = self.text.as_bytes();
    let mut end = match &doc_end {
      Some((offset, ..)) => offset.saturating_sub(1),
      None => {
        let from = range[2] as usize;
        text
          .get(from..)
          .and_then(|rest| rest.iter().position(|it| !is_js_space(*it)))
          .map_or(text.len(), |at| from + at) as u32
      }
    };
    if end > 0 && text.get(end as usize - 1) == Some(&b'\r') {
      end -= 1;
    }
    let mut start = content.map_or(end, |content| self.root.node(content).position.start.offset);
    if let Some(doc_start) = doc_start {
      let after = doc_start.end + 1;
      if start < after && after <= end {
        start = after;
      }
    }
    let position = self.position(start, end);
    let end_point = match &doc_end {
      Some((_, source_end, _)) => self.lines.point(*source_end),
      None => position.end,
    };
    let body = self.add(Entry::new(Kind::DocumentBody, position), content.as_slice());
    self.extra(body, Extra { end: end_comments, ..Extra::default() });
    (body, end_point, trailing)
  }

  /// `transformNode`
  fn node(&mut self, id: Cid, props: &[Source]) -> Id {
    let composed = self.composed(id);
    match &composed.content {
      Content::Alias { name } => {
        let src = composed.src.expect("an alias has a token");
        self.scalar_end(src);
        let content = self.content(composed, props);
        let position = self.position(composed.range[0], composed.range[1]);
        self.content_node(Kind::Alias, position, content, name)
      }
      Content::Map { flow: true, .. } => self.flow_map(composed, props),
      Content::Map { .. } => self.map(composed, props),
      Content::Seq { flow: true, .. } => self.flow_seq(composed, props),
      Content::Seq { .. } => self.seq(composed, props),
      Content::Scalar { kind, value } => match kind {
        ScalarType::BlockLiteral | ScalarType::BlockFolded => {
          self.block_value(composed, *kind, value, props)
        }
        ScalarType::Plain => self.plain(composed, value, props),
        ScalarType::QuoteSingle | ScalarType::QuoteDouble => {
          let src = composed.src.expect("a quoted scalar has a token");
          self.scalar_end(src);
          let content = self.content(composed, props);
          let position = self.position(composed.range[0], composed.range[1]);
          let kind =
            if *kind == ScalarType::QuoteSingle { Kind::QuoteSingle } else { Kind::QuoteDouble };
          self.content_node(kind, position, content, value)
        }
      },
    }
  }

  /// The comments at the end of a flow scalar's token.
  fn scalar_end(&mut self, src: cst::Id) {
    if let Token::FlowScalar { end: Some(end), .. } = self.cst(src) {
      self.extract_comments(end);
    }
  }

  fn content_node(
    &mut self,
    kind: Kind,
    position: Position,
    content: Properties,
    value: &str,
  ) -> Id {
    let id = self.with_properties(Entry::new(kind, position), content, self.pending.len());
    self.root.set_value(id, value);
    id
  }

  /// Adds a node with its tag, anchor, the comments between them, and the children pending from `mark`.
  fn with_properties(&mut self, mut entry: Entry, content: Properties, mark: usize) -> Id {
    entry.tag = content.tag;
    entry.anchor = content.anchor;
    let id = self.add_pending(entry, mark);
    self.extra(id, Extra { middle: content.middle, ..Extra::default() });
    id
  }

  /// `transformPlain`
  fn plain(&mut self, composed: &Composed, value: &str, props: &[Source]) -> Id {
    if composed.range[0] == composed.range[1] {
      let text = self.text.as_bytes();
      let from = composed.range[0] as usize;
      let index =
        text[..from].iter().rposition(|it| !is_js_space(*it)).map_or(0, |at| at + 1) as u32;
      let content = self.content(composed, props);
      return self.content_node(Kind::Plain, self.position(index, index), content, "");
    }
    if let Some(src) = composed.src {
      self.scalar_end(src);
    }
    let content = self.content(composed, props);
    let position = self.position(composed.range[0], composed.range[1]);
    self.content_node(Kind::Plain, position, content, value)
  }

  /// `transformAstBlockValue`
  fn block_value(
    &mut self,
    composed: &Composed,
    kind: ScalarType,
    value: &str,
    props: &[Source],
  ) -> Id {
    let src = composed.src.expect("a block scalar has a token");
    let Token::BlockScalar { props: tokens, .. } = self.cst(src) else { unreachable!() };
    let mut header: Option<Source> = None;
    let mut indicator = None;
    for prop in tokens {
      let Prop::Source(token) = prop else { continue };
      match token.kind {
        cst::Kind::Comment => indicator = Some(self.comment(token)),
        cst::Kind::BlockScalarHeader => header = Some(*token),
        _ => {}
      }
    }
    let header = self.src(&header.expect("a block scalar has a header"));
    let (chomping, indent) = parse_header(header);
    let position = self.position(composed.range[0], composed.range[1]);
    let content = self.content(composed, props);
    let kind = if kind == ScalarType::BlockFolded { Kind::BlockFolded } else { Kind::BlockLiteral };
    let id = self.content_node(kind, position, content, value);
    let node = self.root.entry_mut(id);
    node.chomping = chomping;
    node.indent = indent;
    if let Some(comment) = indicator {
      self.root.extra_mut(id).indicator = Some(comment);
      self.root.entry_mut(comment).parent = Some(id);
    }
    id
  }

  /// `transformContentProperties`
  fn content(&mut self, composed: &Composed, tokens: &[Source]) -> Properties {
    let mut properties = Properties::default();
    let mut first: Option<(u32, u32)> = None;
    for token in tokens {
      let range = (token.offset, token.end);
      match token.kind {
        cst::Kind::Tag => {
          first.get_or_insert(range);
          let source = self.src(token);
          let skip = if source.starts_with("!!") { 2 } else { 1 };
          let value = match composed.tag.as_deref().unwrap_or(&source[skip..]) {
            "!" => "tag:yaml.org,2002:str",
            value => value,
          };
          let id = self.add(Entry::new(Kind::Tag, self.position(range.0, range.1)), &[]);
          self.root.set_value(id, value);
          properties.tag = Some(id);
        }
        cst::Kind::Anchor => {
          first.get_or_insert(range);
          let id = self.add(Entry::new(Kind::Anchor, self.position(range.0, range.1)), &[]);
          self.root.set_value(id, composed.anchor.as_deref().unwrap_or_default());
          properties.anchor = Some(id);
        }
        cst::Kind::Comment => {
          let comment = self.comment(token);
          if first.is_some_and(|first| first.0 <= range.0 && range.1 <= composed.range[0]) {
            properties.middle.push(comment);
          }
        }
        _ => {}
      }
    }
    properties
  }

  /// `transformMap`
  fn map(&mut self, composed: &Composed, props: &[Source]) -> Id {
    let src = composed.src.expect("a block map has a token");
    let items = self.items_of(src);
    let Content::Map { items: pairs, .. } = &composed.content else { unreachable!() };
    let mark = self.pending.len();
    for (index, pair) in pairs.iter().enumerate() {
      let child = self.pair(pair, &items[index.min(items.len() - 1)], Kind::MappingItem);
      self.pending.push(child);
    }
    for item in items.iter().skip(pairs.len()) {
      self.extract_comments(&item.start);
    }
    self.collection(composed, props, Kind::Mapping, mark)
  }

  /// Adds a block map or sequence with the items pending from `mark`.
  fn collection(&mut self, composed: &Composed, props: &[Source], kind: Kind, mark: usize) -> Id {
    let start = self.root.node(self.pending[mark]).position.start;
    let end = self.root.node(*self.pending.last().expect("a collection has an item")).position.end;
    let content = self.content(composed, props);
    self.with_properties(Entry::new(kind, Position { start, end }), content, mark)
  }

  fn items_of(&self, src: cst::Id) -> &'t [Item] {
    match self.cst(src) {
      Token::BlockMap { items, .. }
      | Token::BlockSeq { items, .. }
      | Token::FlowCollection { items, .. } => items,
      _ => &[],
    }
  }

  /// `transformSeq`
  fn seq(&mut self, composed: &Composed, props: &[Source]) -> Id {
    let src = composed.src.expect("a block sequence has a token");
    let items = self.items_of(src);
    let Content::Seq { items: nodes, .. } = &composed.content else { unreachable!() };
    let mark = self.pending.len();
    for (index, node) in nodes.iter().enumerate() {
      let item = &items[index.min(items.len() - 1)];
      let props: &[Source] = &item.start;
      let indicator = props.iter().rfind(|it| it.kind == cst::Kind::SeqItemInd).copied();
      let value = match self.is_empty(*node, props) {
        true => {
          self.comments(props);
          None
        }
        false => Some(self.node(*node, props)),
      };
      let start = match indicator {
        Some(indicator) => self.lines.point(indicator.offset),
        None => self.root.node(value.expect("an item has a value")).position.start,
      };
      let end = match value {
        Some(value) => self.root.node(value).position.end,
        None => self.lines.point(indicator.expect("an item has an indicator").end),
      };
      let item =
        self.add(Entry::new(Kind::SequenceItem, Position { start, end }), value.as_slice());
      self.pending.push(item);
    }
    for item in items.iter().skip(nodes.len()) {
      self.extract_comments(&item.start);
    }
    self.collection(composed, props, Kind::Sequence, mark)
  }

  /// `transformFlowMap` and `transformFlowSeq`
  fn flow_map(&mut self, composed: &Composed, props: &[Source]) -> Id {
    let Content::Map { items: pairs, .. } = &composed.content else { unreachable!() };
    let src = composed.src.expect("a flow map has a token");
    let items = self.items_of(src);
    let mark = self.pending.len();
    for (index, pair) in pairs.iter().enumerate() {
      let child = self.pair(pair, &items[index.min(items.len() - 1)], Kind::FlowMappingItem);
      self.pending.push(child);
    }
    self.flow_collection(composed, props, src, pairs.len(), mark, Kind::FlowMapping)
  }

  fn flow_seq(&mut self, composed: &Composed, props: &[Source]) -> Id {
    let Content::Seq { items: nodes, .. } = &composed.content else { unreachable!() };
    let src = composed.src.expect("a flow sequence has a token");
    let items = self.items_of(src);
    let mark = self.pending.len();
    for (index, node) in nodes.iter().enumerate() {
      let item = &items[index.min(items.len() - 1)];
      let child = self.composed(*node);
      let pair = match &child.content {
        Content::Map { items: pairs, .. } if child.src.is_none() && pairs.len() == 1 => {
          pairs[0].src.filter(|it| it.0 == src && it.1 == index).map(|_| pairs[0].clone())
        }
        _ => None,
      };
      let child = match pair {
        Some(pair) => self.pair(&pair, item, Kind::FlowMappingItem),
        None => {
          let value = self.node(*node, &item.start);
          let position = self.root.node(value).position;
          self.add(Entry::new(Kind::FlowSequenceItem, position), &[value])
        }
      };
      self.pending.push(child);
    }
    self.flow_collection(composed, props, src, nodes.len(), mark, Kind::FlowSequence)
  }

  /// Adds a flow collection with the items pending from `mark`, after `count` items with content.
  fn flow_collection(
    &mut self,
    composed: &Composed,
    props: &[Source],
    src: cst::Id,
    count: usize,
    mark: usize,
    kind: Kind,
  ) -> Id {
    for item in self.items_of(src).iter().skip(count) {
      self.extract_comments(&item.start);
    }
    let Token::FlowCollection { start, end, .. } = self.cst(src) else { unreachable!() };
    let start = *start;
    let rest = self.extract_comments(end);
    let close =
      rest.iter().find(|it| matches!(it.kind, cst::Kind::FlowMapEnd | cst::Kind::FlowSeqEnd));
    let close = close.expect("a flow collection ends");
    let position = self.position(start.offset, close.end);
    let content = self.content(composed, props);
    self.with_properties(Entry::new(kind, position), content, mark)
  }

  /// `isEmptyNode`
  /// `isEmptyNode`: the node is empty, and its props have no tag or anchor.
  fn is_empty(&self, node: Cid, props: &[Source]) -> bool {
    let range = self.composed(node).range;
    range[0] == range[1]
      && !props.iter().any(|it| matches!(it.kind, cst::Kind::Tag | cst::Kind::Anchor))
  }

  /// `transformPair`
  fn pair(&mut self, pair: &Pair, item: &Item, kind: Kind) -> Id {
    // The props are the tokens themselves: what reads them only reads comments, tags and anchors.
    let key_props: &[Source] = &item.start;
    let value_props: &[Source] = item.sep.as_deref().unwrap_or_default();
    let explicit = key_props.iter().rfind(|it| it.kind == cst::Kind::ExplicitKeyInd).copied();
    let indicator = value_props.iter().rfind(|it| it.kind == cst::Kind::MapValueInd).copied();
    let key_token = match item.key {
      Key::Token(key) => Some(key),
      _ => None,
    };
    let key_start = explicit
      .map(|it| it.offset)
      .or(key_token.map(|key| self.cst(key).offset()))
      .or(indicator.map(|it| it.offset))
      .unwrap_or_else(|| self.cst(item.value.expect("an item has a value")).offset());
    let key_end = match (key_token, explicit) {
      (Some(_), _) => self.composed(pair.key).range[1],
      (None, Some(explicit)) => explicit.end,
      (None, None) => key_start,
    };
    let value_range = pair.value.map(|value| {
      let start = indicator
        .map(|it| it.offset)
        .or(item.value.map(|value| self.cst(value).offset()))
        .unwrap_or(self.composed(value).range[0]);
      let end = match (item.value, indicator) {
        (Some(_), _) => self.composed(value).range[1],
        (None, Some(indicator)) => indicator.end,
        (None, None) => start,
      };
      (start, end)
    });
    let key_content = match self.is_empty(pair.key, key_props) {
      true => {
        self.comments(key_props);
        None
      }
      false => Some(self.node(pair.key, key_props)),
    };
    let value_content = match pair.value {
      Some(value) if !self.is_empty(value, value_props) => Some(self.node(value, value_props)),
      _ => {
        self.comments(value_props);
        None
      }
    };
    let key_end =
      key_content.map_or(key_end, |content| self.root.node(content).position.end.offset);
    let key = Entry::new(Kind::MappingKey, self.position(key_start, key_end));
    let key = self.add(key, key_content.as_slice());
    let value = match (value_content, value_range) {
      (None, None) => None,
      (content, range) => {
        let start = range.map_or_else(
          || self.root.node(content.expect("content")).position.start.offset,
          |it| it.0,
        );
        let end = match (content, range) {
          (Some(content), _) => self.root.node(content).position.end.offset,
          (None, Some(range)) => range.0 + 1,
          (None, None) => unreachable!(),
        };
        let value = Entry::new(Kind::MappingValue, self.position(start, end));
        Some(self.add(value, content.as_slice()))
      }
    };
    let key_position = self.root.node(key).position;
    let end = value.map_or(key_position.end, |value| self.root.node(value).position.end);
    let value = value.unwrap_or_else(|| {
      let empty = Position { start: key_position.end, end: key_position.end };
      self.add(Entry::new(Kind::MappingValue, empty), &[])
    });
    self.add(Entry::new(kind, Position { start: key_position.start, end }), &[key, value])
  }
}

#[derive(Default)]
struct Properties {
  tag: Option<Id>,
  anchor: Option<Id>,
  middle: Vec<Id>,
}

fn is_space(token: &Source) -> bool {
  matches!(token.kind, cst::Kind::Space | cst::Kind::Newline)
}

/// `/\S/` doesn't match it: JavaScript's white space, of which a YAML text only has ASCII outside scalars.
fn is_js_space(byte: u8) -> bool {
  matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

/// `parseHeader`: the chomping and the explicit indent of a block scalar's header.
fn parse_header(header: &str) -> (Chomping, Option<u32>) {
  let bytes = header.as_bytes();
  let mut end = bytes.len();
  let trailing = match bytes.last() {
    Some(sign @ (b'+' | b'-')) => {
      end -= 1;
      Some(*sign)
    }
    _ => None,
  };
  let digits_start =
    bytes[..end].iter().rposition(|it| !it.is_ascii_digit()).map_or(0, |at| at + 1);
  let indent = header[digits_start..end].parse().ok();
  let leading = match digits_start.checked_sub(1).map(|at| bytes[at]) {
    Some(sign @ (b'+' | b'-')) => Some(sign),
    _ => None,
  };
  let chomping = match trailing.or(leading) {
    Some(b'+') => Chomping::Keep,
    Some(b'-') => Chomping::Strip,
    _ => Chomping::Clip,
  };
  (chomping, indent)
}
