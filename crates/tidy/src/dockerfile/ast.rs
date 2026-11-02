//! The tree of a Dockerfile, as dprint-plugin-dockerfile parses it. Strings borrow from the text, unless the parser
//! resolved escapes or quotes in them.

use std::borrow::Cow;

/// A byte range of the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
  pub(crate) start: usize,
  pub(crate) end: usize,
}

impl Span {
  pub(crate) fn new(start: usize, end: usize) -> Span {
    Span { start, end }
  }
}

/// A parsed Dockerfile.
pub(crate) struct Dockerfile<'a> {
  pub(crate) content: &'a str,
  pub(crate) instructions: Vec<Instruction<'a>>,
  /// The line continuation and escape character: `\`, or `` ` `` with a `# escape=` directive.
  pub(crate) escape: char,
  /// Where each line starts, from 0.
  line_starts: Vec<usize>,
}

impl<'a> Dockerfile<'a> {
  pub(crate) fn new(
    content: &'a str,
    instructions: Vec<Instruction<'a>>,
    escape: char,
  ) -> Dockerfile<'a> {
    let mut line_starts = vec![0];
    line_starts.extend(memchr::memchr_iter(b'\n', content.as_bytes()).map(|at| at + 1));
    Dockerfile { content, instructions, escape, line_starts }
  }

  /// The line that `at` is on, from 0.
  pub(crate) fn line_index(&self, at: usize) -> usize {
    match self.line_starts.binary_search(&at) {
      Ok(index) => index,
      Err(index) => index - 1,
    }
  }

  pub(crate) fn line_start(&self, line: usize) -> usize {
    self.line_starts[line]
  }
}

pub(crate) enum Instruction<'a> {
  From(FromInstruction<'a>),
  Arg(ArgInstruction<'a>),
  Label(LabelInstruction<'a>),
  Run(ExprInstruction<'a>),
  Entrypoint(ExprInstruction<'a>),
  Cmd(ExprInstruction<'a>),
  Copy(CopyInstruction<'a>),
  Env(EnvInstruction<'a>),
  Shell(ExprInstruction<'a>),
  Onbuild(OnbuildInstruction<'a>),
  Healthcheck(HealthcheckInstruction<'a>),
  Heredoc(HeredocInstruction<'a>),
  Misc(MiscInstruction<'a>),
  /// A line that doesn't parse as its instruction, kept as it is so that one bad line doesn't stop the file.
  Unknown(SpannedString<'a>),
}

impl Instruction<'_> {
  pub(crate) fn span(&self) -> Span {
    match self {
      Instruction::From(it) => it.span,
      Instruction::Arg(it) => it.span,
      Instruction::Label(it) => it.span,
      Instruction::Run(it) | Instruction::Entrypoint(it) | Instruction::Cmd(it) => it.span,
      Instruction::Shell(it) => it.span,
      Instruction::Copy(it) => it.span,
      Instruction::Env(it) => it.span,
      Instruction::Onbuild(it) => it.span,
      Instruction::Healthcheck(it) => it.span,
      Instruction::Heredoc(it) => it.span,
      Instruction::Misc(it) => it.span,
      Instruction::Unknown(it) => it.span,
    }
  }
}

pub(crate) struct SpannedString<'a> {
  pub(crate) span: Span,
  pub(crate) content: Cow<'a, str>,
}

pub(crate) struct SpannedComment<'a> {
  pub(crate) span: Span,
  pub(crate) content: &'a str,
}

/// `["executable", "param"]`
pub(crate) struct StringArray<'a> {
  pub(crate) span: Span,
  pub(crate) elements: Vec<SpannedString<'a>>,
}

pub(crate) enum Component<'a> {
  String(SpannedString<'a>),
  Comment(SpannedComment<'a>),
}

impl Component<'_> {
  pub(crate) fn end(&self) -> usize {
    match self {
      Component::String(string) => string.span.end,
      Component::Comment(comment) => comment.span.end,
    }
  }
}

/// A string over lines that end in a line continuation, with comment lines among them.
pub(crate) struct BreakableString<'a> {
  pub(crate) span: Span,
  pub(crate) components: Vec<Component<'a>>,
}

impl<'a> BreakableString<'a> {
  pub(crate) fn of(string: SpannedString<'a>) -> BreakableString<'a> {
    BreakableString { span: string.span, components: vec![Component::String(string)] }
  }
}

/// The shell form, or the exec form: `["a", "b"]`.
pub(crate) enum Expr<'a> {
  Shell(BreakableString<'a>),
  Exec(StringArray<'a>),
}

/// `RUN`, `CMD`, `ENTRYPOINT` and `SHELL`.
pub(crate) struct ExprInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) expr: Expr<'a>,
}

pub(crate) struct FromInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) flags: Vec<Flag<'a>>,
  pub(crate) image: SpannedString<'a>,
  pub(crate) alias: Option<SpannedString<'a>>,
}

/// `--name=value`
pub(crate) struct Flag<'a> {
  pub(crate) span: Span,
  pub(crate) name: SpannedString<'a>,
  pub(crate) value: SpannedString<'a>,
}

pub(crate) struct ArgInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) name: SpannedString<'a>,
  pub(crate) value: Option<SpannedString<'a>>,
}

pub(crate) struct LabelInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) labels: Vec<Label<'a>>,
}

pub(crate) struct Label<'a> {
  pub(crate) span: Span,
  pub(crate) name: SpannedString<'a>,
  pub(crate) value: SpannedString<'a>,
}

pub(crate) struct CopyInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) flags: Vec<Flag<'a>>,
  pub(crate) args: CopyArgs<'a>,
}

pub(crate) enum CopyArgs<'a> {
  Paths { sources: Vec<SpannedString<'a>>, destination: SpannedString<'a> },
  Exec(StringArray<'a>),
}

pub(crate) struct EnvInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) vars: Vec<EnvVar<'a>>,
}

pub(crate) struct EnvVar<'a> {
  pub(crate) span: Span,
  pub(crate) key: SpannedString<'a>,
  pub(crate) value: BreakableString<'a>,
}

pub(crate) struct OnbuildInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) instruction: Box<Instruction<'a>>,
}

/// `HEALTHCHECK [OPTIONS] CMD command`, or `HEALTHCHECK NONE` without `cmd`.
pub(crate) struct HealthcheckInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) flags: Vec<Flag<'a>>,
  pub(crate) cmd: Option<Box<Instruction<'a>>>,
}

/// An instruction with heredocs: the instruction on its first line, then the bodies and their closing delimiters as
/// they are written.
pub(crate) struct HeredocInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) instruction: Box<Instruction<'a>>,
  pub(crate) body: &'a str,
}

/// An instruction without its own grammar, such as `EXPOSE`, `WORKDIR` or `USER`.
pub(crate) struct MiscInstruction<'a> {
  pub(crate) span: Span,
  pub(crate) instruction: SpannedString<'a>,
  pub(crate) arguments: BreakableString<'a>,
}
