use pulldown_cmark::Alignment;
use unicode_width::UnicodeWidthStr;

use crate::Wrap;

/// A table collected cell by cell. The first row is the header.
#[derive(Debug)]
pub(crate) struct Table {
  pub(crate) alignments: Vec<Alignment>,
  pub(crate) rows: Vec<Vec<String>>,
}

impl Table {
  /// The lines of the table with its columns aligned. As Prettier does, only `Wrap::Never` falls
  /// back to unpadded cells, and only when the aligned table is wider than `width`.
  pub(crate) fn lines(&self, width: usize, wrap: Wrap) -> Vec<String> {
    let mut widths = vec![3; self.alignments.len()];
    for row in &self.rows {
      for (column, cell) in row.iter().enumerate().take(widths.len()) {
        widths[column] = widths[column].max(cell.trim().width());
      }
    }
    let aligned = self.render(&widths, false);
    if wrap == Wrap::Never && aligned.iter().any(|line| line.width() > width) {
      return self.render(&widths, true);
    }
    aligned
  }

  fn render(&self, widths: &[usize], compact: bool) -> Vec<String> {
    let mut lines = Vec::with_capacity(self.rows.len() + 1);
    for (index, row) in self.rows.iter().enumerate() {
      let mut line = String::from("|");
      for (column, &width) in widths.iter().enumerate() {
        let cell = row.get(column).map_or("", |cell| cell.trim());
        let pad = if compact { 0 } else { width - cell.width() };
        let before = match self.alignments[column] {
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
    for (column, &width) in widths.iter().enumerate() {
      let alignment = self.alignments[column];
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

#[cfg(test)]
mod tests {
  use super::*;

  fn table() -> Table {
    Table {
      alignments: vec![Alignment::None, Alignment::Center, Alignment::Right],
      rows: vec![
        vec!["a".into(), "b".into(), "c".into()],
        vec!["long cell".into(), "x".into(), "1".into()],
      ],
    }
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
    let table = Table { alignments: vec![Alignment::None], rows: vec![vec!["日本".into()]] };
    assert_eq!(table.lines(80, Wrap::Preserve), ["| 日本 |", "| ---- |"]);
  }
}
