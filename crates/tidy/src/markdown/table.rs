use std::ops::Range;

use crate::Wrap;
use crate::width::width;
use pulldown_cmark::Alignment;

/// A table collected cell by cell. The first row is the header. Like Prettier, each row keeps the
/// cells it was written with, even where they don't match the header.
#[derive(Debug)]
pub(crate) struct Table {
  pub(crate) alignments: Vec<Alignment>,
  pub(crate) rows: Vec<Vec<String>>,
  /// The source of the current row, and where its last cell ended.
  row: Range<usize>,
  last: usize,
}

impl Table {
  pub(crate) fn new(alignments: Vec<Alignment>) -> Self {
    Self { alignments, rows: Vec::new(), row: 0..0, last: 0 }
  }

  pub(crate) fn row(&mut self, range: Range<usize>) {
    self.rows.push(Vec::new());
    self.last = range.start;
    self.row = range;
  }

  /// Start a cell, unless pulldown-cmark made it up to fill a short row: it sits empty at the end.
  pub(crate) fn cell(&mut self, range: Range<usize>) {
    if range.is_empty() && range.start == self.row.end {
      return;
    }
    self.last = range.end;
    if let Some(row) = self.rows.last_mut() {
      row.push(String::new());
    }
  }

  /// End a row: add the cells past the header's count, which pulldown-cmark drops, as written.
  pub(crate) fn end(&mut self, source: &str) {
    let rest = source[self.last..self.row.end].trim_end();
    let rest = rest.strip_prefix('|').unwrap_or(rest);
    let mut cells = split(rest);
    if rest.is_empty() || (rest.ends_with('|') && !rest.ends_with("\\|")) {
      cells.pop();
    }
    if let Some(row) = self.rows.last_mut() {
      row.extend(cells.into_iter().map(|cell| cell.trim().to_string()));
    }
  }

  /// The lines of the table with its columns aligned. As Prettier does, only `Wrap::Never` falls
  /// back to unpadded cells, and only when the aligned table is wider than `width`.
  pub(crate) fn lines(&self, max: usize, wrap: Wrap) -> Vec<String> {
    let columns = self.rows.iter().map(Vec::len).max().unwrap_or(0).max(self.alignments.len());
    let sizes: Vec<Vec<usize>> =
      self.rows.iter().map(|row| row.iter().map(|cell| width(cell.trim())).collect()).collect();
    let mut widths = vec![3; columns];
    for row in &sizes {
      for (column, &size) in row.iter().enumerate() {
        widths[column] = widths[column].max(size);
      }
    }
    let aligned = self.render(&widths, &sizes, false);
    if wrap == Wrap::Never && aligned.iter().any(|line| width(line) > max) {
      return self.render(&widths, &sizes, true);
    }
    aligned
  }

  fn render(&self, widths: &[usize], sizes: &[Vec<usize>], compact: bool) -> Vec<String> {
    let mut lines = Vec::with_capacity(self.rows.len() + 1);
    for (index, (row, sizes)) in self.rows.iter().zip(sizes).enumerate() {
      let mut line = String::from("|");
      for (column, ((cell, &size), &width)) in row.iter().zip(sizes).zip(widths).enumerate() {
        let cell = cell.trim();
        let pad = if compact { 0 } else { width - size };
        let alignment = self.alignments.get(column).copied().unwrap_or(Alignment::None);
        let before = match alignment {
          Alignment::Right => pad,
          Alignment::Center => pad / 2,
          Alignment::Left | Alignment::None => 0,
        };
        line.push(' ');
        line.extend(std::iter::repeat_n(' ', before));
        line.push_str(cell);
        line.extend(std::iter::repeat_n(' ', pad - before));
        line.push_str(" |");
      }
      lines.push(line);
      if index == 0 {
        lines.push(self.delimiters(widths, compact));
      }
    }
    lines
  }

  fn delimiters(&self, widths: &[usize], compact: bool) -> String {
    let mut line = String::from("|");
    for (&alignment, &width) in self.alignments.iter().zip(widths) {
      let first = matches!(alignment, Alignment::Left | Alignment::Center);
      let last = matches!(alignment, Alignment::Right | Alignment::Center);
      line.push(' ');
      line.push(if first { ':' } else { '-' });
      line.extend(std::iter::repeat_n('-', if compact { 1 } else { width - 2 }));
      line.push(if last { ':' } else { '-' });
      line.push_str(" |");
    }
    line
  }
}

/// The text between unescaped pipes.
fn split(text: &str) -> Vec<&str> {
  let mut cells = Vec::new();
  let mut start = 0;
  let mut escaped = false;
  for (index, byte) in text.bytes().enumerate() {
    match byte {
      b'|' if !escaped => {
        cells.push(&text[start..index]);
        start = index + 1;
      }
      _ => escaped = byte == b'\\' && !escaped,
    }
  }
  cells.push(&text[start..]);
  cells
}

#[cfg(test)]
mod tests {
  use super::*;

  fn table() -> Table {
    let mut table = Table::new(vec![Alignment::None, Alignment::Center, Alignment::Right]);
    table.rows = vec![
      vec!["a".into(), "b".into(), "c".into()],
      vec!["long cell".into(), "x".into(), "1".into()],
    ];
    table
  }

  #[test]
  fn aligns_columns() {
    assert_eq!(
      table().lines(80, Wrap::Preserve),
      ["| a         |  b  |   c |", "| --------- | :-: | --: |", "| long cell |  x  |   1 |",]
    );
  }

  #[test]
  fn compacts_only_when_never_wrapping_a_wide_table() {
    assert_eq!(table().lines(10, Wrap::Preserve)[0], "| a         |  b  |   c |");
    assert_eq!(
      table().lines(10, Wrap::Never),
      ["| a | b | c |", "| --- | :-: | --: |", "| long cell | x | 1 |"]
    );
  }

  #[test]
  fn measures_wide_characters() {
    let mut table = Table::new(vec![Alignment::None]);
    table.rows = vec![vec!["日本".into()]];
    assert_eq!(table.lines(80, Wrap::Preserve), ["| 日本 |", "| ---- |"]);
  }

  #[test]
  fn splits_cells_on_unescaped_pipes() {
    assert_eq!(split(" a | b \\| c |"), [" a ", " b \\| c ", ""]);
  }
}
