//! Dockerfiles as dprint-plugin-dockerfile 0.7.0 formats them, with ports of its parser and generator. A line that
//! doesn't parse as its instruction stays as it is, so formatting never fails. Shell commands keep their words, but
//! runs of whitespace outside quotes collapse; heredocs stay as they are.
//!
//! ```text
//! text ─ parse::parse ─→ Dockerfile ─ print::print ─→ String
//! ```

mod ast;
mod parse;
mod print;

use crate::Error;
use crate::doc::Eol;

/// True if tidy formats the file at `path` as a Dockerfile: `Dockerfile`, `Containerfile`, a variant of them such as
/// `Dockerfile.dev`, or a name that ends with `.dockerfile` or `.containerfile`.
pub fn is_dockerfile_path(path: &str) -> bool {
  let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
  let lower = name.to_ascii_lowercase();
  let variant = |base: &str| {
    name == base
      || name.strip_prefix(base).and_then(|rest| rest.strip_prefix('.')).is_some_and(is_suffix)
  };
  variant("Dockerfile")
    || variant("Containerfile")
    || [".dockerfile", ".containerfile"]
      .iter()
      .any(|end| lower.len() > end.len() && lower.ends_with(end))
}

/// A suffix of a variant, such as `dev`, that isn't the extension of another language.
fn is_suffix(suffix: &str) -> bool {
  !suffix.is_empty()
    && !suffix.contains('.')
    && crate::Language::of_path(&format!("x.{suffix}")).is_none()
}

/// Options of the Dockerfile formatter, dprint-plugin-dockerfile's by default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
  /// `lineWidth`: past it, the values of `ENV`, `LABEL` and `HEALTHCHECK` go on lines of their own.
  pub width: usize,
  pub eol: Eol,
  /// `indentStages`: indent the instructions of each stage, after its `FROM`.
  pub indent_stages: bool,
  /// `indentWidth`: the spaces of a stage's indent.
  pub indent_width: usize,
  /// `healthcheckCmdNewLine`: put the command of a `HEALTHCHECK` with options on its own line.
  pub healthcheck_cmd_new_line: bool,
}

impl Default for Options {
  fn default() -> Self {
    Options {
      width: 120,
      eol: Eol::Lf,
      indent_stages: false,
      indent_width: 2,
      healthcheck_cmd_new_line: false,
    }
  }
}

/// Formats `text`.
pub fn format(text: &str, options: &Options) -> Result<String, Error> {
  Formatter::default().format(text, options).map(str::to_owned)
}

/// Formats one Dockerfile after another, reusing its buffer.
#[derive(Default)]
pub struct Formatter {
  output: String,
}

impl Formatter {
  /// Formats `text`.
  pub fn format(&mut self, text: &str, options: &Options) -> Result<&str, Error> {
    let (bom, text) = crate::without_bom(text);
    self.output.clear();
    let file = parse::parse(text);
    print::print(&file, options, &mut self.output);
    Ok(crate::with_bom(bom, &mut self.output))
  }
}

/// Parses `text` into the tree that prints. For `bench/parse` only.
#[cfg(tidy_bench)]
pub fn bench_parse(text: &str) -> bool {
  std::hint::black_box(parse::parse(text));
  true
}
