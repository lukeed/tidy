//! Prettier's YAML printer (`src/language-yaml/printer-yaml.js` and `print/*`).

use super::ast::{Chomping, Id, Kind, Root};
use super::{Options, ProseWrap};
use crate::doc::{Align, Branch, Docs};
use crate::stack::Stack;
use rustc_hash::FxHashSet;

pub(crate) struct Printer<'p, 'a> {
  pub(crate) root: &'p Root,
  pub(crate) text: &'a str,
  pub(crate) options: &'p Options,
  pub(crate) docs: &'p mut Docs<'a>,
  pub(crate) stack: &'p Stack,
  /// `printedEmptyLineCache`: the ends of the nodes after which an empty line was printed.
  pub(crate) printed: FxHashSet<u32>,
  /// The nodes around the one that prints, outermost first.
  pub(crate) ancestors: Vec<Id>,
}

/// How a node is reached: whether every list on the way to it was at its last item, `isLastDescendantNode`.
#[derive(Clone, Copy)]
struct Step {
  last: bool,
}

impl<'a> Printer<'_, 'a> {
  pub(crate) fn document(&mut self) {
    let root = self.root.root;
    self.print(root, Step { last: true });
  }

  fn node(&self, id: Id) -> &super::ast::Node {
    self.root.node(id)
  }

  fn parent(&self) -> Option<Id> {
    self.ancestors.last().copied()
  }

  fn slice(&self, start: u32, end: u32) -> &'a str {
    &self.text[start as usize..end as usize]
  }

  /// Prints a child: `print(name)` or `path.map(print, name)`.
  fn child(&mut self, parent: Id, child: Id, step: Step) {
    self.ancestors.push(parent);
    self.print(child, step);
    self.ancestors.pop();
  }

  /// The steps to the items of a list: each is last if it is the list's last, and the list is reached by last steps.
  fn list(&self, items: &[Id], step: Step) -> Vec<Step> {
    let count = items.len();
    (0..count).map(|index| Step { last: step.last && index + 1 == count }).collect()
  }

  /// `genericPrint`
  fn print(&mut self, id: Id, step: Step) {
    if !self.stack.has_room() {
      return;
    }
    let node = self.node(id).clone();
    let kind = node.kind;
    if kind != Kind::MappingValue && !node.leading_comments.is_empty() {
      self.join_comments(id, &node.leading_comments);
      self.docs.hardline();
    }
    if let Some(tag) = node.tag {
      self.child(id, tag, Step { last: false });
    }
    if node.tag.is_some() && node.anchor.is_some() {
      self.docs.text(" ");
    }
    if let Some(anchor) = node.anchor {
      self.child(id, anchor, Step { last: false });
    }
    let next_empty_line = matches!(
      kind,
      Kind::Mapping
        | Kind::Sequence
        | Kind::Comment
        | Kind::Directive
        | Kind::MappingItem
        | Kind::SequenceItem
    ) && !self.is_last_descendant(id, step)
      && self.next_empty_line(id);
    if node.tag.is_some() || node.anchor.is_some() {
      match matches!(kind, Kind::Sequence | Kind::Mapping) && node.middle_comments.is_empty() {
        true => self.docs.hardline(),
        false => self.docs.text(" "),
      }
    }
    if !node.middle_comments.is_empty() {
      if node.middle_comments.len() != 1 {
        self.docs.hardline();
      }
      self.join_comments(id, &node.middle_comments);
      self.docs.hardline();
    }
    if self.is_ignored(id) {
      let raw = self.slice(node.position.start.offset, node.position.end.offset);
      self.lines(raw.trim_end());
    } else {
      let group = self.docs.group();
      self.print_node(id, step);
      self.docs.close(group);
    }
    if let Some(comment) = node.trailing_comment
      && !matches!(kind, Kind::Document | Kind::DocumentHead)
    {
      let suffix = self.docs.line_suffix();
      if !(kind == Kind::MappingValue && node.children.is_empty()) {
        self.docs.text(" ");
      }
      // `path.getParentNode(2)`: the parent of the parent of the parent.
      let inline_key =
        self.parent().is_some_and(|parent| self.node(parent).kind == Kind::MappingKey)
          && self.ancestors.len() >= 3
          && self.node(self.ancestors[self.ancestors.len() - 3]).kind == Kind::Mapping
          && is_inline(Some(kind));
      if !inline_key {
        self.docs.break_parent();
      }
      self.child(id, comment, Step { last: false });
      self.docs.close(suffix);
    }
    if should_print_end_comments(&node) {
      // `alignWithSpaces(0, ...)` leaves the indent as it is.
      let align = (kind == Kind::SequenceItem).then(|| self.docs.align(Align::Text("  ")));
      self.docs.hardline();
      for (index, comment) in node.end_comments.iter().enumerate() {
        if index > 0 {
          self.docs.hardline();
        }
        let start = self.node(*comment).position.start.offset;
        if crate::text::is_previous_line_empty(self.text.as_bytes(), start as usize) {
          self.docs.hardline();
        }
        self.child(id, *comment, Step { last: false });
      }
      if let Some(align) = align {
        self.docs.close(align);
      }
    }
    if next_empty_line {
      self.docs.softline();
    }
  }

  fn join_comments(&mut self, parent: Id, comments: &[Id]) {
    for (index, comment) in comments.iter().enumerate() {
      if index > 0 {
        self.docs.hardline();
      }
      self.child(parent, *comment, Step { last: false });
    }
  }

  /// `isLastDescendantNode`
  fn is_last_descendant(&self, id: Id, step: Step) -> bool {
    !matches!(self.node(id).kind, Kind::Tag | Kind::Anchor | Kind::Comment) && step.last
  }

  /// `printNextEmptyLine`: whether to print an empty line after the node, once for each end.
  fn next_empty_line(&mut self, id: Id) -> bool {
    let end = self.node(id).position.end.offset;
    if !self.printed.insert(end) {
      return false;
    }
    let parent = self.parent().map(|parent| self.node(parent));
    is_next_line_empty(self.text, end) && !parent.is_some_and(should_print_end_comments)
  }

  /// `hasPrettierIgnore`
  fn is_ignored(&self, id: Id) -> bool {
    let node = self.node(id);
    let last = match node.kind {
      Kind::DocumentBody => {
        let document = self.node(self.parent().expect("a body is in a document"));
        self.node(document.children[0]).end_comments.last().copied()
      }
      _ => node.leading_comments.last().copied(),
    };
    last.is_some_and(|comment| self.node(comment).value.trim() == self.options.ignore)
  }

  /// `replaceEndOfLine`
  fn lines(&mut self, text: &'a str) {
    for (index, line) in text.split('\n').enumerate() {
      if index > 0 {
        self.docs.literalline();
      }
      self.docs.text(line);
    }
  }

  /// `printNode`
  fn print_node(&mut self, id: Id, step: Step) {
    let node = self.node(id).clone();
    match node.kind {
      Kind::Root => self.print_root(id),
      Kind::Document => self.print_document(id, step),
      Kind::DocumentHead => {
        let steps = self.list(&node.children, step);
        let items = node
          .children
          .iter()
          .zip(steps)
          .chain(node.end_comments.iter().map(|it| (it, Step { last: false })));
        for (index, (item, step)) in items.enumerate() {
          if index > 0 {
            self.docs.hardline();
          }
          self.child(id, *item, step);
        }
      }
      Kind::DocumentBody => self.print_body(id, step),
      Kind::Directive => {
        self.docs.text("%");
        let value = self.docs.keep(&node.value);
        self.docs.text(value);
        for parameter in &node.parameters {
          self.docs.text(" ");
          let parameter = self.docs.keep(parameter);
          self.docs.text(parameter);
        }
      }
      Kind::Comment => {
        self.docs.text("#");
        let value = self.docs.keep(&node.value);
        self.docs.text(value);
      }
      Kind::Alias | Kind::Anchor => {
        self.docs.text(if node.kind == Kind::Alias { "*" } else { "&" });
        let value = self.docs.keep(&node.value);
        self.docs.text(value);
      }
      Kind::Tag => {
        let text = self.slice(node.position.start.offset, node.position.end.offset);
        self.docs.text(text);
      }
      Kind::Plain => {
        let raw = self.slice(node.position.start.offset, node.position.end.offset);
        self.flow_scalar_content(Kind::Plain, raw.into());
      }
      Kind::QuoteDouble | Kind::QuoteSingle => self.print_quote(id),
      Kind::BlockFolded | Kind::BlockLiteral => self.print_block(id, step),
      Kind::Mapping | Kind::Sequence => {
        let steps = self.list(&node.children, step);
        for (index, child) in node.children.iter().enumerate() {
          if index > 0 {
            self.docs.hardline();
          }
          self.child(id, *child, steps[index]);
        }
      }
      Kind::SequenceItem => {
        self.docs.text("- ");
        let align = self.docs.align(Align::Text("  "));
        if let Some(content) = node.children.first() {
          self.child(id, *content, step);
        }
        self.docs.close(align);
      }
      Kind::MappingKey | Kind::MappingValue | Kind::FlowSequenceItem => {
        if let Some(content) = node.children.first() {
          self.child(id, *content, step);
        }
      }
      Kind::MappingItem | Kind::FlowMappingItem => self.print_mapping_item(id, step),
      Kind::FlowMapping | Kind::FlowSequence => self.print_flow_mapping(id, step),
    }
  }

  fn print_root(&mut self, id: Id) {
    let node = self.node(id).clone();
    let last = last_descendant(self.root, id);
    let keep = matches!(self.node(last).kind, Kind::BlockLiteral | Kind::BlockFolded)
      && self.node(last).chomping == Chomping::Keep;
    let count = node.children.len();
    for (index, document) in node.children.iter().enumerate() {
      if index > 0 {
        self.docs.hardline();
      }
      let step = Step { last: index + 1 == count };
      self.child(id, *document, step);
      if self.should_print_document_end_marker(*document, index, &node.children) {
        if !keep {
          self.docs.hardline();
        }
        self.docs.text("...");
        if let Some(comment) = self.node(*document).trailing_comment {
          self.docs.text(" ");
          self.ancestors.push(id);
          self.child(*document, comment, Step { last: false });
          self.ancestors.pop();
        }
      }
    }
    if !keep {
      self.docs.hardline();
    }
  }

  /// `shouldPrintDocumentEndMarker`
  fn should_print_document_end_marker(&self, document: Id, index: usize, documents: &[Id]) -> bool {
    let node = self.node(document);
    if node.document_end_marker || node.trailing_comment.is_some() {
      return true;
    }
    let Some(next) = documents.get(index + 1) else { return false };
    let head = self.node(self.node(*next).children[0]);
    !head.children.is_empty() || !head.end_comments.is_empty()
  }

  fn print_document(&mut self, id: Id, step: Step) {
    let node = self.node(id).clone();
    let (head_id, body_id) = (node.children[0], node.children[1]);
    let head = self.node(head_id).clone();
    let body = self.node(body_id);
    let marker = node.directives_end_marker
      || !head.children.is_empty()
      || !head.end_comments.is_empty()
      || head.trailing_comment.is_some();
    let print_body = !body.children.is_empty() || !body.end_comments.is_empty();
    let mut first = true;
    if marker {
      if !head.children.is_empty() || !head.end_comments.is_empty() {
        self.child(id, head_id, step);
        first = false;
      }
      if !first {
        self.docs.hardline();
      }
      first = false;
      match head.trailing_comment {
        Some(comment) => {
          self.docs.text("--- ");
          self.ancestors.push(id);
          self.child(head_id, comment, Step { last: false });
          self.ancestors.pop();
        }
        None => self.docs.text("---"),
      }
    }
    if print_body {
      if !first {
        self.docs.hardline();
      }
      self.child(id, body_id, step);
    }
  }

  fn print_body(&mut self, id: Id, step: Step) {
    let node = self.node(id).clone();
    let steps = self.list(&node.children, step);
    for (index, child) in node.children.iter().enumerate() {
      if index > 0 {
        self.docs.hardline();
      }
      self.child(id, *child, steps[index]);
    }
    if !node.children.is_empty() && !node.end_comments.is_empty() {
      let last = last_descendant(self.root, id);
      if matches!(self.node(last).kind, Kind::BlockFolded | Kind::BlockLiteral) {
        if self.node(last).chomping != Chomping::Keep {
          self.docs.hardline();
          self.docs.hardline();
        }
      } else {
        let last_child = self.node(*node.children.last().expect("a child"));
        let first_comment = self.node(node.end_comments[0]).position.start.offset as usize;
        let empty = last_child.kind == Kind::Mapping
          && crate::text::is_previous_line_empty(self.text.as_bytes(), first_comment);
        self.docs.hardline();
        if empty {
          self.docs.hardline();
        }
      }
    }
    for (index, comment) in node.end_comments.iter().enumerate() {
      if index > 0 {
        self.docs.hardline();
      }
      self.child(id, *comment, Step { last: false });
    }
  }

  fn print_quote(&mut self, id: Id) {
    let node = self.node(id);
    let kind = node.kind;
    let raw = self.slice(node.position.start.offset + 1, node.position.end.offset - 1);
    let has_escape = match kind {
      Kind::QuoteSingle => raw.contains('\\'),
      _ => raw.as_bytes().windows(2).any(|pair| pair[0] == b'\\' && pair[1] != b'"'),
    };
    let (quote, content): (&str, String) = if has_escape {
      (if kind == Kind::QuoteDouble { "\"" } else { "'" }, raw.into())
    } else if raw.contains('"') {
      let content = match kind {
        Kind::QuoteDouble => raw.replace("\\\"", "\"").replace('\'', "''"),
        _ => raw.into(),
      };
      ("'", content)
    } else if raw.contains('\'') {
      let content = match kind {
        Kind::QuoteSingle => raw.replace("''", "'"),
        _ => raw.into(),
      };
      ("\"", content)
    } else {
      (if self.options.single_quote { "'" } else { "\"" }, raw.into())
    };
    self.docs.text(quote);
    self.flow_scalar_content(kind, content);
    self.docs.text(quote);
  }

  /// `printFlowScalarContent`
  fn flow_scalar_content(&mut self, kind: Kind, content: String) {
    let lines = flow_scalar_line_contents(kind, &content, self.options.prose_wrap);
    for (index, words) in lines.iter().enumerate() {
      if index > 0 {
        self.docs.hardline();
      }
      self.fill(words);
    }
  }

  /// `fill(join(line, words))`
  fn fill(&mut self, words: &[String]) {
    let fill = self.docs.fill();
    for (index, word) in words.iter().enumerate() {
      if index > 0 {
        let part = self.docs.part();
        self.docs.line();
        self.docs.close(part);
      }
      let part = self.docs.part();
      self.docs.owned(word);
      self.docs.close(part);
    }
    self.docs.close(fill);
  }

  /// `printBlock`
  fn print_block(&mut self, id: Id, step: Step) {
    let node = self.node(id).clone();
    let parent_indent = self
      .ancestors
      .iter()
      .filter(|it| matches!(self.node(**it).kind, Kind::Sequence | Kind::Mapping))
      .count();
    let last = self.is_last_descendant(id, step);
    self.docs.text(if node.kind == Kind::BlockFolded { ">" } else { "|" });
    if let Some(indent) = node.indent {
      self.docs.owned(&indent.to_string());
    }
    match node.chomping {
      Chomping::Clip => {}
      Chomping::Keep => self.docs.text("+"),
      Chomping::Strip => self.docs.text("-"),
    }
    if let Some(comment) = node.indicator_comment {
      self.docs.text(" ");
      self.child(id, comment, Step { last: false });
    }
    let lines = self.block_value_line_contents(&node, parent_indent, last);
    let outer = match node.indent {
      None => {
        let dedent = self.docs.align(Align::Dedent);
        let spaces = SPACES[..self.options.tab_width.min(SPACES.len())].as_ref();
        (dedent, self.docs.align(Align::Text(spaces)))
      }
      Some(indent) => {
        let root = self.docs.align(Align::Root);
        let width = (indent as usize - 1 + parent_indent).min(SPACES.len());
        (root, self.docs.align(Align::Text(SPACES[..width].as_ref())))
      }
    };
    for (index, words) in lines.iter().enumerate() {
      if index == 0 {
        self.docs.hardline();
      }
      self.fill(words);
      if index + 1 != lines.len() {
        if words.is_empty() {
          self.docs.hardline();
        } else {
          let root = self.docs.align(Align::Mark);
          self.docs.literalline();
          self.docs.close(root);
        }
      } else if node.chomping == Chomping::Keep && last {
        let root = self.docs.align(Align::Root);
        match words.is_empty() {
          true => self.docs.hardline(),
          false => self.docs.literalline(),
        }
        self.docs.close(root);
      }
    }
    self.docs.close(outer.1);
    self.docs.close(outer.0);
  }

  /// `getBlockValueLineContents`
  fn block_value_line_contents(
    &self,
    node: &super::ast::Node,
    parent_indent: usize,
    last: bool,
  ) -> Vec<Vec<String>> {
    let content = match node.position.start.line == node.position.end.line {
      true => "",
      false => {
        let text = self.slice(node.position.start.offset, node.position.end.offset);
        text.split_once('\n').map_or("", |(_, rest)| rest)
      }
    };
    if content.is_empty() {
      return Vec::new();
    }
    let leading = match node.indent {
      None => content
        .split('\n')
        .find_map(|line| {
          let spaces = line.len() - line.trim_start_matches(' ').len();
          line[spaces..].starts_with(|it: char| !matches!(it, '\n' | '\r' | ' ')).then_some(spaces)
        })
        .unwrap_or(usize::MAX),
      Some(indent) => indent as usize - 1 + parent_indent,
    };
    let raw: Vec<&str> = content.split('\n').map(|line| slice_from(line, leading)).collect();
    let mut lines: Vec<Vec<String>>;
    if self.options.prose_wrap == ProseWrap::Preserve || node.kind == Kind::BlockLiteral {
      lines = raw
        .iter()
        .map(|line| if line.is_empty() { Vec::new() } else { vec![(*line).to_owned()] })
        .collect();
    } else {
      lines = Vec::new();
      for (index, line) in raw.iter().enumerate() {
        let words = split_with_single_space(line);
        let joins = index > 0
          && !words.is_empty()
          && !raw[index - 1].is_empty()
          && !words[0].starts_with(char::is_whitespace)
          && !lines.last().is_some_and(|last: &Vec<String>| {
            let joined = last.join(",");
            joined.starts_with(char::is_whitespace) || joined.ends_with(char::is_whitespace)
          });
        match (joins, lines.last_mut()) {
          (true, Some(last)) => last.extend(words),
          _ => lines.push(words),
        }
      }
      lines = lines
        .into_iter()
        .map(|original| {
          let mut words: Vec<String> = Vec::new();
          for word in original {
            match words.last_mut() {
              Some(last) if last.ends_with(char::is_whitespace) => {
                last.push(' ');
                last.push_str(&word);
              }
              _ => words.push(word),
            }
          }
          words
        })
        .collect();
      if self.options.prose_wrap == ProseWrap::Never {
        lines = lines.into_iter().map(|words| vec![words.join(" ")]).collect();
      }
    }
    if node.chomping == Chomping::Keep {
      if content.ends_with('\n') && lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
      }
      return lines;
    }
    let trailing = lines
      .iter()
      .rev()
      .take_while(|words| words.iter().all(|word| word.trim_end_matches([' ', '\t']).is_empty()))
      .count();
    if trailing == 0 {
      return lines;
    }
    let keep = if trailing >= 2 && !last { trailing - 1 } else { trailing };
    lines.truncate(lines.len() - keep);
    lines
  }

  /// `printMappingItem`
  fn print_mapping_item(&mut self, id: Id, step: Step) {
    let node = self.node(id).clone();
    let (key, value) = (node.children[0], node.children[1]);
    let key_node = self.node(key).clone();
    let value_node = self.node(value).clone();
    let key_content = key_node.children.first().copied();
    let value_content = value_node.children.first().copied();
    let empty_key = is_empty_node(&key_node);
    let empty_value = is_empty_node(&value_node);
    if empty_key && empty_value {
      return self.docs.text(": ");
    }
    let space =
      if key_content.is_some_and(|it| self.node(it).kind == Kind::Alias) { " " } else { "" };
    let parent = self.parent().map(|it| self.node(it).clone());
    if empty_value {
      if node.kind == Kind::FlowMappingItem
        && parent.as_ref().is_some_and(|it| it.kind == Kind::FlowMapping)
      {
        return self.child(id, key, step);
      }
      let set = parent
        .as_ref()
        .and_then(|it| it.tag)
        .is_some_and(|tag| self.node(tag).value == "tag:yaml.org,2002:set");
      if node.kind == Kind::MappingItem
        && self.absolutely_single_line(key_content)
        && !key_content.is_some_and(|it| self.node(it).trailing_comment.is_some())
        && !set
      {
        self.child(id, key, step);
        self.docs.text(space);
        return self.docs.text(":");
      }
      self.docs.text("? ");
      let align = self.docs.align(Align::Text("  "));
      self.child(id, key, step);
      return self.docs.close(align);
    }
    if empty_key {
      self.docs.text(": ");
      let align = self.docs.align(Align::Text("  "));
      self.child(id, value, step);
      return self.docs.close(align);
    }
    if !value_node.leading_comments.is_empty()
      || !is_inline(key_content.map(|it| self.node(it).kind))
    {
      self.docs.text("? ");
      let align = self.docs.align(Align::Text("  "));
      self.child(id, key, step);
      self.docs.close(align);
      self.docs.hardline();
      for comment in &value_node.leading_comments {
        self.ancestors.push(id);
        self.child(value, *comment, Step { last: false });
        self.ancestors.pop();
        self.docs.hardline();
      }
      self.docs.text(": ");
      let align = self.docs.align(Align::Text("  "));
      self.child(id, value, step);
      return self.docs.close(align);
    }
    let key_has = |printer: &Self| {
      key_content.is_some_and(|it| {
        let content = printer.node(it);
        !content.leading_comments.is_empty()
          || !content.middle_comments.is_empty()
          || content.trailing_comment.is_some()
      }) || !key_node.end_comments.is_empty()
    };
    let value_has = value_content.is_some_and(|it| {
      let content = self.node(it);
      !content.leading_comments.is_empty() || !content.middle_comments.is_empty()
    }) || !value_node.end_comments.is_empty();
    if self.single_line(key_content)
      && !key_has(self)
      && !value_has
      && self.absolutely_single_line(value_content)
      && self.absolutely_single_line(key_content)
    {
      self.child(id, key, step);
      self.docs.text(space);
      self.docs.text(": ");
      return self.child(id, value, step);
    }
    let value_kind = value_content.map(|it| self.node(it).kind);
    let breaks_before_value = (!value_node.end_comments.is_empty()
      && value_content.is_some()
      && matches!(value_kind, Some(Kind::FlowMapping | Kind::FlowSequence))
      && value_content.is_some_and(|it| self.node(it).children.is_empty()))
    .then_some(Separator::Space)
    .or_else(|| {
      let content = value_content.map(|it| self.node(it));
      let leading = content.is_some_and(|it| !it.leading_comments.is_empty());
      let end = !value_node.end_comments.is_empty()
        && value_content.is_some()
        && !matches!(value_kind, Some(Kind::Mapping | Kind::Sequence));
      let trailing_key = parent.as_ref().is_some_and(|it| it.kind == Kind::Mapping)
        && key_content.is_some_and(|it| self.node(it).trailing_comment.is_some())
        && is_inline(value_kind);
      let collection = matches!(value_kind, Some(Kind::Mapping | Kind::Sequence))
        && content.is_some_and(|it| it.tag.is_none() && it.anchor.is_none());
      (leading || end || trailing_key || collection).then_some(Separator::Hard)
    })
    .or_else(|| value_content.is_some().then_some(Separator::Line))
    .or_else(|| value_node.trailing_comment.is_some().then_some(Separator::Space));
    let tab = SPACES[..self.options.tab_width.min(SPACES.len())].as_ref();
    let implicit = |printer: &mut Self| {
      let align = printer.docs.align(Align::Text(tab));
      printer.docs.text(space);
      printer.docs.text(":");
      match breaks_before_value {
        Some(Separator::Space) => printer.docs.text(" "),
        Some(Separator::Hard) => printer.docs.hardline(),
        Some(Separator::Line) => printer.docs.line(),
        None => {}
      }
      printer.child(id, value, step);
      printer.docs.close(align);
    };
    if self.absolutely_single_line(key_content) && !key_has(self) {
      let conditional = self.docs.conditional();
      let part = self.docs.part();
      self.child(id, key, step);
      implicit(self);
      self.docs.close(part);
      return self.docs.close(conditional);
    }
    let conditional = self.docs.conditional();
    let part = self.docs.part();
    let group_id = self.docs.group_id();
    let grouped = self.docs.group();
    let question = self.docs.if_break(Branch::Break, None);
    self.docs.text("? ");
    self.docs.close(question);
    let key_group = self.docs.group_with(Some(group_id), false);
    let align = self.docs.align(Align::Text("  "));
    self.child(id, key, step);
    self.docs.close(align);
    self.docs.close(key_group);
    self.docs.close(grouped);
    let explicit = self.docs.if_break(Branch::Break, Some(group_id));
    self.docs.hardline();
    self.docs.text(": ");
    let align = self.docs.align(Align::Text("  "));
    self.child(id, value, step);
    self.docs.close(align);
    self.docs.close(explicit);
    let flat = self.docs.if_break(Branch::Flat, Some(group_id));
    implicit(self);
    self.docs.close(flat);
    self.docs.close(part);
    self.docs.close(conditional);
  }

  /// `isAbsolutelyPrintedAsSingleLineNode`
  fn absolutely_single_line(&self, id: Option<Id>) -> bool {
    let Some(id) = id else { return true };
    let node = self.node(id);
    match node.kind {
      Kind::Plain | Kind::QuoteSingle | Kind::QuoteDouble => {}
      Kind::Alias => return true,
      _ => return false,
    }
    if self.options.prose_wrap == ProseWrap::Preserve {
      return node.position.start.line == node.position.end.line;
    }
    let raw = self.slice(node.position.start.offset, node.position.end.offset);
    if raw.split('\n').any(|line| line.ends_with('\\')) {
      return false;
    }
    match self.options.prose_wrap {
      ProseWrap::Never => !node.value.contains('\n'),
      _ => !node.value.contains(['\n', ' ']),
    }
  }

  /// `isSingleLineNode`
  fn single_line(&self, id: Option<Id>) -> bool {
    let Some(id) = id else { return true };
    let node = self.node(id);
    match node.kind {
      Kind::Plain | Kind::QuoteDouble | Kind::QuoteSingle => {
        node.position.start.line == node.position.end.line
      }
      Kind::Alias => true,
      _ => false,
    }
  }

  /// `printFlowMapping`
  fn print_flow_mapping(&mut self, id: Id, step: Step) {
    let node = self.node(id).clone();
    let mapping = node.kind == Kind::FlowMapping;
    let (open, close) = if mapping { ("{", "}") } else { ("[", "]") };
    let spaced = mapping && !node.children.is_empty() && self.options.bracket_spacing;
    let last_empty = node.children.last().is_some_and(|last| {
      let item = self.node(*last);
      item.kind == Kind::FlowMappingItem
        && is_empty_node(self.node(item.children[0]))
        && is_empty_node(self.node(item.children[1]))
    });
    self.docs.text(open);
    let tab = SPACES[..self.options.tab_width.min(SPACES.len())].as_ref();
    let align = self.docs.align(Align::Text(tab));
    self.spacing(spaced);
    let steps = self.list(&node.children, step);
    for (index, child) in node.children.iter().enumerate() {
      self.child(id, *child, steps[index]);
      if let Some(next) = node.children.get(index + 1) {
        self.docs.text(",");
        self.docs.line();
        let line = self.node(*child).position.start.line;
        if line != self.node(*next).position.start.line {
          self.ancestors.push(id);
          let empty = self.next_empty_line(*child);
          self.ancestors.pop();
          if empty {
            self.docs.softline();
          }
        }
      }
    }
    if self.options.trailing_comma {
      let comma = self.docs.if_break(Branch::Break, None);
      self.docs.text(",");
      self.docs.close(comma);
    }
    if !node.end_comments.is_empty() {
      self.docs.hardline();
      self.join_comments(id, &node.end_comments);
    }
    self.docs.close(align);
    if !last_empty {
      self.spacing(spaced);
    }
    self.docs.text(close);
  }

  fn spacing(&mut self, spaced: bool) {
    match spaced {
      true => self.docs.line(),
      false => self.docs.softline(),
    }
  }
}

