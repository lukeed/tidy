//! Prettier's `src/language-graphql/printer-graphql.js`, and what `src/main/comments/print.js` does with the comments.
//! Ported from bun's `src/format/graphql/printer.rs` (MIT), which corrects two places where Prettier changes the value
//! of a block string.

use super::Options;
use super::comments::{Attached, Placement};
use super::parse::{
  HAS_ALIAS, IS_BLOCK, IS_REPEATABLE, IS_SHORTHAND, Kind, MUTATION, NodeId, SUBSCRIPTION, Span,
  Tree, read_escape,
};
use crate::doc::{Align, Branch, Docs};
use crate::stack::Stack;
use crate::strings::{trim_js_whitespace, trim_js_whitespace_end};
use crate::text::{
  has_newline, has_newline_backwards, is_next_line_empty, is_previous_line_empty, skip_newline,
  skip_spaces,
};

pub(crate) struct Printer<'t, 'a> {
  pub(crate) text: &'a str,
  pub(crate) tree: &'t Tree,
  pub(crate) attached: &'t [Attached],
  pub(crate) docs: &'t mut Docs<'a>,
  pub(crate) stack: &'t Stack,
  pub(crate) options: &'t Options,
}

/// The children of a node that haven't been printed yet.
struct Children<'t> {
  tree: &'t Tree,
  rest: &'t [NodeId],
}

impl<'t> Children<'t> {
  /// Takes the next child if `wanted` accepts its kind.
  fn next_if(&mut self, wanted: impl Fn(Kind) -> bool) -> Option<NodeId> {
    let (&first, rest) = self.rest.split_first()?;
    if !wanted(self.tree.kind(first)) {
      return None;
    }
    self.rest = rest;
    Some(first)
  }

  fn next_of(&mut self, kind: Kind) -> Option<NodeId> {
    self.next_if(|it| it == kind)
  }

  /// Takes the run of children of `kind` that comes next.
  fn all_of(&mut self, kind: Kind) -> &'t [NodeId] {
    let count = self.rest.iter().take_while(|&&id| self.tree.kind(id) == kind).count();
    let (all, rest) = self.rest.split_at(count);
    self.rest = rest;
    all
  }
}

impl<'t, 'a> Printer<'t, 'a> {
  /// The document: its definitions, and a line break at the end.
  pub(crate) fn document(&mut self) {
    self.print(self.tree.root());
  }

  // ───────────────────────────── docs ─────────────────────────────

  fn group(&mut self, content: impl FnOnce(&mut Self)) {
    let mark = self.docs.group();
    content(self);
    self.docs.close(mark);
  }

  fn indent(&mut self, content: impl FnOnce(&mut Self)) {
    let mark = self.docs.indent();
    content(self);
    self.docs.close(mark);
  }

  /// `ifBreak("", text)`
  fn flat(&mut self, text: &'static str) {
    let mark = self.docs.if_break(Branch::Flat, None);
    self.docs.text(text);
    self.docs.close(mark);
  }

