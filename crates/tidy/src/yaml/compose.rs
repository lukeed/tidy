//! The composer of `yaml` 2.9.0 (`src/compose/*`), as Prettier runs it: `keepSourceTokens`, `uniqueKeys: false`,
//! `merge: true`, strict. It turns the syntax tree into documents of nodes, each with its range and its source token.
//! Prettier throws on the first error of a document, so this stops at the first error. Warnings are ignored.

use super::Abort;
use super::cst::{self, Item, Key, Kind, Node, Prop, ScalarKind, Source, Tree};
use super::scalar;
use crate::syntax::{Message, Rejection};

pub(crate) type Cid = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScalarType {
  Plain,
  QuoteSingle,
  QuoteDouble,
  BlockLiteral,
  BlockFolded,
}

/// A pair of a map, with the collection item it comes from.
#[derive(Clone, Debug)]
pub(crate) struct Pair {
  pub(crate) key: Cid,
  pub(crate) value: Option<Cid>,
  /// The collection, and the index of its item.
  pub(crate) src: Option<(cst::Id, usize)>,
}

#[derive(Clone, Debug)]
pub(crate) enum Content {
  Scalar { kind: ScalarType, value: String },
  Alias { name: String },
  Map { flow: bool, items: Vec<Pair> },
  Seq { flow: bool, items: Vec<Cid> },
}

/// A node of a document: `yaml`'s `ParsedNode`.
#[derive(Clone, Debug)]
pub(crate) struct Composed {
  pub(crate) content: Content,
  /// The start, the end of the value, and the end of the node with its comments.
  pub(crate) range: [u32; 3],
  pub(crate) tag: Option<String>,
  pub(crate) anchor: Option<String>,
  pub(crate) src: Option<cst::Id>,
}

/// A document: its contents, its range, and whether it has a `---` marker.
#[derive(Clone, Debug)]
pub(crate) struct Document {
  pub(crate) contents: Option<Cid>,
  pub(crate) range: [u32; 3],
  pub(crate) doc_start: bool,
}

#[derive(Default)]
pub(crate) struct Composition {
  pub(crate) nodes: Vec<Composed>,
  pub(crate) documents: Vec<Document>,
}

impl Composition {
  pub(crate) fn clear(&mut self) {
    self.nodes.clear();
    self.documents.clear();
  }

  pub(crate) fn node(&self, id: Cid) -> &Composed {
    &self.nodes[id as usize]
  }
}

/// `Directives`: the YAML version and the tag handles of the stream and of each document.
#[derive(Clone)]
struct Directives {
  version: Version,
  tags: Vec<(String, String)>,
  at_next_document: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Version {
  V11,
  V12,
}

impl Directives {
  fn new(version: Version) -> Directives {
    Directives { version, tags: default_tags(), at_next_document: false }
  }

  /// `atDocument`: the directives of a document that starts now.
  fn at_document(&mut self) -> Directives {
    let document =
      Directives { version: self.version, tags: self.tags.clone(), at_next_document: false };
    match self.version {
      Version::V11 => self.at_next_document = true,
      Version::V12 => {
        self.at_next_document = false;
        self.version = Version::V12;
        self.tags = default_tags();
      }
    }
    document
  }

  /// `add`: a `%YAML` or `%TAG` directive. Returns the message of an error, not of a warning.
  fn add(&mut self, line: &str) -> Option<Message> {
    if self.at_next_document {
      self.version = Version::V11;
      self.tags = default_tags();
      self.at_next_document = false;
    }
    let mut parts = line.trim_matches(is_js_space).split([' ', '\t']).filter(|it| !it.is_empty());
    let name = parts.next();
    let parts: Vec<&str> = parts.collect();
    match name {
      Some("%TAG") => {
        if parts.len() < 2 {
          return Some(Message::BadDirective);
        }
        let error = (parts.len() != 2).then_some(Message::BadDirective);
        let (handle, prefix) = (parts[0].to_owned(), parts[1].to_owned());
        match self.tags.iter_mut().find(|(it, _)| *it == handle) {
          Some(entry) => entry.1 = prefix,
          None => self.tags.push((handle, prefix)),
        }
        error
      }
      Some("%YAML") => {
        if parts.len() != 1 {
          return Some(Message::BadDirective);
        }
        match parts[0] {
          "1.1" => self.version = Version::V11,
          "1.2" => self.version = Version::V12,
          version => {
            let valid = version.split_once('.').is_some_and(|(a, b)| {
              !a.is_empty()
                && !b.is_empty()
                && (a.bytes().chain(b.bytes())).all(|it| it.is_ascii_digit())
            });
            if !valid {
              return Some(Message::BadDirective);
            }
          }
        }
        None
      }
      _ => None,
    }
  }

