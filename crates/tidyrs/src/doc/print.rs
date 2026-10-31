//! Prettier's `printDocToString` (`src/document/printer`), step for step.

use std::collections::HashMap;

use super::{Align, Doc, Docs, GroupId, Line, Node, width};

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

#[derive(Clone, Copy)]
struct Command {
  indent: u32,
  mode: Mode,
  doc: Doc,
  /// `DOC_FILL_PRINTED_LENGTH`: the parts of a fill already printed.
  offset: u32,
}

impl Command {
  fn new(indent: u32, mode: Mode, doc: Doc) -> Self {
    Self { indent, mode, doc, offset: 0 }
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
}

/// Every indent of one run, with `makeIndent` and `makeAlign` memoized.
struct Indents<'a> {
  list: Vec<Indent<'a>>,
  made: HashMap<(u32, Step<'a>), u32>,
  tabs: bool,
  tab_width: usize,
}

const ROOT: u32 = 0;

impl<'a> Indents<'a> {
  fn new(options: &Options) -> Self {
    let root = Indent { value: String::new(), length: 0, queue: Vec::new(), root: ROOT };
    Self {
      list: vec![root],
      made: HashMap::new(),
      tabs: options.tabs,
      tab_width: options.tab_width,
    }
  }

  fn get(&self, indent: u32) -> &Indent<'a> {
    &self.list[indent as usize]
  }

  fn align(&mut self, indent: u32, align: Align<'a>) -> u32 {
    match align {
      Align::Width(0) => indent,
      Align::Mark => {
        let parent = self.get(indent);
        let marked = Indent {
          value: parent.value.clone(),
          length: parent.length,
          queue: parent.queue.clone(),
          root: indent,
        };
        self.list.push(marked);
        self.list.len() as u32 - 1
      }
      Align::Root => self.get(indent).root,
      Align::Dedent => self.make(indent, Step::Dedent),
      Align::Width(width) => self.make(indent, Step::Width(width)),
      Align::Text(text) => self.make(indent, Step::Text(text)),
    }
  }