  fn slice(&self, span: Span) -> &'a str {
    self.text.get(span.start as usize..span.end as usize).unwrap_or_default()
  }

  fn source(&mut self, span: Span) {
    self.docs.text(self.slice(span));
  }

  // ───────────────────────────── comments ─────────────────────────────

  fn comments_of(&self, id: NodeId) -> &'t [Attached] {
    let start = self.attached.partition_point(|it| it.node < id);
    let count = self.attached[start..].iter().take_while(|it| it.node == id).count();
    &self.attached[start..start + count]
  }

  fn comment_span(&self, attached: &Attached) -> Span {
    self.tree.comments.get(attached.comment as usize).copied().unwrap_or_default()
  }

  /// `printComment`: `#` and the comment without the white space at its end.
  fn comment(&mut self, span: Span) {
    let text = self.slice(span);
    let len = trim_js_whitespace_end(text.as_bytes()).len();
    self.docs.text(&text[..len]);
  }

  /// `printLeadingComment`
  fn leading_comment(&mut self, span: Span) {
    self.comment(span);
    self.docs.hardline();
    let bytes = self.text.as_bytes();
    if has_newline(bytes, skip_newline(bytes, skip_spaces(bytes, span.end as usize))) {
      self.docs.hardline();
    }
  }

  /// `printTrailingComment`, for each trailing comment.
  fn trailing_comments(&mut self, comments: &[Attached]) {
    let mut suffixed = false;
    for attached in comments.iter().filter(|it| it.placement == Placement::Trailing) {
      let span = self.comment_span(attached);
      let bytes = self.text.as_bytes();
      let own_line = suffixed || has_newline_backwards(bytes, span.start as usize);
      let mark = self.docs.line_suffix();
      if own_line {
        self.docs.hardline();
        if is_previous_line_empty(bytes, span.start as usize) {
          self.docs.hardline();
        }
      } else {
        self.docs.text(" ");
      }
      self.comment(span);
      self.docs.close(mark);
      if !own_line {
        self.docs.break_parent();
      }
      suffixed = true;
    }
  }

  /// `printDanglingComments` with `indent: true`
  fn dangling_comments(&mut self, id: NodeId) {
    let comments = self.comments_of(id);
    if !comments.iter().any(|it| it.placement == Placement::Dangling) {
      return;
    }
    self.indent(|p| {
      for attached in comments.iter().filter(|it| it.placement == Placement::Dangling) {
        p.docs.hardline();
        p.comment(p.comment_span(attached));
      }
    });
  }

  /// True if a comment of the node is the ignore directive.
  fn ignored(&self, comments: &[Attached]) -> bool {
    comments.iter().any(|it| {
      let comment = self.slice(self.comment_span(it)).as_bytes();
      trim_js_whitespace(comment.get(1..).unwrap_or_default()) == self.options.ignore.as_bytes()
    })
  }

  // ───────────────────────────── nodes ─────────────────────────────

  /// Prints a node with its comments. A node with the ignore directive is printed as it is written.
  fn print(&mut self, id: NodeId) {
    if !self.stack.has_room() {
      return;
    }
    let comments = self.comments_of(id);
    if comments.is_empty() {
      return self.node(id);
    }
    for attached in comments.iter().filter(|it| it.placement == Placement::Leading) {
      self.leading_comment(self.comment_span(attached));
    }
    match self.ignored(comments) {
      true => self.source(self.tree.span(id)),
      false => self.node(id),
    }
    self.trailing_comments(comments);
  }

  fn optional(&mut self, id: Option<NodeId>) {
    if let Some(id) = id {
      self.print(id);
    }
  }

  fn children(&self, id: NodeId) -> Children<'t> {
    Children { tree: self.tree, rest: self.tree.children(id) }
  }

  fn next_line_empty(&self, id: NodeId) -> bool {
    is_next_line_empty(self.text.as_bytes(), self.tree.span(id).end as usize)
  }

  /// `printSequence`, joined by line breaks: an empty line after a node is kept.
  fn sequence(&mut self, nodes: &[NodeId]) {
    for (index, &id) in nodes.iter().enumerate() {
      self.print(id);
      if index + 1 < nodes.len() {
        if self.next_line_empty(id) {
          self.docs.hardline();
        }
        self.docs.hardline();
      }
    }
  }

  /// `{`, `nodes` on their own lines, `}`
  fn block(&mut self, nodes: &[NodeId]) {
    self.docs.text("{");
    self.indent(|p| {
      p.docs.hardline();
      p.sequence(nodes);
    });
    self.docs.hardline();
    self.docs.text("}");
  }

  /// `(a, b)`, or each on its own line without the commas. `keep` keeps the empty lines, as `printSequence` does.
  fn parenthesized(&mut self, nodes: &[NodeId], keep: bool) {
    if nodes.is_empty() {
      return;
    }
    self.group(|p| {
      p.docs.text("(");
      p.indent(|p| {
        p.docs.softline();
        p.list(nodes, keep);
      });
      p.docs.softline();
      p.docs.text(")");
    });
  }

  /// `join([ifBreak("", ", "), softline], nodes)`
  fn list(&mut self, nodes: &[NodeId], keep: bool) {
    for (index, &id) in nodes.iter().enumerate() {
      self.print(id);
      if index + 1 == nodes.len() {
        break;
      }
      if keep && self.next_line_empty(id) {
        self.docs.hardline();
      }
      self.flat(", ");
      self.docs.softline();
    }
  }

  /// `printDescription`
  fn description(&mut self, children: &mut Children<'t>, owner: Kind) {
    let Some(description) = children.next_of(Kind::StringValue) else { return };
    self.print(description);
    let block = self.tree.node(description).is_some_and(|node| node.flags & IS_BLOCK != 0);
    match owner == Kind::InputValueDefinition && !block {
      true => self.docs.line(),
      false => self.docs.hardline(),
    }
  }

  /// `printDirectives`
  fn directives(&mut self, children: &mut Children<'t>, owner: Kind) {
    let directives = children.all_of(Kind::Directive);
    if directives.is_empty() {
      return;
    }
    let join = |p: &mut Self| {
      for (index, &directive) in directives.iter().enumerate() {
        if index > 0 {
          p.docs.line();
        }
        p.print(directive);
      }
    };
    if matches!(owner, Kind::FragmentDefinition | Kind::OperationDefinition) {
      return self.group(|p| {
        p.docs.line();
        join(p);
      });
    }
    self.docs.text(" ");
    self.group(|p| {
      p.indent(|p| {
        p.docs.softline();
        join(p);
      });
    });
  }

  fn operation(&mut self, flags: u8) {
    self.docs.text(match flags {
      MUTATION => "mutation",
      SUBSCRIPTION => "subscription",
      _ => "query",
    });
  }

  /// A string is decoded, and then only `"`, `\` and line breaks are escaped again.
  fn string(&mut self, span: Span) {
    let raw = self.slice(span);
    if !raw.contains('\\') {
      return self.docs.text(raw);
    }
    let mut value = String::with_capacity(raw.len());
    value.push('"');
    let mut rest = raw.get(1..raw.len().saturating_sub(1)).unwrap_or_default();
    while let Some(at) = rest.find('\\') {
      value.push_str(&rest[..at]);
      let (character, len) = read_escape(&rest.as_bytes()[at..]).unwrap_or(('\\', 1));
      match character {
        '"' => value.push_str("\\\""),
        '\\' => value.push_str("\\\\"),
        '\n' => value.push_str("\\n"),
        _ => value.push(character),
      }
      rest = rest.get(at + len..).unwrap_or_default();
    }
    value.push_str(rest);
    value.push('"');
    self.docs.owned(&value);
  }

  /// A block string, from `content`, the text between its `"""`, dedented as graphql-js's `dedentBlockStringLines`
  /// does. Prettier trims a value of one line and the spaces at the end of each line; tidy keeps both, since they are
  /// part of the value.
  fn block_string(&mut self, content: &'a str) {
    let space = |line: &str| line.len() - line.trim_start_matches([' ', '\t']).len();
    let blank = |line: &str| space(line) == line.len();
    let lines = || content.split("\r\n").flat_map(|line| line.split(['\n', '\r']));
    let indent = lines().skip(1).filter(|line| !blank(line)).map(space).min().unwrap_or(usize::MAX);
    let first = lines().position(|line| !blank(line)).unwrap_or(0);
    let count = lines()
      .enumerate()
      .filter(|(_, line)| !blank(line))
      .last()
      .map_or(0, |(last, _)| last + 1 - first);
    let dedented = lines().enumerate().skip(first).take(count).map(|(index, line)| match index {
      0 => line,
      _ => line.get(indent.min(line.len())..).unwrap_or_default(),
    });

    // Line suffixes go before the string, or they would be printed inside it.
    self.docs.line_suffix_boundary();
    self.docs.text("\"\"\"");
    let ends_in_space = |line: &str| line.ends_with([' ', '\t']);
    let padded = space(content) > 0 || ends_in_space(content);
    if count == 1 && padded && !content.contains(['\r', '\n']) {
      self.docs.text(content);
      return self.docs.text("\"\"\"");
    }
    // A `literalline` in a `markAsRoot` breaks the line at the current indent without trimming its end.
    let mark = self.docs.align(Align::Mark);
    let mut at_quotes = count == 1 && first == 0 && space(content) > 0;
    let (mut empty, mut spaced) = (0, false);
    for line in dedented {
      if line.is_empty() {
        empty += 1;
        continue;
      }
      if !std::mem::take(&mut at_quotes) {
        self.breaks(empty + 1, spaced);
      }
      empty = 0;
      self.docs.text(line);
      spaced = ends_in_space(line);
    }
    self.breaks(1, spaced);
    self.docs.text("\"\"\"");
    self.docs.close(mark);
  }

  /// `count` line breaks. The first doesn't trim the line before it if `spaced`.
  fn breaks(&mut self, count: usize, spaced: bool) {
    for index in 0..count {
      match index == 0 && spaced {
        true => self.docs.literalline(),
        false => self.docs.hardline(),
      }
    }
  }

  /// Prints a node without its comments: Prettier's `genericPrint`.
  fn node(&mut self, id: NodeId) {
    let Some(&node) = self.tree.node(id) else { return };
    let kind = node.kind;
    let mut children = self.children(id);
    match kind {
      Kind::Document => {
        self.sequence(children.rest);
        self.docs.hardline();
      }
      Kind::OperationDefinition => {
        self.description(&mut children, kind);
        let operation = node.flags != IS_SHORTHAND;
        let name = children.next_of(Kind::Name);
        let variables = children.all_of(Kind::VariableDefinition);
        if operation {
          self.operation(node.flags);
          if name.is_some() || !variables.is_empty() {
            self.docs.text(" ");
          }
          self.optional(name);
        }
        self.parenthesized(variables, false);
        self.directives(&mut children, kind);
        if operation {
          self.docs.text(" ");
        }
        self.optional(children.next_of(Kind::SelectionSet));
      }
      Kind::FragmentDefinition => {
        self.description(&mut children, kind);
        self.docs.text("fragment ");
        self.optional(children.next_of(Kind::Name));
        self.parenthesized(children.all_of(Kind::VariableDefinition), false);
        self.docs.text(" on ");
        self.optional(children.next_of(Kind::NamedType));
        self.directives(&mut children, kind);
        self.docs.text(" ");
        self.optional(children.next_of(Kind::SelectionSet));
      }
      Kind::SelectionSet => self.block(children.rest),
      Kind::Field => self.group(|p| {
        if node.flags & HAS_ALIAS != 0 {
          p.optional(children.next_of(Kind::Name));
          p.docs.text(": ");
        }
        p.optional(children.next_of(Kind::Name));
        p.parenthesized(children.all_of(Kind::Argument), true);
        p.directives(&mut children, kind);
        if let Some(selection) = children.next_of(Kind::SelectionSet) {
          p.docs.text(" ");
          p.print(selection);
        }
      }),
      Kind::Name
      | Kind::IntValue
      | Kind::FloatValue
      | Kind::EnumValue
      | Kind::BooleanValue
      | Kind::NullValue => self.source(node.span),
      Kind::StringValue if node.flags & IS_BLOCK != 0 => {
        let raw = self.slice(node.span);
        self.block_string(raw.get(3..raw.len().saturating_sub(3)).unwrap_or_default());
      }
      Kind::StringValue => self.string(node.span),
      Kind::Variable => {
        self.docs.text("$");
        self.optional(children.next_of(Kind::Name));
      }
      Kind::ListValue => self.group(|p| {
        p.docs.text("[");
        p.dangling_comments(id);
        if !children.rest.is_empty() {
          p.indent(|p| {
            p.docs.softline();
            p.list(children.rest, false);
          });
        }
        p.docs.softline();
        p.docs.text("]");
      }),
      Kind::ObjectValue => self.group(|p| {
        let space = p.options.bracket_spacing && !children.rest.is_empty();
        p.docs.text(if space { "{ " } else { "{" });
        p.dangling_comments(id);
        if !children.rest.is_empty() {
          p.indent(|p| {
            p.docs.softline();
            p.list(children.rest, false);
          });
        }
        p.docs.softline();
        if space {
          p.flat(" ");
        }
        p.docs.text("}");
      }),
      Kind::ObjectField | Kind::Argument | Kind::FragmentArgument => {
        self.optional(children.next_of(Kind::Name));
        self.docs.text(": ");
        self.optional(children.next_if(Kind::is_value));
      }
      Kind::Directive => {
        self.docs.text("@");
        self.optional(children.next_of(Kind::Name));
        self.parenthesized(children.all_of(Kind::Argument), true);
      }
      Kind::NamedType => self.optional(children.next_of(Kind::Name)),
      Kind::VariableDefinition | Kind::InputValueDefinition => {
        self.description(&mut children, kind);
        self.optional(children.next_if(|it| matches!(it, Kind::Variable | Kind::Name)));
        self.docs.text(": ");
        self.optional(children.next_if(Kind::is_type));
        if let Some(default) = children.next_if(Kind::is_value) {
          self.docs.text(" = ");
          self.print(default);
        }
        self.directives(&mut children, kind);
      }
      Kind::ObjectTypeExtension
      | Kind::ObjectTypeDefinition
      | Kind::InputObjectTypeExtension
      | Kind::InputObjectTypeDefinition
      | Kind::InterfaceTypeExtension
      | Kind::InterfaceTypeDefinition => {
        match kind {
          Kind::ObjectTypeDefinition
          | Kind::InputObjectTypeDefinition
          | Kind::InterfaceTypeDefinition => self.description(&mut children, kind),
          _ => self.docs.text("extend "),
        }
        self.docs.text(match kind {
          Kind::ObjectTypeDefinition | Kind::ObjectTypeExtension => "type ",
          Kind::InputObjectTypeDefinition | Kind::InputObjectTypeExtension => "input ",
          _ => "interface ",
        });
        self.optional(children.next_of(Kind::Name));
        let interfaces = children.all_of(Kind::NamedType);
        if !interfaces.is_empty() {
          self.docs.text(" implements ");
          self.indent(|p| {
            p.group(|p| {
              for (index, &interface) in interfaces.iter().enumerate() {
                if index > 0 {
                  p.docs.text(" &");
                  p.docs.line();
                }
                p.print(interface);
              }
            });
          });
        }
        self.directives(&mut children, kind);
        if !children.rest.is_empty() {
          self.docs.text(" ");
          self.block(children.rest);
        }
      }
      Kind::FieldDefinition => {
        self.description(&mut children, kind);
        self.optional(children.next_of(Kind::Name));
        self.parenthesized(children.all_of(Kind::InputValueDefinition), true);
        self.docs.text(": ");
        self.optional(children.next_if(Kind::is_type));
        self.directives(&mut children, kind);
      }
      Kind::DirectiveDefinition => {
        self.description(&mut children, kind);
        self.docs.text("directive @");
        self.optional(children.next_of(Kind::Name));
        self.parenthesized(children.all_of(Kind::InputValueDefinition), true);
        self.directives(&mut children, kind);
        if node.flags & IS_REPEATABLE != 0 {
          self.docs.text(" repeatable");
        }
        self.docs.text(" on ");
        for (index, &location) in children.rest.iter().enumerate() {
          if index > 0 {
            self.docs.text(" | ");
          }
          self.print(location);
        }
      }
      Kind::DirectiveExtension => {
        self.docs.text("extend directive @");
        self.optional(children.next_of(Kind::Name));
        self.directives(&mut children, kind);
      }
      Kind::EnumTypeExtension | Kind::EnumTypeDefinition => {
        self.description(&mut children, kind);
        if kind == Kind::EnumTypeExtension {
          self.docs.text("extend ");
        }
        self.docs.text("enum ");
        self.optional(children.next_of(Kind::Name));
        self.directives(&mut children, kind);
        if !children.rest.is_empty() {
          self.docs.text(" ");
          self.block(children.rest);
        }
      }
      Kind::EnumValueDefinition => {
        self.description(&mut children, kind);
        self.optional(children.next_of(Kind::Name));
        self.directives(&mut children, kind);
      }
      Kind::SchemaExtension | Kind::SchemaDefinition => {
        self.description(&mut children, kind);
        self.docs.text(if kind == Kind::SchemaExtension { "extend schema" } else { "schema" });
        self.directives(&mut children, kind);
        if !children.rest.is_empty() {
          self.docs.text(" ");
          self.block(children.rest);
        }
      }
      Kind::OperationTypeDefinition => {
        self.operation(node.flags);
        self.docs.text(": ");
        self.optional(children.next_of(Kind::NamedType));
      }
      Kind::FragmentSpread => {
        self.docs.text("...");
        self.optional(children.next_of(Kind::Name));
        self.parenthesized(children.all_of(Kind::FragmentArgument), true);
        self.directives(&mut children, kind);
      }
      Kind::InlineFragment => {
        self.docs.text("...");
        if let Some(condition) = children.next_of(Kind::NamedType) {
          self.docs.text(" on ");
          self.print(condition);
        }
        self.directives(&mut children, kind);
        self.docs.text(" ");
        self.optional(children.next_of(Kind::SelectionSet));
      }
      Kind::UnionTypeExtension | Kind::UnionTypeDefinition => self.group(|p| {
        p.description(&mut children, kind);
        p.group(|p| {
          if kind == Kind::UnionTypeExtension {
            p.docs.text("extend ");
          }
          p.docs.text("union ");
          p.optional(children.next_of(Kind::Name));
          p.directives(&mut children, kind);
          if children.rest.is_empty() {
            return;
          }
          p.docs.text(" =");
          p.flat(" ");
          p.indent(|p| {
            let mark = p.docs.if_break(Branch::Break, None);
            p.docs.line();
            p.docs.text("| ");
            p.docs.close(mark);
            for (index, &member) in children.rest.iter().enumerate() {
              if index > 0 {
                p.docs.line();
                p.docs.text("| ");
              }
              p.print(member);
            }
          });
        });
      }),
      Kind::ScalarTypeExtension | Kind::ScalarTypeDefinition => {
        self.description(&mut children, kind);
        if kind == Kind::ScalarTypeExtension {
          self.docs.text("extend ");
        }
        self.docs.text("scalar ");
        self.optional(children.next_of(Kind::Name));
        self.directives(&mut children, kind);
      }
      Kind::NonNullType => {
        self.optional(children.next_if(Kind::is_type));
        self.docs.text("!");
      }
      Kind::ListType => {
        self.docs.text("[");
        self.optional(children.next_if(Kind::is_type));
        self.docs.text("]");
      }
    }
  }
}
