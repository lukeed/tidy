//! Prettier's `print/comma-separated-value-group.js` and `print/parenthesized-value-group.js`.

use super::Dialect as Syntax;
use super::misc::{ends_with_hex_escape, ends_with_unescaped_backslash};
use super::postcss::Kind;
use super::printer::{Printer, Statement};
use super::sink::Separator;
use super::value_parser::{Before, ValueId, ValueKind, ValueRef};
use crate::syntax::{Message, SyntaxError};
use crate::text::{self, is_next_line_empty};

/// `isAtWordPlaceholderNode`
fn is_at_word_placeholder(node: ValueRef<'_>) -> bool {
  super::printer::is_at_word_placeholder(node.values, node.id)
}

fn is_operator(node: ValueRef<'_>, operator: u8) -> bool {
  node.kind() == ValueKind::Operator && node.node().first_byte == operator
}

fn is_multiplication(node: ValueRef<'_>) -> bool {
  is_operator(node, b'*')
}

fn is_division(node: ValueRef<'_>) -> bool {
  is_operator(node, b'/')
}

fn is_addition(node: ValueRef<'_>) -> bool {
  is_operator(node, b'+')
}

fn is_subtraction(node: ValueRef<'_>) -> bool {
  is_operator(node, b'-')
}

fn is_math_operator(node: ValueRef<'_>) -> bool {
  node.kind() == ValueKind::Operator
    && matches!(node.node().first_byte, b'*' | b'/' | b'+' | b'-' | b'%')
}

fn word(node: ValueRef<'_>) -> Option<&[u8]> {
  node.value().filter(|_| node.kind() == ValueKind::Word)
}

fn is_the_word(node: ValueRef<'_>, text: &[u8]) -> bool {
  word(node) == Some(text)
}

fn is_one_of_the_words(node: ValueRef<'_>, words: &[&[u8]]) -> bool {
  word(node).is_some_and(|it| words.contains(&it))
}

/// `isWordNode`
fn is_word_or_at_word(node: ValueRef<'_>) -> bool {
  matches!(node.kind(), ValueKind::Word | ValueKind::AtWord)
}

fn is_func(node: ValueRef<'_>) -> bool {
  node.kind() == ValueKind::Func
}

fn is_colon(node: ValueRef<'_>) -> bool {
  node.kind() == ValueKind::Colon
}

fn is_comment(node: ValueRef<'_>) -> bool {
  node.kind() == ValueKind::Comment
}

fn is_inline_comment(node: ValueRef<'_>) -> bool {
  node.kind() == ValueKind::Comment && node.node().inline
}

fn has_empty_raw_before(node: ValueRef<'_>) -> bool {
  node.node().before == Before::Empty
}

/// True if there's no whitespace before `node` in the source.
fn has_no_whitespace_before(mut node: ValueRef<'_>) -> bool {
  loop {
    let first = match node.kind() {
      ValueKind::ParenGroup if node.node().open != 0 => Some(node.at(node.node().open)),
      ValueKind::ParenGroup | ValueKind::CommaGroup => node.group(0),
      _ => None,
    };
    match first {
      Some(first) => node = first,
      None => return has_empty_raw_before(node),
    }
  }
}

