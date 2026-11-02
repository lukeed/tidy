//! Prints a file as the plugin's printer does, through Prettier: a hard line drops the spaces and tabs at the end of
//! the line before it.

use super::Options;
use super::parse::{Kind, Line, Node, Row, Tree};

pub(crate) fn print(tree: &Tree, options: &Options, out: &mut String) {
  let indent = if options.tabs { "\t".to_owned() } else { " ".repeat(options.tab_width) };
  let mut writer = Writer { out, eol: options.eol.text(), indent, spaces: options.spaces };
  match tree {
    Tree::Plugin(nodes) => {
      writer.nodes(nodes);
      writer.newline(false);
    }
    Tree::Kept(rows) => writer.rows(rows),
  }
}

struct Writer<'o> {
  out: &'o mut String,
  eol: &'static str,
  indent: String,
  spaces: bool,
}

impl Writer<'_> {
  /// The plugin's `printLines`: a blank line where the node starts more than a line after the last one ended. An empty
  /// section ends with a line break of its own, so the plugin adds a blank line after it on each pass; tidy adds none.
  fn nodes(&mut self, nodes: &[Node]) {
    let mut last: Option<&Node> = None;
    for node in nodes {
      if let Some(last) = last {
        if node.start.saturating_sub(last.end) > 1 && !empty(last) {
          self.newline(false);
        }
        self.newline(false);
      }
      self.node(node);
      last = Some(node);
    }
  }

  fn node(&mut self, node: &Node) {
    match &node.kind {
      Kind::Comment(text) => self.out.push_str(text),
      Kind::Param { key, value } => self.param(key, value),
      Kind::List { key, values } => {
        self.out.push_str(key);
        self.out.push_str(if self.spaces { " =" } else { "=" });
        self.newline(true);
        for (index, value) in values.iter().enumerate() {
          if index > 0 {
            self.newline(true);
          }
          self.out.push_str(value);
        }
      }
      Kind::Section { name, children } => {
        self.section(name);
        self.newline(false);
        self.nodes(children);
      }
    }
  }

  /// Rows with their indents, and one blank line where the text has any.
  fn rows(&mut self, rows: &[Row]) {
    let mut blank = false;
    for row in rows {
      if let Line::Blank = row.line {
        blank = !self.out.is_empty();
        continue;
      }
      if blank {
        self.out.push_str(self.eol);
        blank = false;
      }
      self.out.push_str(row.indent);
      match row.line {
        Line::Blank => {}
        Line::Comment(text) | Line::Text(text) => self.out.push_str(text),
        Line::Param { key, value } => self.param(key, value),
        Line::Section { name, comment } => {
          self.section(name);
          if let Some(comment) = comment {
            self.out.push(' ');
            self.out.push_str(comment);
          }
        }
      }
      self.newline(false);
    }
  }

  fn param(&mut self, key: &str, value: &str) {
    self.out.push_str(key);
    self.out.push_str(if self.spaces { " = " } else { "=" });
    self.out.push_str(value);
  }

  fn section(&mut self, name: &str) {
    self.out.push('[');
    self.out.push_str(name);
    self.out.push(']');
  }

  fn newline(&mut self, indented: bool) {
    let end = self.out.trim_end_matches([' ', '\t']).len();
    self.out.truncate(end);
    self.out.push_str(self.eol);
    if indented {
      self.out.push_str(&self.indent);
    }
  }
}

fn empty(node: &Node) -> bool {
  matches!(&node.kind, Kind::Section { children, .. } if children.is_empty())
}