#[derive(Clone, Copy)]
enum Separator {
  Space,
  Hard,
  Line,
}

const SPACES: &str = "                                                                                                                                ";

/// `isNode(node, ["plain", ...])` of `isInlineNode`: true for no node.
fn is_inline(kind: Option<Kind>) -> bool {
  match kind {
    None => true,
    Some(kind) => matches!(
      kind,
      Kind::Plain
        | Kind::QuoteDouble
        | Kind::QuoteSingle
        | Kind::Alias
        | Kind::FlowMapping
        | Kind::FlowSequence
    ),
  }
}

/// `isEmptyNode`: no children, and no comments.
fn is_empty_node(node: &super::ast::Node) -> bool {
  node.children.is_empty()
    && node.leading_comments.is_empty()
    && node.middle_comments.is_empty()
    && node.indicator_comment.is_none()
    && node.trailing_comment.is_none()
    && node.end_comments.is_empty()
}

/// `shouldPrintEndComments`
fn should_print_end_comments(node: &super::ast::Node) -> bool {
  !node.end_comments.is_empty()
    && !matches!(
      node.kind,
      Kind::DocumentHead | Kind::DocumentBody | Kind::FlowMapping | Kind::FlowSequence
    )
}

/// `getLastDescendantNode`
fn last_descendant(root: &Root, id: Id) -> Id {
  match root.node(id).children.last() {
    Some(last) => last_descendant(root, *last),
    None => id,
  }
}