  /// `tagName`: the full name of a tag, or `None` and the reason it has none.
  fn tag_name(&self, source: &str) -> std::result::Result<String, Message> {
    if source == "!" {
      return Ok("!".into());
    }
    if !source.starts_with('!') {
      return Err(Message::BadTag);
    }
    if let Some(verbatim) = source.strip_prefix("!<") {
      let verbatim = verbatim.get(..verbatim.len().saturating_sub(1)).unwrap_or_default();
      if verbatim == "!" || verbatim == "!!" || !source.ends_with('>') {
        return Err(Message::BadTag);
      }
      return Ok(verbatim.into());
    }
    let split = source.rfind('!').expect("a tag starts with !") + 1;
    let (handle, suffix) = source.split_at(split);
    if suffix.is_empty() {
      return Err(Message::BadTag);
    }
    if let Some((_, prefix)) = self.tags.iter().find(|(it, _)| it == handle) {
      return decode_uri_component(suffix)
        .map(|suffix| format!("{prefix}{suffix}"))
        .ok_or(Message::BadTag);
    }
    if handle == "!" {
      return Ok(source.into());
    }
    Err(Message::BadTag)
  }
}

fn default_tags() -> Vec<(String, String)> {
  vec![("!!".into(), "tag:yaml.org,2002:".into())]
}

/// JavaScript's `String.prototype.trim` whitespace, of which only ASCII matters in a directive line.
fn is_js_space(character: char) -> bool {
  character.is_whitespace() || character == '\u{feff}'
}

/// `decodeURIComponent`: `None` where it throws.
fn decode_uri_component(text: &str) -> Option<String> {
  let bytes = text.as_bytes();
  let mut out = Vec::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'%' {
      let hex = text.get(i + 1..i + 3)?;
      out.push(u8::from_str_radix(hex, 16).ok()?);
      i += 3;
    } else {
      out.push(bytes[i]);
      i += 1;
    }
  }
  String::from_utf8(out).ok()
}

/// What follows a list of tokens: `resolveProps`'s `next`.
#[derive(Clone, Copy)]
enum Next {
  None,
  Node(cst::Id),
  Source(Source),
}

/// `resolveProps`'s result.
#[derive(Clone, Debug, Default)]
struct Props {
  comma: Option<Source>,
  found: Option<Source>,
  comment: String,
  has_newline: bool,
  anchor: Option<Source>,
  tag: Option<Source>,
  newline_after_prop: Option<Source>,
  end: u32,
  start: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Indicator {
  DocStart,
  ExplicitKey,
  MapValue,
  SeqItem,
}

impl Indicator {
  fn kind(self) -> Kind {
    match self {
      Indicator::DocStart => Kind::DocStart,
      Indicator::ExplicitKey => Kind::ExplicitKeyInd,
      Indicator::MapValue => Kind::MapValueInd,
      Indicator::SeqItem => Kind::SeqItemInd,
    }
  }
}

pub(crate) struct Composer<'c> {
  pub(crate) text: &'c str,
  pub(crate) tree: &'c Tree,
  pub(crate) out: &'c mut Composition,
  pub(crate) rejection: &'c Rejection,
  pub(crate) stack: &'c crate::stack::Stack,
  directives: Directives,
  at_root: bool,
}

type Result<T> = std::result::Result<T, Abort>;

impl<'c> Composer<'c> {
  pub(crate) fn new(
    text: &'c str,
    tree: &'c Tree,
    out: &'c mut Composition,
    rejection: &'c Rejection,
    stack: &'c crate::stack::Stack,
  ) -> Self {
    Composer {
      text,
      tree,
      out,
      rejection,
      stack,
      directives: Directives::new(Version::V12),
      at_root: true,
    }
  }

  fn error(&self, message: Message, at: u32) -> Abort {
    Abort::Syntax(self.rejection.note(message, at))
  }