/// `isParenGroupNode`. Returns the `(` if `node` is one.
fn paren_group_open(node: ValueRef<'_>) -> Option<ValueRef<'_>> {
  let group = node.node();
  (group.kind == ValueKind::ParenGroup && group.open != 0 && group.close != 0)
    .then(|| node.at(group.open))
}

/// True if `node` ends with a backslash that escapes the whitespace after it.
fn ends_with_escaped_whitespace(node: ValueRef<'_>) -> bool {
  node.kind() == ValueKind::Word
    && node.value().is_some_and(ends_with_unescaped_backslash)
    && node.whitespace_after() > 0
}

fn ends_where_starts(a: ValueRef<'_>, b: ValueRef<'_>) -> bool {
  a.node().loc.end_offset().is_some() && a.node().loc.end_offset() == b.node().loc.start_offset()
}

/// `isColorAdjusterFuncNode`
fn is_color_adjuster_func(node: ValueRef<'_>) -> bool {
  const NAMES: [&[u8]; 25] = [
    b"red",
    b"green",
    b"blue",
    b"alpha",
    b"a",
    b"rgb",
    b"hue",
    b"h",
    b"saturation",
    b"s",
    b"lightness",
    b"l",
    b"whiteness",
    b"w",
    b"blackness",
    b"b",
    b"tint",
    b"shade",
    b"blend",
    b"blenda",
    b"contrast",
    b"hsl",
    b"hsla",
    b"hwb",
    b"hwba",
  ];
  is_func(node)
    && node.value().is_some_and(|value| NAMES.iter().any(|name| value.eq_ignore_ascii_case(name)))
}

fn is_possible_font_size(node: Option<ValueRef<'_>>) -> bool {
  match node.map(ValueRef::kind) {
    Some(ValueKind::Number) => true,
    Some(ValueKind::Func) => {
      let value = node.and_then(ValueRef::value).unwrap_or_default().to_ascii_lowercase();
      matches!(&value[..], b"var" | b"calc" | b"min" | b"max" | b"clamp")
        || value.starts_with(b"--")
    }
    _ => false,
  }
}

/// `isKeyValuePairNode`
fn is_key_value_pair(node: ValueRef<'_>) -> bool {
  node.kind() == ValueKind::CommaGroup && node.group(1).is_some_and(is_colon)
}

/// `isKeyValuePairInParenGroupNode`
fn is_key_value_pair_in_paren_group(node: ValueRef<'_>) -> bool {
  node.kind() == ValueKind::ParenGroup && node.group(0).is_some_and(is_key_value_pair)
}

/// The wrapper that `printCommaSeparatedValueGroup` puts around the parts.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Shape {
  /// `group(indent(parts))`
  Indent,
  /// `group(fill(parts))`
  Fill,
  /// `group(indent(fill(parts)))`
  IndentFill,
}