/// `isNextLineEmpty` of YAML: a line with only white space follows the line of `end`, from its last character on.
fn is_next_line_empty(text: &str, end: u32) -> bool {
  let mut start = (end as usize).saturating_sub(1).min(text.len());
  while !text.is_char_boundary(start) {
    start -= 1;
  }
  let mut newlines = 0;
  for character in text[start..].chars() {
    if character == '\n' {
      newlines += 1;
    }
    if newlines == 1 && !is_js_whitespace(character) {
      return false;
    }
    if newlines == 2 {
      return true;
    }
  }
  false
}

/// `line.slice(count)`, by characters.
fn slice_from(line: &str, count: usize) -> &str {
  match line.char_indices().nth(count) {
    Some((at, _)) => &line[at..],
    None => "",
  }
}

/// `splitWithSingleSpace`: splits at single spaces, keeping runs of spaces in the words.
fn split_with_single_space(text: &str) -> Vec<String> {
  if text.is_empty() {
    return Vec::new();
  }
  let bytes = text.as_bytes();
  let mut words = Vec::new();
  let mut start = 0;
  for (at, byte) in bytes.iter().enumerate() {
    let single = *byte == b' '
      && at > 0
      && bytes[at - 1] != b' '
      && at + 1 < bytes.len()
      && bytes[at + 1] != b' ';
    if single {
      words.push(text[start..at].to_owned());
      start = at + 1;
    }
  }
  words.push(text[start..].to_owned());
  words
}

