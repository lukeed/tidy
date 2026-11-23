//! Whether the output of a format would format differently again. Prettier attaches a comment by where it sits: on a
//! line of its own, at the end of a line, or between code on one line, and by whether a block comment spans lines. A
//! comment that the output places differently than the input did can attach to other code when the output is formatted
//! again, and so can a JSDoc comment that moved to another indent, as it was wrapped for the old one. Prettier then
//! prints something else on the next run; tidy prints that next output instead, which stays.

/// Where a comment sits on its lines.
#[derive(PartialEq, Eq, Debug)]
enum Placement {
  /// Only white space before it on its line.
  OwnLine,
  /// Only white space after it on its last line.
  EndOfLine,
  Remaining,
}

/// True if a comment sits differently in `output` than in `input`. `printed` holds the start of each printed comment in
/// both, as `(input, output)`. With `tab_width`, JSDoc comments are formatted, so a change of indent counts too.
pub(crate) fn is_unsettled(
  input: &[u8],
  output: &[u8],
  printed: &[(u32, u32)],
  tab_width: Option<usize>,
) -> bool {
  printed.iter().any(|&(from, to)| {
    let (from, to) = (from as usize, to as usize);
    placement(input, from) != placement(output, to)
      || spans_lines(input, from) != spans_lines(output, to)
      || tab_width.is_some_and(|tab| {
        input[from..].starts_with(b"/**") && indent(input, from, tab) != indent(output, to, tab)
      })
  })
}

/// The end of the comment at `start` of `text`.
fn end(text: &[u8], start: usize) -> usize {
  let rest = &text[start..];
  let len = match rest.starts_with(b"//") {
    true => rest.iter().position(|&byte| matches!(byte, b'\n' | b'\r')).unwrap_or(rest.len()),
    false => crate::strings::index_of(&rest[2..], b"*/").map_or(rest.len(), |at| at + 4),
  };
  start + len
}

/// True if the comment at `start` of `text` is a block comment that spans lines.
fn spans_lines(text: &[u8], start: usize) -> bool {
  text[start..end(text, start)].iter().any(|&byte| matches!(byte, b'\n' | b'\r'))
}

/// Where the comment at `start` of `text` sits, as Prettier's `attach` tells.
fn placement(text: &[u8], start: usize) -> Placement {
  let before = &text[..start];
  let blank = before.iter().rev().take_while(|&&byte| matches!(byte, b' ' | b'\t')).count();
  if matches!(before.len().checked_sub(blank + 1).map(|at| before[at]), None | Some(b'\n' | b'\r'))
  {
    return Placement::OwnLine;
  }
  match text[end(text, start)..].iter().find(|&&byte| !matches!(byte, b' ' | b'\t')) {
    None | Some(b'\n' | b'\r') => Placement::EndOfLine,
    Some(_) => Placement::Remaining,
  }
}

/// The width of the white space right before `start`, as the JSDoc formatter measures it.
fn indent(text: &[u8], start: usize, tab_width: usize) -> usize {
  let before = text[..start].iter().rev().take_while(|&&byte| matches!(byte, b' ' | b'\t'));
  before.map(|&byte| if byte == b'\t' { tab_width } else { 1 }).sum()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tells_where_a_comment_sits() {
    let text = b"a\n  // own\nb; // end\nc /* mid */ d;\n/* end */\n";
    let at = |needle: &[u8]| text.windows(needle.len()).position(|it| it == needle).unwrap();
    assert_eq!(placement(text, at(b"// own")), Placement::OwnLine);
    assert_eq!(placement(text, at(b"// end")), Placement::EndOfLine);
    assert_eq!(placement(text, at(b"/* mid */")), Placement::Remaining);
    assert_eq!(placement(text, at(b"/* end */")), Placement::OwnLine);
    assert_eq!(placement(b"/* first */ a", 0), Placement::OwnLine);
  }

  #[test]
  fn finds_a_comment_that_moved() {
    let input = b"a satisfies\n// c\nB;\n";
    let output = b"a satisfies // c\nB;\n";
    assert!(is_unsettled(input, output, &[(12, 12)], None));
    assert!(!is_unsettled(input, input, &[(12, 12)], None));
  }

  #[test]
  fn finds_a_block_comment_that_joined_its_lines() {
    let input = b"/**\n * a\n */\nb;\n";
    let output = b"/** A */\nb;\n";
    assert!(is_unsettled(input, output, &[(0, 0)], None));
  }

  #[test]
  fn finds_a_jsdoc_comment_that_changed_indent() {
    let input = b"/** a */\nb;\n";
    let output = b"  /** a */\nb;\n";
    assert!(is_unsettled(input, output, &[(0, 2)], Some(2)));
    assert!(!is_unsettled(input, output, &[(0, 2)], None));
  }
}
