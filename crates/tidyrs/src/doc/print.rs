//! Prettier's `printDocToString` (`src/document/printer`), step for step, over ranges of the stream.

use rustc_hash::FxHashMap;

use super::{Branch, Docs, Element, Line, Margin, NONE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Eol {
  #[default]
  Lf,
  Crlf,
  Cr,
}

impl Eol {
  fn text(self) -> &'static str {
    match self {
      Eol::Lf => "\n",
      Eol::Crlf => "\r\n",
      Eol::Cr => "\r",
    }
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
  pub width: usize,
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: Eol,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
  Break,
  Flat,
}

/// A command of Prettier's printer: the elements from `pos` to `end`, which are a concatenation of docs. A fill's
/// command walks its parts from `pos`, as Prettier's `DOC_FILL_PRINTED_LENGTH` does.
#[derive(Clone, Copy)]
struct Command {
  indent: u32,
  mode: Mode,
  pos: u32,
  end: u32,
  fill: bool,
}

impl Command {
  fn new(indent: u32, mode: Mode, pos: u32, end: u32) -> Self {
    Self { indent, mode, pos, end, fill: false }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Step<'a> {
  Indent,
  Dedent,
  Width(usize),
  Text(&'a str),
}

struct Indent<'a> {
  value: String,
  length: usize,
  queue: Vec<Step<'a>>,
  root: u32,
  /// `makeIndent` of this indent, once made.
  indented: Option<u32>,
}

/// Every indent of one run, with `makeIndent` and `makeAlign` memoized.
struct Indents<'a> {
  list: Vec<Indent<'a>>,
  made: FxHashMap<(u32, Step<'a>), u32>,
  tabs: bool,
  tab_width: usize,
}

const ROOT: u32 = 0;

impl<'a> Indents<'a> {
  fn new(options: &Options) -> Self {
    let root =
      Indent { value: String::new(), length: 0, queue: Vec::new(), root: ROOT, indented: None };
    Self {
      list: vec![root],
      made: FxHashMap::default(),
      tabs: options.tabs,
      tab_width: options.tab_width,
    }
  }

  fn get(&self, indent: u32) -> &Indent<'a> {
    &self.list[indent as usize]
  }

  fn align(&mut self, indent: u32, margin: Margin, docs: &Docs<'a>) -> u32 {
    match margin {
      Margin::Width(0) => indent,
      Margin::Mark => {
        let parent = self.get(indent);
        let marked = Indent {
          value: parent.value.clone(),
          length: parent.length,
          queue: parent.queue.clone(),
          root: indent,
          indented: None,
        };
        self.list.push(marked);
        self.list.len() as u32 - 1
      }
      Margin::Root => self.get(indent).root,
      Margin::Dedent => self.make(indent, Step::Dedent),
      Margin::Width(width) => self.make(indent, Step::Width(width as usize)),
      Margin::Text(index) => self.make(indent, Step::Text(docs.text_at(index).0)),
    }
  }

  /// `generateIndent`.
  fn make(&mut self, indent: u32, step: Step<'a>) -> u32 {
    if step == Step::Indent {
      if let Some(made) = self.list[indent as usize].indented {
        return made;
      }
    } else if let Some(&made) = self.made.get(&(indent, step)) {
      return made;
    }
    let parent = self.get(indent);
    let mut queue = parent.queue.clone();
    if step == Step::Dedent {
      queue.pop();
    } else {
      queue.push(step);
    }
    let (mut value, mut length) = (String::new(), 0);
    let (mut tabs, mut spaces) = (0, 0);
    let tab_width = self.tab_width;
    let flush = |value: &mut String,
                 length: &mut usize,
                 tabs: &mut usize,
                 spaces: &mut usize,
                 use_tabs: bool| {
      if use_tabs {
        if *tabs > 0 {
          value.extend(std::iter::repeat_n('\t', *tabs));
          *length += tab_width * *tabs;
        }
      } else if *spaces > 0 {
        value.extend(std::iter::repeat_n(' ', *spaces));
        *length += *spaces;
      }
      *tabs = 0;
      *spaces = 0;
    };
    for step in &queue {
      match *step {
        Step::Indent => {
          flush(&mut value, &mut length, &mut tabs, &mut spaces, self.tabs);
          if self.tabs {
            value.push('\t');
            length += tab_width;
          } else {
            value.extend(std::iter::repeat_n(' ', tab_width));
            length += tab_width;
          }
        }
        Step::Text(text) => {
          flush(&mut value, &mut length, &mut tabs, &mut spaces, self.tabs);
          value.push_str(text);
          length += text.len();
        }
        Step::Width(width) => {
          tabs += 1;
          spaces += width;
        }
        Step::Dedent => unreachable!(),
      }
    }
    flush(&mut value, &mut length, &mut tabs, &mut spaces, false);
    let root = parent.root;
    self.list.push(Indent { value, length, queue, root, indented: None });
    let made = self.list.len() as u32 - 1;
    if step == Step::Indent {
      self.list[indent as usize].indented = Some(made);
    } else {
      self.made.insert((indent, step), made);
    }
    made
  }
}

/// `PrintResult`: trims only what was written since the last trim.
struct Output {
  text: String,
  settled: usize,
}

impl Output {
  fn trim(&mut self) -> usize {
    let kept = self.text[self.settled..].trim_end_matches([' ', '\t']).len() + self.settled;
    let count = self.text.len() - kept;
    self.text.truncate(kept);
    self.settled = kept;
    count
  }
}

struct Modes(Vec<Option<Mode>>);

impl Modes {
  fn get(&self, id: u32) -> Option<Mode> {
    self.0.get(id as usize).copied().flatten()
  }

  fn set(&mut self, id: u32, mode: Mode) {
    let index = id as usize;
    if self.0.len() <= index {
      self.0.resize(index + 1, None);
    }
    self.0[index] = Some(mode);
  }
}

/// The contents of the container at `at`, which ends at `end`.
fn inside(at: u32, end: u32) -> (u32, u32) {
  (at + 1, end)
}

/// Prints the stream, breaking its groups to fit in `options.width` columns.
#[inline(never)]
pub fn print(docs: &Docs<'_>, options: &Options) -> String {
  let eol = options.eol.text();
  let mut indents = Indents::new(options);
  let mut modes = Modes(Vec::new());
  let mut output = Output { text: String::with_capacity(docs.len() as usize * 4), settled: 0 };
  let mut position = 0isize;
  let mut commands = vec![Command::new(ROOT, Mode::Break, 1, docs.len())];
  let mut remeasure = false;
  let mut suffix: Vec<Command> = Vec::new();
  let mut scratch: Vec<Frame> = Vec::new();

  'commands: while let Some(mut command) = commands.pop() {
    if command.fill {
      let Command { indent, mode, pos, end, .. } = command;
      if pos >= end {
        continue;
      }
      let remaining = options.width as isize - position;
      let pending = !suffix.is_empty();
      let Element::Part { end: after } = docs.element(pos) else { unreachable!() };
      let (start, stop) = inside(pos, after);
      let flat = Command::new(indent, Mode::Flat, start, stop);
      let broken = Command::new(indent, Mode::Break, start, stop);
      let content_fits = match plain(docs, start, stop) {
        Some(size) => remaining >= size as isize,
        None => fits(docs, &mut scratch, &[flat], &[], remaining, pending, &modes, true),
      };
      if after >= end {
        commands.push(if content_fits { flat } else { broken });
        continue;
      }
      let Element::Part { end: next } = docs.element(after) else { unreachable!() };
      let (space_start, space_stop) = inside(after, next);
      let space_flat = Command::new(indent, Mode::Flat, space_start, space_stop);
      let space_broken = Command::new(indent, Mode::Break, space_start, space_stop);
      if next >= end {
        if content_fits {
          commands.extend([space_flat, flat]);
        } else {
          commands.extend([space_broken, broken]);
        }
        continue;
      }
      let Element::Part { end: last } = docs.element(next) else { unreachable!() };
      let (second_start, second_stop) = inside(next, last);
      commands.push(Command { indent, mode, pos: next, end, fill: true });
      let second = Command::new(indent, Mode::Flat, second_start, second_stop);
      let pair_fits = match (
        plain(docs, start, stop),
        line_of(docs, space_start, space_stop),
        text_of(docs, second_start, second_stop),
      ) {
        (Some(first), Some(line @ (Line::Space | Line::Soft)), Some((empty, size))) => {
          let space = usize::from(line == Line::Space && !empty);
          remaining >= (first as usize + space + size as usize) as isize
        }
        _ => fits(
          docs,
          &mut scratch,
          &[second, space_flat, flat],
          &[],
          remaining,
          pending,
          &modes,
          true,
        ),
      };
      if pair_fits {
        commands.extend([space_flat, flat]);
      } else if content_fits {
        commands.extend([space_broken, flat]);
      } else {
        commands.extend([space_broken, broken]);
      }
      continue;
    }

    while command.pos < command.end {
      let Command { indent, mode, pos, .. } = command;
      let element = docs.element(pos);
      command.pos += 1;
      match element {
        Element::Text(index) => {
          let (text, size) = docs.text_at(index);
          if eol != "\n" && text.contains('\n') {
            output.text.push_str(&text.replace('\n', eol));
          } else {
            output.text.push_str(text);
          }
          position += size as isize;
        }
        Element::Trim => position -= output.trim() as isize,
        Element::LineSuffixBoundary => {
          if !suffix.is_empty() {
            commands.push(command);
            commands.push(Command::new(indent, mode, Docs::HARD, Docs::HARD + 1));
            continue 'commands;
          }
        }
        Element::Line(line) => {
          if mode == Mode::Flat && matches!(line, Line::Space | Line::Soft) {
            if line == Line::Space {
              output.text.push(' ');
              position += 1;
            }
            continue;
          }
          if mode == Mode::Flat {
            remeasure = true;
          }
          if !suffix.is_empty() {
            command.pos -= 1;
            commands.push(command);
            commands.extend(suffix.drain(..).rev());
            continue 'commands;
          }
          if line == Line::Literal {
            output.text.push_str(eol);
            let root = indents.get(indents.get(indent).root);
            output.text.push_str(&root.value);
            position = root.length as isize;
          } else {
            output.trim();
            let value = &indents.get(indent).value;
            output.text.push_str(eol);
            output.text.push_str(value);
            position = indents.get(indent).length as isize;
          }
        }
        Element::Group { end, id, broken, states } => {
          command.pos = end;
          if command.pos < command.end {
            commands.push(command);
          }
          let (start, stop) = if states { first_state(docs, pos, end) } else { inside(pos, end) };
          let next = if mode == Mode::Flat && !remeasure {
            Command::new(indent, if broken { Mode::Break } else { Mode::Flat }, start, stop)
          } else {
            remeasure = false;
            let remaining = options.width as isize - position;
            let pending = !suffix.is_empty();
            let flat = Command::new(indent, Mode::Flat, start, stop);
            if !broken
              && fits(docs, &mut scratch, &[flat], &commands, remaining, pending, &modes, false)
            {
              flat
            } else if states {
              let all = states_of(docs, pos, end);
              let (last_start, last_stop) = all[all.len() - 1];
              let mut chosen = Command::new(indent, Mode::Break, last_start, last_stop);
              if !broken {
                for &(state_start, state_stop) in &all[1..all.len() - 1] {
                  let flat = Command::new(indent, Mode::Flat, state_start, state_stop);
                  if fits(docs, &mut scratch, &[flat], &commands, remaining, pending, &modes, false)
                  {
                    chosen = flat;
                    break;
                  }
                }
              }
              chosen
            } else {
              Command::new(indent, Mode::Break, start, stop)
            }
          };
          if id != NONE {
            modes.set(id, next.mode);
          }
          commands.push(next);
          continue 'commands;
        }
        Element::Fill { end } => {
          command.pos = end;
          if command.pos < command.end {
            commands.push(command);
          }
          commands.push(Command { indent, mode, pos: pos + 1, end, fill: true });
          continue 'commands;
        }
        Element::Part { end } => {
          // A part outside of a fill's walk, as in a state that is printed: its contents.
          command.pos = end;
          if command.pos < command.end {
            commands.push(command);
          }
          commands.push(Command::new(indent, mode, pos + 1, end));
          continue 'commands;
        }
        Element::Indent { end } => {
          command.pos = end;
          if command.pos < command.end {
            commands.push(command);
          }
          let indent = indents.make(indent, Step::Indent);
          commands.push(Command::new(indent, mode, pos + 1, end));
          continue 'commands;
        }
        Element::Align { end, margin } => {
          command.pos = end;
          if command.pos < command.end {
            commands.push(command);
          }
          let indent = indents.align(indent, margin, docs);
          commands.push(Command::new(indent, mode, pos + 1, end));
          continue 'commands;
        }
        Element::IfBreak { end, id, branch } => {
          let group = if id == NONE { Some(mode) } else { modes.get(id) };
          let wanted = match branch {
            Branch::Break => Mode::Break,
            Branch::Flat => Mode::Flat,
          };
          if group != Some(wanted) || pos + 1 == end {
            command.pos = end;
            continue;
          }
          command.pos = end;
          if command.pos < command.end {
            commands.push(command);
          }
          commands.push(Command::new(indent, mode, pos + 1, end));
          continue 'commands;
        }
        Element::IndentIfBreak { end, id, negate } => {
          command.pos = end;
          let indent = match modes.get(id) {
            Some(Mode::Break) if negate => indent,
            Some(Mode::Break) => indents.make(indent, Step::Indent),
            Some(Mode::Flat) if negate => indents.make(indent, Step::Indent),
            Some(Mode::Flat) => indent,
            None => continue,
          };
          if command.pos < command.end {
            commands.push(command);
          }
          commands.push(Command::new(indent, mode, pos + 1, end));
          continue 'commands;
        }
        Element::LineSuffix { end } => {
          command.pos = end;
          suffix.push(Command::new(indent, mode, pos + 1, end));
        }
      }
    }
    if commands.is_empty() && !suffix.is_empty() {
      commands.extend(suffix.drain(..).rev());
    }
  }
  output.text
}

