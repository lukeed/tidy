//! INI files as prettier-plugin-ini 1.3.0 formats them, and the INI dialects it reads too: EditorConfig, git config,
//! `.npmrc` and Python's configparser files. tidy also reads the lines the plugin rejects: continuation lines, keys
//! without values, `key: value`, a comment after a section, and `]` in a section's name.
//!
//! ```text
//! text ─ parse::parse ─→ Tree ─ print::print ─→ String
//! ```

mod parse;
mod print;

use crate::Error;
use crate::doc::Eol;

/// How a file reads its lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dialect {
  #[default]
  Ini,
  /// git config and `.gitmodules`: every line keeps its indent, and an indented line is a key of its own.
  Git,
}

/// The names that the plugin formats, and `.npmrc`.
const NAMES: [&str; 12] = [
  ".coveragerc",
  ".flake8",
  ".pylintrc",
  "pylintrc",
  "buildozer.spec",
  "vlcrc",
  ".editorconfig",
  ".shellcheckrc",
  ".npmrc",
  ".gitconfig",
  ".gitmodules",
  "gitconfig",
];

/// The extensions that the plugin formats, but `.pro`, which is mostly qmake's, and `.reg`, which regedit writes in
/// UTF-16.
const EXTENSIONS: [&str; 16] = [
  "ini",
  "cfg",
  "cnf",
  "dof",
  "lektorproject",
  "prefs",
  "properties",
  "url",
  "sfv",
  "mc",
  "editorconfig",
  "gitconfig",
  "outjob",
  "pcbdoc",
  "prjpcb",
  "schdoc",
];

impl Dialect {
  /// The dialect of the file at `path`, or `None` if tidy doesn't format it as INI.
  pub fn of_path(path: &str) -> Option<Dialect> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let lower = name.to_ascii_lowercase();
    let extension = lower.rsplit_once('.').map(|(_, extension)| extension);
    let git = matches!(lower.as_str(), ".gitconfig" | ".gitmodules" | "gitconfig")
      || extension == Some("gitconfig");
    let ini = NAMES.contains(&lower.as_str())
      || extension.is_some_and(|extension| EXTENSIONS.contains(&extension));
    match (git, ini) {
      (true, _) => Some(Dialect::Git),
      (false, true) => Some(Dialect::Ini),
      _ => None,
    }
  }
}

/// Options of the INI formatter, prettier-plugin-ini's, but with spaces around `=` by default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
  pub dialect: Dialect,
  /// `iniSpaceAroundEquals`: `key = value` rather than `key=value`.
  pub spaces: bool,
  /// The indent of the values of a key over lines.
  pub tab_width: usize,
  pub tabs: bool,
  pub eol: Eol,
}

impl Default for Options {
  fn default() -> Self {
    Options { dialect: Dialect::Ini, spaces: true, tab_width: 2, tabs: false, eol: Eol::Lf }
  }
}

/// Formats `text`.
pub fn format(text: &str, options: &Options) -> Result<String, Error> {
  Formatter::default().format(text, options).map(str::to_owned)
}

/// Formats one INI file after another, reusing its buffer.
#[derive(Default)]
pub struct Formatter {
  output: String,
}

impl Formatter {
  /// Formats `text`.
  pub fn format(&mut self, text: &str, options: &Options) -> Result<&str, Error> {
    let (bom, text) = crate::without_bom(text);
    self.output.clear();
    // Prettier's core prints nothing for a text of white space only.
    if !parse::trim(text).is_empty() {
      let tree = parse::parse(text, options.dialect);
      print::print(&tree, options, &mut self.output);
    }
    Ok(crate::with_bom(bom, &mut self.output))
  }
}