  fn src(&self, source: &Source) -> &'c str {
    &self.text[source.offset as usize..source.end as usize]
  }

  fn node(&self, id: cst::Id) -> &'c Node {
    let tree: &'c Tree = self.tree;
    tree.node(id)
  }

  /// `Composer.compose(tokens, true, text.length)`
  pub(crate) fn compose(&mut self) -> Result<()> {
    let tree: &'c Tree = self.tree;
    let mut at_directives = false;
    let mut current: Option<Document> = None;
    for &token in &tree.tokens {
      match self.node(token) {
        Node::Directive { offset, end } => {
          let line = &self.text[*offset as usize..*end as usize];
          if let Some(message) = self.directives.add(line) {
            return Err(self.error(message, *offset));
          }
          at_directives = true;
        }
        Node::Document { offset, .. } => {
          let document = self.document(token)?;
          if at_directives && !document.doc_start {
            return Err(self.error(Message::ExpectedDocumentStart, *offset));
          }
          if let Some(previous) = current.replace(document) {
            self.out.documents.push(previous);
          }
          at_directives = false;
        }
        Node::Source(_) => {}
        Node::Error { offset, .. } => return Err(self.error(Message::UnexpectedToken, *offset)),
        Node::DocumentEnd { offset, source_end, end } => {
          let Some(document) = current.as_mut() else {
            return Err(self.error(Message::UnexpectedToken, *offset));
          };
          let end = self.resolve_end(end.as_deref(), *source_end, true)?;
          document.range[2] = end;
        }
        node => return Err(self.error(Message::UnexpectedToken, node.offset())),
      }
    }
    match current {
      Some(document) => self.out.documents.push(document),
      None => {
        let end = self.text.len() as u32;
        if at_directives {
          return Err(self.error(Message::ExpectedDocumentStart, end));
        }
        self.out.documents.push(Document {
          contents: None,
          range: [0, end, end],
          doc_start: false,
        });
      }
    }
    Ok(())
  }

  /// `composeDoc`
  fn document(&mut self, id: cst::Id) -> Result<Document> {
    let Node::Document { offset, start, value, end } = self.node(id) else { unreachable!() };
    let directives = self.directives.at_document();
    let saved = std::mem::replace(&mut self.directives, directives);
    self.at_root = true;
    let next = match (value, end.as_ref().and_then(|end| end.first())) {
      (Some(value), _) => Next::Node(*value),
      (None, Some(first)) => Next::Source(*first),
      _ => Next::None,
    };
    let props = self.resolve_props(start, Indicator::DocStart, None, next, *offset, 0, true)?;
    let doc_start = props.found.is_some();
    if doc_start
      && let Some(value) = value
      && matches!(self.node(*value), Node::BlockMap { .. } | Node::BlockSeq { .. })
      && !props.has_newline
    {
      return Err(self.error(Message::BlockAfterDocumentStart, props.end));
    }
    let contents = match value {
      Some(value) => self.compose_node(*value, &props)?,
      None => self.compose_empty(props.end, Some(start), None, &props)?,
    };
    let content_end = self.out.node(contents).range[2];
    let end = self.resolve_end(end.as_deref(), content_end, false)?;
    let document =
      Document { contents: Some(contents), range: [*offset, content_end, end], doc_start };
    self.directives = saved;
    Ok(document)
  }

  fn push(&mut self, node: Composed) -> Cid {
    self.out.nodes.push(node);
    self.out.nodes.len() as Cid - 1
  }

  /// `composeNode`
  fn compose_node(&mut self, token: cst::Id, props: &Props) -> Result<Cid> {
    if !self.stack.has_room() {
      return Err(Abort::TooDeep);
    }
    let node = match self.node(token) {
      Node::FlowScalar { kind: ScalarKind::Alias, .. } => {
        let id = self.compose_alias(token)?;
        if props.anchor.is_some() || props.tag.is_some() {
          return Err(self.error(Message::AliasWithProperties, self.node(token).offset()));
        }
        id
      }
      Node::FlowScalar { .. } | Node::BlockScalar { .. } => {
        let id = self.compose_scalar(token, props.tag)?;
        self.set_anchor(id, props.anchor)?;
        id
      }
      Node::BlockMap { .. } | Node::BlockSeq { .. } | Node::FlowCollection { .. } => {
        let id = self.compose_collection(token, props)?;
        self.set_anchor(id, props.anchor)?;
        id
      }
      node => return Err(self.error(Message::UnexpectedToken, node.offset())),
    };
    self.out.nodes[node as usize].src = Some(token);
    Ok(node)
  }

  fn set_anchor(&mut self, id: Cid, anchor: Option<Source>) -> Result<()> {
    let Some(anchor) = anchor else { return Ok(()) };
    let name = &self.src(&anchor)[1..];
    if name.is_empty() {
      return Err(self.error(Message::BadAnchor, anchor.offset));
    }
    self.out.nodes[id as usize].anchor = Some(name.to_owned());
    Ok(())
  }

  /// `composeEmptyNode`
  fn compose_empty(
    &mut self,
    offset: u32,
    before: Option<&[Source]>,
    pos: Option<usize>,
    props: &Props,
  ) -> Result<Cid> {
    let offset = empty_scalar_position(offset, before, pos);
    let tag = match props.tag {
      Some(tag) => Some(self.tag_name(tag)?),
      None => None,
    };
    let mut node = Composed {
      content: Content::Scalar { kind: ScalarType::Plain, value: String::new() },
      range: [offset, offset, offset],
      tag,
      anchor: None,
      src: None,
    };
    if !props.comment.is_empty() {
      node.range[2] = props.end;
    }
    let id = self.push(node);
    self.set_anchor(id, props.anchor)?;
    Ok(id)
  }

  /// `composeAlias`
  fn compose_alias(&mut self, token: cst::Id) -> Result<Cid> {
    let Node::FlowScalar { offset, source_end, end, .. } = self.node(token) else { unreachable!() };
    let name = &self.text[*offset as usize + 1..*source_end as usize];
    if name.is_empty() {
      return Err(self.error(Message::BadAnchor, *offset));
    }
    let end = self.resolve_end(end.as_deref(), *source_end, true)?;
    let node = Composed {
      content: Content::Alias { name: name.to_owned() },
      range: [*offset, *source_end, end],
      tag: None,
      anchor: None,
      src: None,
    };
    Ok(self.push(node))
  }

  /// `composeScalar`
  fn compose_scalar(&mut self, token: cst::Id, tag: Option<Source>) -> Result<Cid> {
    let (kind, value, range) = match self.node(token) {
      Node::BlockScalar { .. } => self.block_scalar(token)?,
      _ => self.flow_scalar(token)?,
    };
    let tag = match tag {
      Some(tag) => Some(self.tag_name(tag)?),
      None => None,
    };
    if let Some(name) = &tag
      && name == "tag:yaml.org,2002:timestamp"
      && !scalar::is_timestamp(&value)
    {
      return Err(self.error(Message::BadTag, range[0]));
    }
    let node =
      Composed { content: Content::Scalar { kind, value }, range, tag, anchor: None, src: None };
    Ok(self.push(node))
  }

  fn tag_name(&self, tag: Source) -> Result<String> {
    self.directives.tag_name(self.src(&tag)).map_err(|message| self.error(message, tag.offset))
  }

  /// `resolveFlowScalar`
  fn flow_scalar(&self, token: cst::Id) -> Result<(ScalarType, String, [u32; 3])> {
    let Node::FlowScalar { kind, offset, source_end, end, .. } = self.node(token) else {
      unreachable!()
    };
    let source = &self.text[*offset as usize..*source_end as usize];
    let error = |(message, at): (Message, usize)| self.error(message, *offset + at as u32);
    let (kind, value) = match kind {
      ScalarKind::Plain => (ScalarType::Plain, scalar::plain(source).map_err(error)?),
      ScalarKind::Single => {
        (ScalarType::QuoteSingle, scalar::single_quoted(source).map_err(error)?)
      }
      _ => (ScalarType::QuoteDouble, scalar::double_quoted(source).map_err(error)?),
    };
    let end = self.resolve_end(end.as_deref(), *source_end, true)?;
    Ok((kind, value, [*offset, *source_end, end]))
  }

  /// `resolveBlockScalar`
  fn block_scalar(&self, token: cst::Id) -> Result<(ScalarType, String, [u32; 3])> {
    let Node::BlockScalar { offset, indent, props, source } = self.node(token) else {
      unreachable!()
    };
    let header = match props.first() {
      Some(Prop::Source(header)) if header.kind == Kind::BlockScalarHeader => *header,
      _ => return Err(self.error(Message::UnexpectedToken, *offset)),
    };
    let mut length = (header.end - header.offset) as usize;
    let mut has_space = false;
    for prop in &props[1..] {
      match prop {
        Prop::Source(token) => match token.kind {
          Kind::Space => {
            has_space = true;
            length += (token.end - token.offset) as usize;
          }
          Kind::Newline => length += (token.end - token.offset) as usize,
          Kind::Comment if !has_space => {
            return Err(self.error(Message::CommentWithoutSpace, token.offset));
          }
          Kind::Comment => length += (token.end - token.offset) as usize,
          _ => return Err(self.error(Message::UnexpectedToken, token.offset)),
        },
        Prop::Token(id) => {
          return Err(self.error(Message::UnexpectedToken, self.node(*id).offset()));
        }
      }
    }
    let content = &self.text[source.0 as usize..source.1 as usize];
    let at_root = self.at_root;
    let result = scalar::block(
      self.src(&header),
      content,
      *indent as usize,
      *offset as usize,
      length,
      at_root,
    )
    .map_err(|(message, at)| self.error(message, at as u32))?;
    let end = *offset + length as u32 + (source.1 - source.0);
    Ok((result.0, result.1, [*offset, end, end]))
  }

  /// `resolveEnd`: the offset after the tokens at the end of a node, which can only be spaces, comments and line
  /// breaks.
  fn resolve_end(&self, end: Option<&[Source]>, mut offset: u32, req_space: bool) -> Result<u32> {
    let mut has_space = false;
    for token in end.unwrap_or_default() {
      match token.kind {
        Kind::Space => has_space = true,
        Kind::Comment if req_space && !has_space => {
          return Err(self.error(Message::CommentWithoutSpace, token.offset));
        }
        Kind::Comment => {}
        Kind::Newline => has_space = true,
        _ => return Err(self.error(Message::UnexpectedToken, token.offset)),
      }
      offset += token.end - token.offset;
    }
    Ok(offset)
  }

  /// `resolveProps`
  #[allow(clippy::too_many_arguments)]
  fn resolve_props(
    &self,
    tokens: &[Source],
    indicator: Indicator,
    flow: Option<bool>,
    next: Next,
    offset: u32,
    parent_indent: u32,
    start_on_newline: bool,
  ) -> Result<Props> {
    let mut props = Props::default();
    let mut at_newline = start_on_newline;
    let mut has_space = start_on_newline;
    let mut comment_sep = String::new();
    let mut req_space = false;
    let mut tab: Option<Source> = None;
    let mut start: Option<u32> = None;
    let next_kind = |next: Next| -> Option<NextKind> {
      match next {
        Next::None => None,
        Next::Source(source) => Some(match source.kind {
          Kind::Space => NextKind::Space,
          Kind::Newline => NextKind::Newline,
          Kind::Comma => NextKind::Comma,
          _ => NextKind::Other,
        }),
        Next::Node(id) => Some(match self.node(id) {
          Node::FlowCollection { .. } => NextKind::FlowCollection,
          Node::BlockMap { .. } | Node::BlockSeq { .. } => NextKind::Block,
          Node::FlowScalar { kind: ScalarKind::Plain, offset, source_end, .. }
            if offset == source_end =>
          {
            NextKind::EmptyScalar
          }
          _ => NextKind::Other,
        }),
      }
    };
    let next_offset = |next: Next| match next {
      Next::None => 0,
      Next::Source(source) => source.offset,
      Next::Node(id) => self.node(id).offset(),
    };
    for token in tokens {
      if req_space {
        if !matches!(token.kind, Kind::Space | Kind::Newline | Kind::Comma) {
          return Err(self.error(Message::PropertyWithoutSpace, token.offset));
        }
        req_space = false;
      }
      if let Some(tab) = tab.take()
        && at_newline
        && !matches!(token.kind, Kind::Comment | Kind::Newline)
      {
        return Err(self.error(Message::TabIndent, tab.offset));
      }
      match token.kind {
        Kind::Space => {
          let flow_doc =
            indicator == Indicator::DocStart && next_kind(next) == Some(NextKind::FlowCollection);
          if flow.is_none() && !flow_doc && self.src(token).contains('\t') {
            tab = Some(*token);
          }
          has_space = true;
        }
        Kind::Comment => {
          if !has_space {
            return Err(self.error(Message::CommentWithoutSpace, token.offset));
          }
          let cb = match &self.src(token)[1..] {
            "" => " ",
            text => text,
          };
          if props.comment.is_empty() {
            props.comment = cb.to_owned();
          } else {
            props.comment.push_str(&comment_sep);
            props.comment.push_str(cb);
          }
          comment_sep.clear();
          at_newline = false;
        }
        Kind::Newline => {
          if !at_newline {
            comment_sep.push_str(self.src(token));
          } else if !props.comment.is_empty() {
            props.comment.push_str(self.src(token));
          }
          at_newline = true;
          props.has_newline = true;
          if props.anchor.is_some() || props.tag.is_some() {
            props.newline_after_prop = Some(*token);
          }
          has_space = true;
        }
        Kind::Anchor => {
          if props.anchor.is_some() {
            return Err(self.error(Message::MultipleAnchors, token.offset));
          }
          props.anchor = Some(*token);
          start.get_or_insert(token.offset);
          at_newline = false;
          has_space = false;
          req_space = true;
        }
        Kind::Tag => {
          if props.tag.is_some() {
            return Err(self.error(Message::MultipleTags, token.offset));
          }
          props.tag = Some(*token);
          start.get_or_insert(token.offset);
          at_newline = false;
          has_space = false;
          req_space = true;
        }
        kind if kind == indicator.kind() => {
          if props.anchor.is_some() || props.tag.is_some() || props.found.is_some() {
            return Err(self.error(Message::UnexpectedToken, token.offset));
          }
          props.found = Some(*token);
          at_newline = matches!(indicator, Indicator::SeqItem | Indicator::ExplicitKey);
          has_space = false;
        }
        Kind::Comma if flow.is_some() => {
          if props.comma.is_some() {
            return Err(self.error(Message::UnexpectedToken, token.offset));
          }
          props.comma = Some(*token);
          at_newline = false;
          has_space = false;
        }
        _ => return Err(self.error(Message::UnexpectedToken, token.offset)),
      }
    }
    props.end = tokens.last().map_or(offset, |last| last.end);
    let kind = next_kind(next);
    if req_space
      && kind.is_some()
      && !matches!(
        kind,
        Some(NextKind::Space | NextKind::Newline | NextKind::Comma | NextKind::EmptyScalar)
      )
    {
      return Err(self.error(Message::PropertyWithoutSpace, next_offset(next)));
    }
    if let Some(tab) = tab
      && ((at_newline && tab.indent <= parent_indent) || kind == Some(NextKind::Block))
    {
      return Err(self.error(Message::TabIndent, tab.offset));
    }
    props.start = start.unwrap_or(props.end);
    Ok(props)
  }

  /// `composeCollection`
  fn compose_collection(&mut self, token: cst::Id, props: &Props) -> Result<Cid> {
    let tag_name = match props.tag {
      Some(tag) => Some(self.tag_name(tag)?),
      None => None,
    };
    let node = self.node(token);
    if matches!(node, Node::BlockSeq { .. }) {
      let last = match (props.anchor, props.tag) {
        (Some(anchor), Some(tag)) => Some(if anchor.offset > tag.offset { anchor } else { tag }),
        (anchor, tag) => anchor.or(tag),
      };
      if let Some(last) = last
        && props.newline_after_prop.is_none_or(|nl| nl.offset < last.offset)
      {
        return Err(self.error(Message::MissingNewlineAfterProps, last.offset));
      }
    }
    let map = match node {
      Node::BlockMap { .. } => true,
      Node::BlockSeq { .. } => false,
      Node::FlowCollection { start, .. } => start.kind == Kind::FlowMapStart,
      _ => unreachable!(),
    };
    let id = match node {
      Node::BlockMap { .. } => self.block_map(token)?,
      Node::BlockSeq { .. } => self.block_seq(token)?,
      _ => self.flow_collection(token)?,
    };
    let tag = tag_name.map(|name| match (name.as_str(), map) {
      ("!", true) => "tag:yaml.org,2002:map".to_owned(),
      ("!", false) => "tag:yaml.org,2002:seq".to_owned(),
      _ => name,
    });
    if let Some(tag) = &tag {
      self.check_collection_tag(id, tag, map, props)?;
    }
    self.out.nodes[id as usize].tag = tag;
    Ok(id)
  }

  /// The errors of the collection tags of YAML 1.1 that `yaml` knows: `!!omap`, `!!pairs` and `!!set`.
  fn check_collection_tag(&self, id: Cid, tag: &str, map: bool, props: &Props) -> Result<()> {
    let at = props.tag.map_or(0, |tag| tag.offset);
    let node = self.out.node(id);
    match (tag, &node.content) {
      ("tag:yaml.org,2002:omap" | "tag:yaml.org,2002:pairs", Content::Seq { items, .. })
        if !map =>
      {
        let mut seen: Vec<String> = Vec::new();
        for item in items {
          let key = match &self.out.node(*item).content {
            Content::Map { items, .. } if items.len() > 1 => {
              return Err(self.error(Message::BadTag, at));
            }
            Content::Map { items, .. } => items.first().map(|pair| pair.key),
            _ => Some(*item),
          };
          if tag.ends_with("omap")
            && let Some(key) = key
            && let Content::Scalar { value, .. } = &self.out.node(key).content
          {
            if seen.contains(value) {
              return Err(self.error(Message::BadTag, at));
            }
            seen.push(value.clone());
          }
        }
        Ok(())
      }
      ("tag:yaml.org,2002:set", Content::Map { items, .. }) if map => {
        let null = |value: &Option<Cid>| match value.map(|it| self.out.node(it)) {
          None => true,
          Some(Composed {
            content: Content::Scalar { kind: ScalarType::Plain, value },
            tag: None,
            ..
          }) => {
            matches!(value.as_str(), "" | "~" | "null" | "Null" | "NULL")
          }
          _ => false,
        };
        match items.iter().all(|pair| null(&pair.value)) {
          true => Ok(()),
          false => Err(self.error(Message::BadTag, at)),
        }
      }
      _ => Ok(()),
    }
  }

  /// `resolveBlockMap`
  fn block_map(&mut self, token: cst::Id) -> Result<Cid> {
    let Node::BlockMap { offset: map_offset, indent, items } = self.node(token) else {
      unreachable!()
    };
    let (map_offset, indent) = (*map_offset, *indent);
    self.at_root = false;
    let mut offset = map_offset;
    let mut comment_end: Option<u32> = None;
    let mut pairs = Vec::new();
    for (index, item) in items.iter().enumerate() {
      let Item { start, key, sep, value, .. } = item;
      let next = match (key, sep.as_ref().and_then(|sep| sep.first())) {
        (Key::Token(key), _) => Next::Node(*key),
        (_, Some(first)) => Next::Source(*first),
        _ => Next::None,
      };
      let key_props =
        self.resolve_props(start, Indicator::ExplicitKey, None, next, offset, indent, true)?;
      let implicit = key_props.found.is_none();
      if implicit {
        if let Key::Token(key) = key {
          match self.node(*key) {
            Node::BlockSeq { .. } => return Err(self.error(Message::BadIndent, offset)),
            node if node.indent_of().is_some_and(|it| it != indent) => {
              return Err(self.error(Message::BadIndent, offset));
            }
            _ => {}
          }
        }
        if key_props.anchor.is_none() && key_props.tag.is_none() && sep.is_none() {
          comment_end = Some(key_props.end);
          continue;
        }
        let key_token = match key {
          Key::Token(key) => Some(*key),
          _ => None,
        };
        if key_props.newline_after_prop.is_some() || self.contains_newline(key_token) {
          return Err(self.error(Message::MultilineImplicitKey, offset));
        }
      } else if key_props.found.is_some_and(|found| found.indent != indent) {
        return Err(self.error(Message::BadIndent, offset));
      }
      let key_start = key_props.end;
      let key_node = match key {
        Key::Token(key) => self.compose_node(*key, &key_props)?,
        _ => self.compose_empty(key_start, Some(start), None, &key_props)?,
      };
      let key_end = self.out.node(key_node).range[2];
      let next = value.map_or(Next::None, Next::Node);
      let starts_on_newline = match key {
        Key::Token(key) => matches!(self.node(*key), Node::BlockScalar { .. }),
        _ => true,
      };
      let sep_tokens: &[Source] = sep.as_deref().unwrap_or_default();
      let value_props = self.resolve_props(
        sep_tokens,
        Indicator::MapValue,
        None,
        next,
        key_end,
        indent,
        starts_on_newline,
      )?;
      offset = value_props.end;
      let src = Some((token, index));
      if let Some(found) = value_props.found {
        if implicit {
          if value.is_some_and(|value| matches!(self.node(value), Node::BlockMap { .. }))
            && !value_props.has_newline
          {
            return Err(self.error(Message::BadIndent, offset));
          }
          if key_props.start + 1024 < found.offset {
            return Err(self.error(Message::KeyTooLong, key_start));
          }
        }
        let value_node = match value {
          Some(value) => self.compose_node(*value, &value_props)?,
          None => self.compose_empty(offset, Some(sep_tokens), None, &value_props)?,
        };
        offset = self.out.node(value_node).range[2];
        pairs.push(Pair { key: key_node, value: Some(value_node), src });
      } else {
        if implicit {
          return Err(self.error(Message::ExpectedColon, key_start));
        }
        pairs.push(Pair { key: key_node, value: None, src });
      }
    }
    if let Some(end) = comment_end
      && end != 0
      && end < offset
    {
      return Err(self.error(Message::UnexpectedToken, end));
    }
    let node = Composed {
      content: Content::Map { flow: false, items: pairs },
      range: [map_offset, offset, comment_end.unwrap_or(offset)],
      tag: None,
      anchor: None,
      src: None,
    };
    Ok(self.push(node))
  }

  /// `resolveBlockSeq`
  fn block_seq(&mut self, token: cst::Id) -> Result<Cid> {
    let Node::BlockSeq { offset: seq_offset, indent, items } = self.node(token) else {
      unreachable!()
    };
    let (seq_offset, indent) = (*seq_offset, *indent);
    self.at_root = false;
    let mut offset = seq_offset;
    let mut comment_end: Option<u32> = None;
    let mut nodes = Vec::new();
    for Item { start, value, .. } in items {
      let next = value.map_or(Next::None, Next::Node);
      let props =
        self.resolve_props(start, Indicator::SeqItem, None, next, offset, indent, true)?;
      if props.found.is_none() {
        if props.anchor.is_some() || props.tag.is_some() || value.is_some() {
          let at = match value.map(|value| self.node(value)) {
            Some(Node::BlockSeq { .. }) => props.end,
            _ => offset,
          };
          return Err(self.error(Message::BadIndent, at));
        }
        comment_end = Some(props.end);
        continue;
      }
      let node = match value {
        Some(value) => self.compose_node(*value, &props)?,
        None => self.compose_empty(props.end, Some(start), None, &props)?,
      };
      offset = self.out.node(node).range[2];
      nodes.push(node);
    }
    let node = Composed {
      content: Content::Seq { flow: false, items: nodes },
      range: [seq_offset, offset, comment_end.unwrap_or(offset)],
      tag: None,
      anchor: None,
      src: None,
    };
    Ok(self.push(node))
  }

  /// `resolveFlowCollection`
  fn flow_collection(&mut self, token: cst::Id) -> Result<Cid> {
    let Node::FlowCollection { offset: fc_offset, indent, start: open, items, end } =
      self.node(token)
    else {
      unreachable!()
    };
    let (fc_offset, indent) = (*fc_offset, *indent);
    let map = open.kind == Kind::FlowMapStart;
    let at_root = std::mem::replace(&mut self.at_root, false);
    let mut offset = fc_offset + (open.end - open.offset);
    let mut pairs: Vec<Pair> = Vec::new();
    let mut nodes: Vec<Cid> = Vec::new();
    for (index, item) in items.iter().enumerate() {
      let Item { start, key, sep, value, .. } = item;
      let key_token = match key {
        Key::Token(key) => Some(*key),
        _ => None,
      };
      let next = match (key_token, sep.as_ref().and_then(|sep| sep.first())) {
        (Some(key), _) => Next::Node(key),
        (_, Some(first)) => Next::Source(*first),
        _ => Next::None,
      };
      let props = self.resolve_props(
        start,
        Indicator::ExplicitKey,
        Some(map),
        next,
        offset,
        indent,
        false,
      )?;
      if props.found.is_none() {
        if props.anchor.is_none() && props.tag.is_none() && sep.is_none() && value.is_none() {
          if let Some(comma) = props.comma.filter(|_| index == 0) {
            return Err(self.error(Message::UnexpectedToken, comma.offset));
          }
          if index + 1 < items.len() {
            return Err(self.error(Message::UnexpectedToken, props.start));
          }
          offset = props.end;
          continue;
        }
        if !map && self.contains_newline(key_token) {
          return Err(self.error(
            Message::MultilineImplicitKey,
            key_token.map_or(0, |key| self.node(key).offset()),
          ));
        }
      }
      if index == 0 {
        if let Some(comma) = props.comma {
          return Err(self.error(Message::UnexpectedToken, comma.offset));
        }
      } else if props.comma.is_none() {
        return Err(self.error(Message::ExpectedComma, props.start));
      }
      if !map && sep.is_none() && props.found.is_none() {
        let node = match value {
          Some(value) => self.compose_node(*value, &props)?,
          None => self.compose_empty(props.end, sep.as_deref(), None, &props)?,
        };
        if value.is_some_and(|value| is_block(self.node(value))) {
          return Err(self.error(Message::BlockInFlow, self.out.node(node).range[0]));
        }
        offset = self.out.node(node).range[2];
        nodes.push(node);
        continue;
      }
      let key_start = props.end;
      let key_node = match key_token {
        Some(key) => self.compose_node(key, &props)?,
        None => self.compose_empty(key_start, Some(start), None, &props)?,
      };
      if key_token.is_some_and(|key| is_block(self.node(key))) {
        return Err(self.error(Message::BlockInFlow, self.out.node(key_node).range[0]));
      }
      let key_end = self.out.node(key_node).range[2];
      let sep_tokens: &[Source] = sep.as_deref().unwrap_or_default();
      let next = value.map_or(Next::None, Next::Node);
      let value_props = self.resolve_props(
        sep_tokens,
        Indicator::MapValue,
        Some(map),
        next,
        key_end,
        indent,
        false,
      )?;
      if let Some(found) = value_props.found {
        if !map && props.found.is_none() {
          for st in sep_tokens {
            if *st == found {
              break;
            }
            if st.kind == Kind::Newline {
              return Err(self.error(Message::MultilineImplicitKey, st.offset));
            }
          }
          if props.start + 1024 < found.offset {
            return Err(self.error(Message::KeyTooLong, found.offset));
          }
        }
      } else if value.is_some() {
        return Err(self.error(Message::ExpectedComma, value_props.start));
      }
      let value_node = match value {
        Some(value) => Some(self.compose_node(*value, &value_props)?),
        None if value_props.found.is_some() => {
          Some(self.compose_empty(value_props.end, Some(sep_tokens), None, &value_props)?)
        }
        None => None,
      };
      if let Some(node) = value_node
        && value.is_some_and(|value| is_block(self.node(value)))
      {
        return Err(self.error(Message::BlockInFlow, self.out.node(node).range[0]));
      }
      let pair = Pair { key: key_node, value: value_node, src: Some((token, index)) };
      offset = match value_node {
        Some(node) => self.out.node(node).range[2],
        None => value_props.end,
      };
      if map {
        pairs.push(pair);
      } else {
        let last = self.out.node(value_node.unwrap_or(key_node)).range;
        let first = self.out.node(key_node).range[0];
        let node = Composed {
          content: Content::Map { flow: true, items: vec![pair] },
          range: [first, last[1], last[2]],
          tag: None,
          anchor: None,
          src: None,
        };
        nodes.push(self.push(node));
      }
    }
    let expected = if map { Kind::FlowMapEnd } else { Kind::FlowSeqEnd };
    let mut ends = end.iter();
    let close_end = match ends.next() {
      Some(close) if close.kind == expected => close.end,
      _ => {
        let message = if at_root { Message::UnclosedBracket } else { Message::BadIndent };
        return Err(self.error(message, offset));
      }
    };
    let rest: Vec<Source> = ends.copied().collect();
    let range = match rest.is_empty() {
      true => [fc_offset, close_end, close_end],
      false => [fc_offset, close_end, self.resolve_end(Some(&rest), close_end, true)?],
    };
    let content = match map {
      true => Content::Map { flow: true, items: pairs },
      false => Content::Seq { flow: true, items: nodes },
    };
    let node = Composed { content, range, tag: None, anchor: None, src: None };
    Ok(self.push(node))
  }

  /// `containsNewline`
  fn contains_newline(&self, key: Option<cst::Id>) -> bool {
    let Some(key) = key else { return false };
    match self.node(key) {
      Node::FlowScalar { offset, source_end, end, .. } => {
        self.text[*offset as usize..*source_end as usize].contains('\n')
          || end.iter().flatten().any(|st| st.kind == Kind::Newline)
      }
      Node::FlowCollection { items, .. } => items.iter().any(|it| {
        it.start.iter().any(|st| st.kind == Kind::Newline)
          || it.sep.iter().flatten().any(|st| st.kind == Kind::Newline)
          || self.contains_newline(match it.key {
            Key::Token(key) => Some(key),
            _ => None,
          })
          || self.contains_newline(it.value)
      }),
      _ => true,
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NextKind {
  Space,
  Newline,
  Comma,
  EmptyScalar,
  FlowCollection,
  Block,
  Other,
}

fn is_block(node: &Node) -> bool {
  matches!(node, Node::BlockMap { .. } | Node::BlockSeq { .. })
}

/// `emptyScalarPosition`: right after the last token that isn't a space, a comment or a line break, and the spaces
/// after it.
fn empty_scalar_position(mut offset: u32, before: Option<&[Source]>, pos: Option<usize>) -> u32 {
  let Some(before) = before else { return offset };
  let pos = pos.unwrap_or(before.len());
  let mut i = pos;
  while i > 0 {
    i -= 1;
    let st = before[i];
    if matches!(st.kind, Kind::Space | Kind::Comment | Kind::Newline) {
      offset -= st.end - st.offset;
      continue;
    }
    let mut j = i + 1;
    while let Some(st) = before.get(j).filter(|st| st.kind == Kind::Space) {
      offset += st.end - st.offset;
      j += 1;
    }
    break;
  }
  offset
}

impl Node {
  /// `'indent' in key ? key.indent`: the indent of a token that has one.
  fn indent_of(&self) -> Option<u32> {
    match *self {
      Node::FlowScalar { indent, .. }
      | Node::BlockScalar { indent, .. }
      | Node::BlockMap { indent, .. }
      | Node::BlockSeq { indent, .. }
      | Node::FlowCollection { indent, .. } => Some(indent),
      _ => None,
    }
  }
}