/// The width of the range when it is one text, or nothing.
fn plain(docs: &Docs<'_>, start: u32, stop: u32) -> Option<u32> {
  text_of(docs, start, stop).map(|(_, size)| size)
}

/// Whether the range is one text, empty or not, and its width.
fn text_of(docs: &Docs<'_>, start: u32, stop: u32) -> Option<(bool, u32)> {
  match stop - start {
    0 => Some((true, 0)),
    1 => match docs.element(start) {
      Element::Text(index) => {
        let (text, size) = docs.text_at(index);
        Some((text.is_empty(), size))
      }
      _ => None,
    },
    _ => None,
  }
}

/// The line the range is, if it is one.
fn line_of(docs: &Docs<'_>, start: u32, stop: u32) -> Option<Line> {
  match (stop - start, docs.element(start)) {
    (1, Element::Line(line)) => Some(line),
    _ => None,
  }
}

/// The contents of a conditional group's first state.
fn first_state(docs: &Docs<'_>, at: u32, end: u32) -> (u32, u32) {
  states_of(docs, at, end)[0]
}

/// The contents of each state of the conditional group at `at`.
fn states_of(docs: &Docs<'_>, at: u32, end: u32) -> Vec<(u32, u32)> {
  let mut states = Vec::new();
  let mut pos = at + 1;
  while pos < end {
    let Element::Part { end: stop } = docs.element(pos) else { unreachable!() };
    states.push((pos + 1, stop));
    pos = stop;
  }
  states
}