impl<'a> Printer<'a, '_> {
  /// Returns an ancestor, within the value, of the node being printed. Level 0 is the parent, level 1 the
  /// grandparent.
  fn value_ancestor<'v>(&self, statement: Statement<'v, 'a>, level: usize) -> Option<ValueRef<'v>> {
    let id = *self.value_stack.get(self.value_stack.len().checked_sub(level + 1)?)?;
    Some(ValueRef { values: statement.values, id })
  }

  /// `node` is a `value-comma_group`.
  fn shape_of_comma_group(&self, statement: Statement<'_, 'a>, node: ValueRef<'_>) -> Shape {
    let at_rule = statement.at_rule;
    if at_rule.is_some_and(|it| self.is_scss_control_directive(it)) {
      Shape::Indent
    } else if node.groups().len() == 2
      && node.group(0).and_then(ValueRef::value) == Some(b"url")
      && at_rule.is_some_and(|it| it.name == b"import")
    {
      // `insideURLFunctionInImportAtRuleNode`: `@import url("very long") projection,tv`
      Shape::Fill
    } else {
      Shape::IndentFill
    }
  }

  pub(crate) fn print_comma_separated_value_group(
    &mut self,
    statement: Statement<'_, 'a>,
    id: ValueId,
  ) {
    let node = ValueRef { values: statement.values, id };
    let values = statement.values;
    let parent = self.value_ancestor(statement, 0);
    let grandparent = self.value_ancestor(statement, 1);
    // `getPropOfDeclNode`
    let declaration_prop =
      statement.css_ancestor(Kind::Decl).map(|node| text::to_lower_case(&node.prop));
    let is_grid_value = parent.is_some_and(|it| it.kind() == ValueKind::Value)
      && declaration_prop
        .as_ref()
        .is_some_and(|prop| **prop == *b"grid" || prop.starts_with(b"grid-template"));
    let at_rule = statement.at_rule;
    let is_control_directive = at_rule.is_some_and(|it| self.is_scss_control_directive(it));
    let has_inline_comment = node.groups().any(is_inline_comment);
    let is_custom_property = statement.is_custom_property();
    let is_in_paren_group = parent.is_some_and(|it| it.kind() == ValueKind::ParenGroup);

    // `insideValueFunctionNode`
    let function = self.value_function(values);
    let inside_url = function.is_some_and(|name| name.eq_ignore_ascii_case(b"url"));
    let inside_calc = function.is_some_and(|name| name.eq_ignore_ascii_case(b"calc"));
    let inside_type = function.is_some_and(|name| name.eq_ignore_ascii_case(b"type"));
    let is_in_scss_if = self.syntax() == Syntax::Scss
      && is_in_paren_group
      && grandparent.is_some_and(|it| is_func(it) && it.value() == Some(b"if"));

    let shape = self.shape_of_comma_group(statement, node);
    self.sink.start_group(false);
    if shape != Shape::Fill {
      self.sink.start_indent();
    }
    self.sink.start_fill();
    let mut inside_scss_interpolation_in_string = false;
    let mut did_break = false;

    for (i, i_node) in node.groups().enumerate() {
      let prev_node = i.checked_sub(1).and_then(|at| node.group(at));
      let next_node = node.group(i + 1);

      let is_at_end_of_line = is_inline_comment(i_node) && next_node.is_none();
      if is_at_end_of_line {
        self.sink.start_line_suffix();
        self.sink.token(" ");
      }
      // `100${unit}` and `0.${n}` in a JavaScript template literal become one number, or a number with its unit,
      // once the template is evaluated. Prettier prints `100 ${unit}` and `0 ${n}`.
      let number_before_placeholder = next_node
        .filter(|next| is_at_word_placeholder(*next) && i_node.is_number_right_before(*next))
        .and_then(|_| i_node.number_source_text());
      self.index_in_comma_group = i;
      match number_before_placeholder {
        Some(number) => self.sink.text(number),
        None => {
          self.value_stack.push(id);
          self.print_value(statement, i_node.id, prev_node.map(|it| it.id));
          self.value_stack.pop();
        }
      }
      if is_at_end_of_line {
        self.sink.end_line_suffix();
        continue;
      }

      if inside_url {
        if next_node.is_some_and(is_addition) || is_addition(i_node) {
          self.sink.token(" ");
        }
        continue;
      }

      // The wildcard in `@forward .. as a-*`.
      if statement.inside_at_rule(&[b"forward"])
        && word(i_node).is_some_and(|it| !it.is_empty())
        && prev_node.is_some_and(|it| is_the_word(it, b"as"))
      {
        // Prettier assumes that another node follows and throws if there's none.
        if next_node.is_none() {
          self.fail(SyntaxError(Message::UnreadableAtRule, statement.node().start as u32));
        }
        if next_node.is_some_and(is_multiplication) {
          continue;
        }
      }
      // Tailwind's `@utility a-*`.
      if statement.inside_at_rule(&[b"utility"])
        && word(i_node).is_some()
        && next_node.is_some_and(is_multiplication)
      {
        continue;
      }
      let Some(next_node) = next_node else {
        // `(b a\ )`, which Prettier prints as `(b a\)`. At the end of the whole value, the declaration's
        // printer handles this.
        if is_in_paren_group && ends_with_escaped_whitespace(i_node) && i_node.has_more_after() {
          self.sink.token(" ");
        }
        continue;
      };
      if is_custom_property && has_no_whitespace_before(next_node) {
        continue;
      }
      // `if(condition: value; else: value)`
      if is_in_scss_if && is_the_word(next_node, b";") {
        continue;
      }
      if number_before_placeholder.is_some()
        || (word(i_node).is_some()
          && is_at_word_placeholder(next_node)
          && ends_where_starts(i_node, next_node))
      {
        continue;
      }

      // `"#{my-fn("_")}"`
      if let Some(value) = i_node.value().filter(|_| i_node.kind() == ValueKind::String) {
        let opening = crate::strings::last_index_of(value, b"#{");
        let closing = crate::strings::last_index_of_char(value, b'}');
        match (opening, closing) {
          (Some(opening), Some(closing)) => inside_scss_interpolation_in_string = opening > closing,
          (Some(_), None) => inside_scss_interpolation_in_string = true,
          (None, Some(_)) => inside_scss_interpolation_in_string = false,
          (None, None) => {}
        }
      }
      if inside_scss_interpolation_in_string {
        continue;
      }

      if is_colon(i_node) || is_colon(next_node) {
        continue;
      }
      // `@@var`, `@var[ @notVarNested ][notVar]` in Less.
      if i_node.kind() == ValueKind::AtWord
        && i_node.value().is_some_and(|value| value.is_empty() || value.ends_with(b"["))
      {
        continue;
      }
      if word(next_node).is_some_and(|it| it.starts_with(b"]")) {
        continue;
      }
      // `~"escaped"` in Less.
      if i_node.value() == Some(b"~") {
        continue;
      }
      if self.syntax() == Syntax::Less {
        if is_the_word(next_node, b"[") {
          continue;
        }
        if word(i_node).is_some_and(|it| it.ends_with(b"[")) && is_word_or_at_word(next_node) {
          continue;
        }
      }

      let is_next_word = matches!(
        next_node.kind(),
        ValueKind::Word
          | ValueKind::Number
          | ValueKind::String
          | ValueKind::Func
          | ValueKind::AtWord
      );
      // The first space after `\31` ends the escape, so `\31 x` is one word and `\31  x` is two. Prettier can
      // break the line there, which splits that word in two. The parser reads `\31` as the unit of a number
      // without digits.
      if i_node.kind() == ValueKind::Number
        && is_next_word
        && i_node.whitespace_after() > 0
        && i_node.number_source_text().is_some_and(ends_with_hex_escape)
      {
        self.sink.token(match i_node.whitespace_after() {
          1 => " ",
          _ => "  ",
        });
        continue;
      }
      let i_value = i_node.value().filter(|it| !it.is_empty());
      if i_node.kind() != ValueKind::String
        && i_value.is_some_and(|it| crate::strings::contains_char(it, b'\\'))
        && !is_comment(next_node)
      {
        // Prettier prints nothing between the two: `Font\ Awesome` becomes `Font\Awesome`, and `a\b c` becomes
        // one word.
        if next_node.node().before == Before::Spaces {
          let is_word = is_next_word;
          match ends_with_escaped_whitespace(i_node) {
            true if is_word && i_node.whitespace_after() > 1 => self.sink.token("  "),
            true => self.sink.token(" "),
            // `\e9  x`
            false
              if is_word
                && i_node.whitespace_after() > 1
                && i_value.is_some_and(ends_with_hex_escape) =>
            {
              self.sink.token("  ")
            }
            false if is_word => self.sink.token(" "),
            false => {}
          }
        }
        continue;
      }
      // An escaped `/`.
      if prev_node.and_then(ValueRef::value).is_some_and(|it| {
        !it.is_empty() && crate::strings::index_of_char_usize(it, b'\\') == Some(it.len() - 1)
      }) && is_division(i_node)
      {
        continue;
      }
      if i_node.value() == Some(b"\\") {
        continue;
      }
      // `isPostcssSimpleVarNode`: `$$(style)Color`
      if is_func(i_node)
        && i_node.value() == Some(b"$$")
        && word(next_node).is_some()
        && next_node.node().before != Before::Spaces
      {
        continue;
      }

      // `#{variable}`
      let is_hash = |node: ValueRef<'_>| is_the_word(node, b"#");
      let is_left_brace = |node: ValueRef<'_>| is_the_word(node, b"{");
      let is_right_brace = |node: ValueRef<'_>| is_the_word(node, b"}");
      if is_hash(i_node)
        || is_left_brace(i_node)
        || is_right_brace(next_node)
        || (is_left_brace(next_node) && has_empty_raw_before(next_node))
        || (is_right_brace(i_node) && has_empty_raw_before(next_node))
      {
        continue;
      }
      // `--#{$var}`
      if i_node.value() == Some(b"--") && is_hash(next_node) {
        continue;
      }

      let is_math = is_math_operator(i_node);
      let is_next_math = is_math_operator(next_node);

      // Next to an interpolation, the spaces around an operator stay as they are.
      if ((is_math && is_hash(next_node)) || (is_next_math && is_right_brace(i_node)))
        && has_empty_raw_before(next_node)
      {
        continue;
      }
      // `type(<number>+)`
      if is_addition(next_node) && inside_type && has_empty_raw_before(next_node) {
        continue;
      }
      // `-fb-url(/abs/path/)`
      if prev_node.is_none() && is_division(i_node) {
        continue;
      }
      // In `calc()`, the spaces around `+` and `-` stay as they are.
      if inside_calc
        && (is_addition(i_node)
          || is_addition(next_node)
          || is_subtraction(i_node)
          || is_subtraction(next_node))
        && has_empty_raw_before(next_node)
      {
        continue;
      }
      // A unary minus before a function.
      if self.syntax() == Syntax::Scss
        && is_math
        && is_subtraction(i_node)
        && is_func(next_node)
        && !ends_where_starts(i_node, next_node)
      {
        self.sink.token(" ");
        continue;
      }

      let next_next_node = node.group(i + 2);

      // `color(red l(+ 20%))`
      let is_color_adjuster = (is_addition(i_node) || is_subtraction(i_node))
        && i == 0
        && (next_node.kind() == ValueKind::Number
          || (next_node.kind() == ValueKind::Word && next_node.node().is_hex))
        && grandparent.is_some_and(is_color_adjuster_func)
        && !has_empty_raw_before(next_node);
      let is_func_or_word = |node: ValueRef<'_>| is_func(node) || is_word_or_at_word(node);
      let require_space_before_operator =
        next_next_node.is_some_and(is_func_or_word) || is_func_or_word(i_node);
      let require_space_after_operator =
        is_func_or_word(next_node) || prev_node.is_some_and(is_func_or_word);

      // `/`, `+`, `-`
      if !(is_multiplication(next_node) || is_multiplication(i_node))
        && !inside_calc
        && !is_color_adjuster
        && ((is_division(next_node) && !require_space_before_operator)
          || (is_division(i_node) && !require_space_after_operator)
          || (is_addition(next_node) && !require_space_before_operator)
          || (is_addition(i_node) && !require_space_after_operator)
          || is_subtraction(next_node)
          || is_subtraction(i_node))
        && (has_empty_raw_before(next_node) || (is_math && prev_node.is_none_or(is_math_operator)))
      {
        continue;
      }

      // `-(`
      if matches!(self.syntax(), Syntax::Scss | Syntax::Less)
        && is_math
        && is_subtraction(i_node)
        && paren_group_open(next_node).is_some_and(|open| ends_where_starts(i_node, open))
      {
        continue;
      }

      if is_inline_comment(i_node) {
        self.sink.fill_separator(if is_in_paren_group {
          Separator::DedentedHardLine
        } else {
          Separator::HardLine
        });
        continue;
      }

      // The keywords of `@if`, `@each`, `@for`.
      if is_control_directive
        && (is_one_of_the_words(next_node, &[b"==", b"!="])
          || is_one_of_the_words(next_node, &[b"<", b">", b"<=", b">="])
          || is_one_of_the_words(next_node, &[b"and", b"or", b"not"])
          || is_the_word(i_node, b"in")
          || is_one_of_the_words(i_node, &[b"from", b"through", b"end"]))
      {
        self.sink.token(" ");
        continue;
      }

      if at_rule.is_some_and(|it| it.name.eq_ignore_ascii_case(b"namespace")) {
        self.sink.token(" ");
        continue;
      }

      if is_grid_value {
        if i_node.node().has_source()
          && next_node.node().has_source()
          && i_node.node().loc.start_line() != next_node.node().loc.start_line()
        {
          self.sink.fill_separator(Separator::HardLine);
          did_break = true;
        } else {
          self.sink.token(" ");
        }
        continue;
      }

      // `font: 12px/1.5 a`
      if declaration_prop.as_ref().is_some_and(|prop| **prop == *b"font" || prop.starts_with(b"--"))
      {
        if is_division(next_node)
          && has_empty_raw_before(next_node)
          && is_possible_font_size(Some(i_node))
        {
          continue;
        }
        if is_division(i_node) && has_empty_raw_before(i_node) && is_possible_font_size(prev_node) {
          continue;
        }
      }

      if is_next_math {
        self.sink.token(" ");
        continue;
      }
      // `function(returns-list($list)...)`
      if next_node.value() == Some(b"...") {
        continue;
      }
      if is_at_word_placeholder(i_node)
        && is_at_word_placeholder(next_node)
        && ends_where_starts(i_node, next_node)
      {
        continue;
      }
      if is_at_word_placeholder(i_node)
        && paren_group_open(next_node).is_some_and(|open| ends_where_starts(i_node, open))
      {
        self.sink.fill_separator(Separator::SoftLine);
        continue;
      }
      if i_node.value() == Some(b"with") && paren_group_open(next_node).is_some() {
        self.sink.nest_fill();
        self.sink.token(" ");
        continue;
      }
      // `--a#{(1) + 2}`
      if i_node.value().is_some_and(|it| it.ends_with(b"#")) && next_node.value() == Some(b"{") {
        match next_node.kind() {
          ValueKind::Func if paren_group_open(next_node.at(next_node.node().group)).is_some() => {
            continue;
          }
          ValueKind::Func => {}
          // Prettier assumes it's a function and throws.
          _ => self.fail(SyntaxError(Message::UnreadableValue, statement.node().start as u32)),
        }
      }

      // It's printed as a line suffix, which includes the space before it.
      if is_inline_comment(next_node) && next_next_node.is_none() {
        continue;
      }
      // The value lines up with the block comments before it.
      if at_rule.is_none()
        && is_comment(i_node)
        && !i_node.node().inline
        && node.groups().take(i).all(is_comment)
      {
        self.sink.fill_separator(Separator::DedentedLine);
        continue;
      }
      self.sink.fill_separator(Separator::Line);
    }

    if has_inline_comment {
      self.sink.break_parent();
    }
    if did_break {
      self.sink.start_fill_with_hard_line();
    }
    match shape {
      Shape::Indent => self.sink.end_fill_as_array(),
      Shape::Fill | Shape::IndentFill => self.sink.end_fill(),
    }
    if shape != Shape::Fill {
      self.sink.end_indent();
    }
    self.sink.end_group();
  }

  /// `printTrailingComma`: true if the source has a comma after the last item in the parentheses of `node`.
  fn has_comma_before_closing_parenthesis(&self, node: ValueRef<'_>) -> bool {
    let (Some(start), Some(end)) = (
      node.groups().next_back().and_then(|it| it.node().loc.start_offset()),
      Some(node.node().close)
        .filter(|&it| it != 0)
        .and_then(|it| node.at(it).node().loc.start_offset()),
    ) else {
      return false;
    };
    crate::strings::trim_js_whitespace_end(
      self.original_text().get(start as usize..end as usize).unwrap_or_default(),
    )
    .ends_with(b",")
  }

  /// `isSCSSMapItemNode` for `node`, whose ancestors are in `self.value_stack`.
  fn is_scss_map_item(&self, statement: Statement<'_, 'a>, node: ValueRef<'_>) -> bool {
    if self.syntax() != Syntax::Scss {
      return false;
    }
    if node.groups().len() == 0 {
      return false;
    }
    // `$key: (value)` isn't a list.
    if paren_group_open(node).is_some()
      && node.groups().len() == 1
      && node.group(0).is_some_and(|it| it.kind() != ValueKind::CommaGroup)
    {
      return false;
    }
    let parent = self.value_ancestor(statement, 0);
    if parent.is_some_and(|it| is_func(it) && it.value() == Some(b"if")) {
      return false;
    }
    let grandparent = self.value_ancestor(statement, 1);
    let is_in_pair = grandparent.is_some_and(is_key_value_pair_in_paren_group);
    if !is_key_value_pair_in_paren_group(node) && !is_in_pair {
      return false;
    }
    // `$map: (key: value, other-key: other-value)`
    if statement.css_ancestor(Kind::Decl).is_some_and(|it| it.prop.starts_with(b"$")) {
      return true;
    }
    // `$map: (key: (value other-value other-other-value))`
    if is_in_pair {
      return !parent.is_some_and(|it| it.groups().any(is_math_operator));
    }
    // `func((key: value, other-key: other-value))`
    grandparent.is_some_and(is_func)
  }

  /// `shouldBreakList` for `node`, whose ancestors are in `self.value_stack`.
  fn should_break_list(&self, statement: Statement<'_, 'a>, node: ValueRef<'_>) -> bool {
    super::printer::is_list_with_comma_group(node.values, node.id)
      && self.is_top_level_of_value(statement)
      && match statement.node().kind {
        Kind::Decl => !statement.node().prop.starts_with(b"--"),
        Kind::AtRule => statement.node().variable,
        _ => false,
      }
  }

  /// True if the node being printed is `root.group.group` of a value.
  fn is_top_level_of_value(&self, statement: Statement<'_, 'a>) -> bool {
    matches!(self.value_stack[..], [root, value]
            if statement.values.node(root).kind == ValueKind::Root && statement.values.node(value).kind == ValueKind::Value)
  }

  pub(crate) fn print_parenthesized_value_group(
    &mut self,
    statement: Statement<'_, 'a>,
    id: ValueId,
  ) {
    let node = ValueRef { values: statement.values, id };
    let (open, close) = (node.node().open, node.node().close);
    let parent = self.value_ancestor(statement, 0);
    let paren = |paren: ValueId| match paren {
      0 => "",
      paren if statement.values.value(paren) == Some(b"(") => "(",
      _ => ")",
    };
    let count = node.groups().len();
    let is_custom_property = statement.is_custom_property();

    let is_url = parent.is_some_and(|it| {
      is_func(it) && it.value().is_some_and(|value| value.eq_ignore_ascii_case(b"url"))
    });
    if is_url
      && (count == 1
        || node.group(0).is_some_and(|first| {
          first.kind() == ValueKind::CommaGroup
            && first.group(0).and_then(word).is_some_and(|it| it.starts_with(b"data:"))
        }))
    {
      self.sink.token(paren(open));
      for (index, child) in node.groups().enumerate() {
        if index > 0 {
          self.sink.token(",");
        }
        self.print_child_value(statement, id, child.id);
      }
      return self.sink.token(paren(close));
    }

    if open == 0 {
      let force_hard_line = self.should_break_list(statement, node);
      // `shouldPrecededBySoftline`
      let is_preceded_by_softline =
        self.is_top_level_of_value(statement) && statement.node().kind == Kind::Decl;
      self.sink.start_indent();
      match force_hard_line {
        true => self.sink.hard_line(),
        false => {
          self.sink.start_group(false);
          if is_preceded_by_softline {
            self.sink.soft_line();
          }
          self.sink.start_fill();
        }
      }
      for (index, child) in node.groups().enumerate() {
        if index > 0 {
          match force_hard_line {
            true => self.sink.hard_line(),
            false if is_custom_property && has_no_whitespace_before(child) => {}
            false => self.sink.fill_separator(Separator::Line),
          }
        }
        self.print_child_value(statement, id, child.id);
        if index + 1 < count {
          // Otherwise the backslash would escape the `,`.
          if ends_with_escaped_whitespace(child) {
            self.sink.token(" ");
          }
          self.sink.token(",");
        }
      }
      if !force_hard_line {
        self.sink.end_fill();
        self.sink.end_group();
      }
      return self.sink.end_indent();
    }

    let is_var = parent.is_some_and(|it| {
      is_func(it) && it.value().is_some_and(|value| value.eq_ignore_ascii_case(b"var"))
    });
    let is_scss_map_item = self.is_scss_map_item(statement, node);
    // The caller printing the enclosing comma group recorded this node's index. Searching for it instead would be
    // quadratic in a group with many parenthesized members.
    let hint = self.index_in_comma_group;
    let index_in_parent =
      parent.filter(|it| it.kind() == ValueKind::CommaGroup).and_then(|parent| {
        match parent.group(hint).is_some_and(|it| it.id == id) {
          true => Some(hint),
          false => parent.groups().position(|it| it.id == id),
        }
        .map(|index| (parent, index))
      });
    // `isKeyInValuePairNode`
    let is_key = index_in_parent.is_some_and(|(parent, index)| {
      is_key_value_pair(parent) && parent.group(index + 1).is_some_and(is_colon)
    });
    // `isConfigurationNode`: `@use "a" with (..)`
    let is_configuration = close != 0
      && node.groups().all(|it| it.kind() == ValueKind::CommaGroup)
      && index_in_parent.is_some_and(|(parent, index)| {
        parent.kind() == ValueKind::CommaGroup
          && index
            .checked_sub(1)
            .and_then(|at| parent.group(at))
            .is_some_and(|it| is_the_word(it, b"with"))
      });
    let should_break = is_configuration || (is_scss_map_item && !is_key);
    let should_dedent = is_configuration || is_key;

    if should_dedent {
      self.sink.start_dedent();
    }
    self.sink.start_group(should_break);
    self.sink.token(paren(open));
    self.sink.start_indent();
    self.sink.soft_line();
    for (index, child) in node.groups().enumerate() {
      let is_last = index + 1 == count;
      // Prettier turns `f(a,b)` into `f(⏎a,⏎b⏎)`.
      if index > 0 && !(is_custom_property && has_no_whitespace_before(child)) {
        self.sink.line();
      }
      // A key-value pair whose value is in parentheses is already indented.
      let is_dedented = child.kind() == ValueKind::CommaGroup
        && child.group(1).is_some_and(is_colon)
        && child.group(0).is_some_and(|it| it.kind() != ValueKind::ParenGroup)
        && child.group(2).is_some_and(|it| it.kind() == ValueKind::ParenGroup)
        && self.shape_of_comma_group(statement, child) == Shape::IndentFill;
      if is_dedented {
        self.sink.start_group(false);
        self.sink.start_dedent();
      }
      self.print_child_value(statement, id, child.id);
      // `(a\ )`, which Prettier prints as `(a\)`. A single word isn't in a comma group, which would otherwise
      // print the space.
      if ends_with_escaped_whitespace(child) {
        self.sink.token(" ");
      }
      if is_dedented {
        self.sink.end_indent();
        self.sink.end_group();
      }

      if !is_last {
        self.sink.token(",");
      } else {
        let is_only_comments = is_comment(child)
          || (child.kind() == ValueKind::CommaGroup && child.groups().all(is_comment));
        if is_var && self.has_comma_before_closing_parenthesis(node) {
          self.sink.token(",");
        } else if !is_only_comments && self.trailing_comma && is_scss_map_item {
          self.sink.if_break(",");
        }
      }

      if !is_last
        && child.kind() == ValueKind::CommaGroup
        && let Some(last) = child.groups().next_back()
        && let Some(end) = last.node().loc.end_offset().filter(|_| last.node().has_source())
      {
        // This can read text past the end of the declaration.
        self.is_memoizable = false;
        if is_next_line_empty(self.original_text(), end as usize) {
          self.sink.hard_line();
        }
      }
    }
    self.sink.end_indent();
    self.sink.soft_line();
    self.sink.line_suffix_boundary();
    self.sink.token(paren(close));
    self.sink.end_group();
    if should_dedent {
      self.sink.end_indent();
    }
  }
}