  /// `generateIndent`.
  fn make(&mut self, indent: u32, step: Step<'a>) -> u32 {
    if let Some(&made) = self.made.get(&(indent, step)) {
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
    self.list.push(Indent { value, length, queue, root });
    let made = self.list.len() as u32 - 1;
    self.made.insert((indent, step), made);
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
  fn get(&self, id: GroupId) -> Option<Mode> {
    self.0.get(id.0 as usize).copied().flatten()
  }

  fn set(&mut self, id: GroupId, mode: Mode) {
    let index = id.0 as usize;
    if self.0.len() <= index {
      self.0.resize(index + 1, None);
    }
    self.0[index] = Some(mode);
  }
}

/// Prints `doc`, breaking its groups to fit in `options.width` columns.
pub fn print<'a>(docs: &mut Docs<'a>, doc: Doc, options: &Options) -> String {
  docs.propagate(doc);
  let docs = &*docs;
  let eol = options.eol.text();
  let mut indents = Indents::new(options);
  let mut modes = Modes(Vec::new());
  let mut output = Output { text: String::new(), settled: 0 };
  let mut position = 0isize;
  let mut commands = vec![Command::new(ROOT, Mode::Break, doc)];
  let mut remeasure = false;
  let mut suffix: Vec<Command> = Vec::new();

  while let Some(command) = commands.pop() {
    let Command { indent, mode, doc, offset } = command;
    match docs.node(doc) {
      Node::Text(text) => {
        if !text.is_empty() {
          if eol != "\n" && text.contains('\n') {
            output.text.push_str(&text.replace('\n', eol));
          } else {
            output.text.push_str(text);
          }
          if !commands.is_empty() {
            position += width(text) as isize;
          }
        }
      }
      Node::Concat(list) => {
        for &part in docs.parts(list).iter().rev() {
          commands.push(Command::new(indent, mode, part));
        }
      }
      Node::Indent(contents) => {
        let indent = indents.make(indent, Step::Indent);
        commands.push(Command::new(indent, mode, contents));
      }
      Node::Align(align, contents) => {
        let indent = indents.align(indent, align);
        commands.push(Command::new(indent, mode, contents));
      }
      Node::Trim => position -= output.trim() as isize,
      Node::Group { contents, id, broken, states } => {
        let next = if mode == Mode::Flat && !remeasure {
          Command::new(indent, if broken { Mode::Break } else { Mode::Flat }, contents)
        } else {
          remeasure = false;
          let remaining = options.width as isize - position;
          let pending = !suffix.is_empty();
          let flat = Command::new(indent, Mode::Flat, contents);
          if !broken && fits(docs, flat, &commands, remaining, pending, &modes, false) {
            flat
          } else if let Some(list) = states {
            let states = docs.parts(list);
            let last = states[states.len() - 1];
            let mut chosen = Command::new(indent, Mode::Break, last);
            if !broken {
              for &state in &states[1..states.len() - 1] {
                let flat = Command::new(indent, Mode::Flat, state);
                if fits(docs, flat, &commands, remaining, pending, &modes, false) {
                  chosen = flat;
                  break;
                }
              }
            }
            chosen
          } else {
            Command::new(indent, Mode::Break, contents)
          }
        };
        commands.push(next);
        if let Some(id) = id {
          modes.set(id, next.mode);
        }
      }
      Node::Fill(list) => {
        let remaining = options.width as isize - position;
        let parts = docs.parts(list);
        let length = parts.len() - offset as usize;
        if length == 0 {
          continue;
        }
        let content = parts[offset as usize];
        let flat = Command::new(indent, Mode::Flat, content);
        let broken = Command::new(indent, Mode::Break, content);
        let pending = !suffix.is_empty();
        let content_fits = fits(docs, flat, &[], remaining, pending, &modes, true);
        if length == 1 {
          commands.push(if content_fits { flat } else { broken });
          continue;
        }
        let whitespace = parts[offset as usize + 1];
        let space_flat = Command::new(indent, Mode::Flat, whitespace);
        let space_broken = Command::new(indent, Mode::Break, whitespace);
        if length == 2 {
          if content_fits {
            commands.extend([space_flat, flat]);
          } else {
            commands.extend([space_broken, broken]);
          }
          continue;
        }
        let second = parts[offset as usize + 2];
        commands.push(Command { indent, mode, doc, offset: offset + 2 });
        let both = Pair { first: content, space: whitespace, second };
        if fits_pair(docs, indent, both, remaining, pending, &modes) {
          commands.extend([space_flat, flat]);
        } else if content_fits {
          commands.extend([space_broken, flat]);
        } else {
          commands.extend([space_broken, broken]);
        }
      }
      Node::IfBreak { broken, flat, id } => {
        let group = match id {
          Some(id) => modes.get(id),
          None => Some(mode),
        };
        match group {
          Some(Mode::Break) if broken != Docs::EMPTY => {
            commands.push(Command::new(indent, mode, broken))
          }
          Some(Mode::Flat) if flat != Docs::EMPTY => {
            commands.push(Command::new(indent, mode, flat))
          }
          _ => {}
        }
      }
      Node::IndentIfBreak { contents, id, negate } => match modes.get(id) {
        Some(Mode::Break) => {
          let indent = if negate { indent } else { indents.make(indent, Step::Indent) };
          commands.push(Command::new(indent, mode, contents));
        }
        Some(Mode::Flat) => {
          let indent = if negate { indents.make(indent, Step::Indent) } else { indent };
          commands.push(Command::new(indent, mode, contents));
        }
        None => {}
      },
      Node::LineSuffix(contents) => suffix.push(Command::new(indent, mode, contents)),
      Node::LineSuffixBoundary => {
        if !suffix.is_empty() {
          commands.push(Command::new(indent, mode, Docs::HARD));
        }
      }
      Node::Line(line) => {
        if mode == Mode::Flat && matches!(line, Line::Space | Line::Soft) {
          if line == Line::Space {
            output.text.push(' ');
            position += 1;
          }
        } else {
          if mode == Mode::Flat {
            remeasure = true;
          }
          if !suffix.is_empty() {
            commands.push(command);
            commands.extend(suffix.drain(..).rev());
          } else if line == Line::Literal {
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
      }
      Node::Label(_, contents) => commands.push(Command::new(indent, mode, contents)),
      Node::BreakParent => {}
    }
    if commands.is_empty() && !suffix.is_empty() {
      commands.extend(suffix.drain(..).rev());
    }
  }
  output.text
}

#[derive(Clone, Copy)]
struct Pair {
  first: Doc,
  space: Doc,
  second: Doc,
}

/// `fits` of `[content, whitespace, secondContent]` in flat mode, as a fill measures it.
fn fits_pair(
  docs: &Docs,
  indent: u32,
  pair: Pair,
  remaining: isize,
  pending: bool,
  modes: &Modes,
) -> bool {
  let rest =
    [Command::new(indent, Mode::Flat, pair.second), Command::new(indent, Mode::Flat, pair.space)];
  let first = Command::new(indent, Mode::Flat, pair.first);
  fits_with(docs, first, &rest, &[], remaining, pending, modes, true)
}

/// `fits`: whether `next`, then `rest` from its end, fit on the line in `remaining` columns.
fn fits(
  docs: &Docs,
  next: Command,
  rest: &[Command],
  remaining: isize,
  pending: bool,
  modes: &Modes,
  must_be_flat: bool,
) -> bool {
  fits_with(docs, next, &[], rest, remaining, pending, modes, must_be_flat)
}

#[allow(clippy::too_many_arguments)]
fn fits_with(
  docs: &Docs,
  next: Command,
  queued: &[Command],
  rest: &[Command],
  mut remaining: isize,
  mut pending: bool,
  modes: &Modes,
  must_be_flat: bool,
) -> bool {
  let mut rest_index = rest.len();
  let mut space = false;
  let mut trailing = 0usize;
  let mut commands: Vec<(Mode, Doc, u32)> =
    queued.iter().map(|c| (c.mode, c.doc, c.offset)).collect();
  commands.push((next.mode, next.doc, next.offset));
  while remaining >= 0 {
    let Some((mode, doc, offset)) = commands.pop() else {
      if rest_index == 0 {
        return true;
      }
      rest_index -= 1;
      let command = rest[rest_index];
      commands.push((command.mode, command.doc, command.offset));
      continue;
    };
    match docs.node(doc) {
      Node::Text(text) => {
        if !text.is_empty() {
          if space {
            remaining -= 1;
            space = false;
            trailing += 1;
          }
          remaining -= width(text) as isize;
          let kept = text.trim_end_matches([' ', '\t']).len();
          trailing = if kept == 0 { trailing + text.len() } else { text.len() - kept };
        }
      }
      Node::Concat(list) => {
        for &part in docs.parts(list).iter().rev() {
          commands.push((mode, part, 0));
        }
      }
      Node::Fill(list) => {
        for &part in docs.parts(list)[offset as usize..].iter().rev() {
          commands.push((mode, part, 0));
        }
      }
      Node::Indent(contents)
      | Node::Align(_, contents)
      | Node::IndentIfBreak { contents, .. }
      | Node::Label(_, contents) => commands.push((mode, contents, 0)),
      Node::Trim => {
        remaining += trailing as isize;
        trailing = 0;
      }
      Node::Group { contents, broken, states, .. } => {
        if must_be_flat && broken {
          return false;
        }
        let mode = if broken { Mode::Break } else { mode };
        let contents = match states {
          Some(list) if mode == Mode::Break => *docs.parts(list).last().unwrap(),
          _ => contents,
        };
        commands.push((mode, contents, 0));
      }
      Node::IfBreak { broken, flat, id } => {
        let group = match id {
          Some(id) => modes.get(id).unwrap_or(Mode::Flat),
          None => mode,
        };
        let contents = if group == Mode::Break { broken } else { flat };
        if contents != Docs::EMPTY {
          commands.push((mode, contents, 0));
        }
      }
      Node::Line(line) => {
        if mode == Mode::Break || matches!(line, Line::Hard | Line::Literal) {
          return true;
        }
        if line == Line::Space {
          space = true;
        }
      }
      Node::LineSuffix(_) => pending = true,
      Node::LineSuffixBoundary => {
        if pending {
          return false;
        }
      }
      Node::BreakParent => {}
    }
  }
  false
}