/// A command of `fits`: its mode, and the range of elements left.
type Frame = (Mode, u32, u32);

/// `fits`: whether `next`, which are commands in reverse order, and then `rest` from its end, fit on the line in
/// `remaining` columns.
#[allow(clippy::too_many_arguments)]
fn fits(
  docs: &Docs<'_>,
  frames: &mut Vec<Frame>,
  next: &[Command],
  rest: &[Command],
  mut remaining: isize,
  mut pending: bool,
  modes: &Modes,
  must_be_flat: bool,
) -> bool {
  let mut rest_index = rest.len();
  let mut space = false;
  let mut trailing = 0usize;
  frames.clear();
  frames.extend(next.iter().map(|command| (command.mode, command.pos, command.end)));
  while remaining >= 0 {
    let Some(frame) = frames.last_mut() else {
      if rest_index == 0 {
        return true;
      }
      rest_index -= 1;
      let command = rest[rest_index];
      frames.push((command.mode, command.pos, command.end));
      continue;
    };
    let (mode, pos, end) = *frame;
    if pos >= end {
      frames.pop();
      continue;
    }
    frame.1 += 1;
    match docs.element(pos) {
      Element::Text(index) => {
        let (text, size) = docs.text_at(index);
        if !text.is_empty() {
          if space {
            remaining -= 1;
            space = false;
            trailing += 1;
          }
          remaining -= size as isize;
          if docs.trims {
            let kept = text.trim_end_matches([' ', '\t']).len();
            trailing = if kept == 0 { trailing + text.len() } else { text.len() - kept };
          }
        }
      }
      Element::Trim => {
        remaining += trailing as isize;
        trailing = 0;
      }
      Element::Line(line) => {
        if mode == Mode::Break || matches!(line, Line::Hard | Line::Literal) {
          return true;
        }
        if line == Line::Space {
          space = true;
        }
      }
      Element::LineSuffixBoundary => {
        if pending {
          return false;
        }
      }
      Element::LineSuffix { end } => {
        frame.1 = end;
        pending = true;
      }
      Element::Group { end, broken, states, .. } => {
        frame.1 = end;
        if must_be_flat && broken {
          return false;
        }
        let mode = if broken { Mode::Break } else { mode };
        let (start, stop) = if !states {
          inside(pos, end)
        } else if mode == Mode::Break {
          *states_of(docs, pos, end).last().unwrap()
        } else {
          first_state(docs, pos, end)
        };
        frames.push((mode, start, stop));
      }
      Element::IfBreak { end, id, branch } => {
        frame.1 = end;
        let group = if id == NONE { mode } else { modes.get(id).unwrap_or(Mode::Flat) };
        let wanted = match branch {
          Branch::Break => Mode::Break,
          Branch::Flat => Mode::Flat,
        };
        if group == wanted {
          frames.push((mode, pos + 1, end));
        }
      }
      Element::Fill { end }
      | Element::Part { end }
      | Element::Indent { end }
      | Element::Align { end, .. }
      | Element::IndentIfBreak { end, .. } => {
        frame.1 = end;
        frames.push((mode, pos + 1, end));
      }
    }
  }
  false
}