/// `getFlowScalarLineContents`
fn flow_scalar_line_contents(kind: Kind, content: &str, wrap: ProseWrap) -> Vec<Vec<String>> {
  let parts: Vec<&str> = content.split('\n').collect();
  let count = parts.len();
  let raw: Vec<&str> = parts
    .iter()
    .enumerate()
    .map(|(index, line)| match (index == 0, index + 1 == count) {
      (true, true) => *line,
      (false, false) => line.trim_matches(is_js_whitespace),
      (true, false) => line.trim_end_matches(is_js_whitespace),
      (false, true) => line.trim_start_matches(is_js_whitespace),
    })
    .collect();
  if wrap == ProseWrap::Preserve {
    return raw
      .iter()
      .map(|line| if line.is_empty() { Vec::new() } else { vec![(*line).to_owned()] })
      .collect();
  }
  let mut lines: Vec<Vec<String>> = Vec::new();
  for (index, line) in raw.iter().enumerate() {
    let words = split_with_single_space(line);
    let joins = index > 0
      && !raw[index - 1].is_empty()
      && !words.is_empty()
      && !(kind == Kind::QuoteDouble
        && lines.last().and_then(|it| it.last()).is_some_and(|it| it.ends_with('\\')));
    match (joins, lines.last_mut()) {
      (true, Some(last)) => last.extend(words),
      _ => lines.push(words),
    }
  }
  if wrap == ProseWrap::Never {
    return lines.into_iter().map(|words| vec![words.join(" ")]).collect();
  }
  lines
}

/// JavaScript's `trim` white space.
fn is_js_whitespace(character: char) -> bool {
  character.is_whitespace() || character == '\u{feff}'
}
