use pulldown_cmark::Parser;

use super::{Gap, Writer, flags, inline};
use crate::Wrap;
use crate::width::width;

impl<'s> Writer<'s, '_> {
  /// Write the link reference definitions that come before `at`. pulldown-cmark reports each label
  /// once, so the gap since the last sibling block is searched for the definitions it left out:
  /// with no events of their own, they are all that a gap can hold.
  pub(super) fn flush(&mut self, at: usize) {
    let at = at.min(self.source.len());
    while let Some(definition) =
      self.definitions.get(self.next).filter(|definition| definition.span.start < at)
    {
      let span = definition.span.clone();
      let line = definition.line.clone();
      self.next += 1;
      self.leftovers(span.start);
      if !self.ignored.contains(&span.start) {
        self.define(span.clone(), line);
      }
      self.cursor = Some(span.end);
    }
    self.leftovers(at);
  }

  /// Write the definitions between the cursor and `to` that pulldown-cmark left out.
  fn leftovers(&mut self, to: usize) {
    let Some(mut from) = self.cursor.filter(|&from| from < to) else { return };
    while from < to && self.source[from..to].contains('[') {
      let chunk = &self.source[from..to];
      let parser = Parser::new_ext(chunk, flags());
      let first = parser.reference_definitions().iter().map(|(_, definition)| definition);
      let Some(definition) = first.min_by_key(|definition| definition.span.start) else { break };
      let span = from + definition.span.start..from + definition.span.end;
      let title = definition.title.as_deref();
      let line = inline::definition(&self.source[span.clone()], &definition.dest, title);
      if !self.ignored.contains(&span.start) {
        self.define(span.clone(), line);
      }
      from = span.end;
    }
    self.cursor = Some(to);
  }

  /// Write one definition, beside the one before it, as Prettier does. When wrapping, one too wide
  /// for its line puts its destination and title on lines of their own, indented 2.
  fn define(&mut self, span: std::ops::Range<usize>, defined: Option<inline::Defined>) {
    self.adjacent = None;
    self.joins = None;
    if self.gap == Gap::Source {
      self.gap = Gap::Blank;
    }
    if self.defined == Some((self.out.len(), self.containers.len())) {
      self.gap = Gap::Line;
    }
    self.separate();
    match defined {
      Some(defined) => {
        let line = defined.line();
        let room = self.options.width.saturating_sub(self.indent());
        if self.options.wrap == Wrap::Always && width(&line) > room {
          self.write(&defined.head);
          for part in [Some(&defined.url), defined.title.as_ref()].into_iter().flatten() {
            self.newline();
            self.write("  ");
            self.write(part);
          }
        } else {
          self.write(&line);
        }
      }
      None => {
        let raw = self.source[span].trim_end();
        let mut lines = raw.lines();
        if let Some(first) = lines.next() {
          self.write(first.trim_start_matches([' ', '\t', '>']));
        }
        for line in lines {
          self.newline();
          self.write(line.trim_start_matches([' ', '\t', '>']));
        }
      }
    }
    self.newline();
    self.gap = Gap::Blank;
    self.defined = Some((self.out.len(), self.containers.len()));
  }
}
