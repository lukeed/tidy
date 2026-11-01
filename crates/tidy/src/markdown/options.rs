/// A starting set of style choices. Override any field after picking one.
///
/// # Example
/// ```
/// use mdtidy::{Options, Preset, Wrap};
/// let options = Options { wrap: Wrap::Always, ..Options::from(Preset::Preserve) };
/// assert_eq!(options.ignore, "mdtidy-ignore");
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
  /// The choices Prettier makes.
  #[default]
  Prettier,
  /// Keep every marker as written and only normalize the layout of blocks.
  Preserve,
}

/// How paragraphs break into lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Wrap {
  /// Keep the line breaks as written.
  #[default]
  Preserve,
  /// Fill each line up to [`Options::width`].
  Always,
  /// Put each paragraph on one line.
  Never,
}

/// The marker of bullet list items. Adjacent sibling lists alternate with another marker so they
/// stay separate lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bullet {
  Keep,
  /// `-`, alternating with `*`.
  Dash,
  /// `*`, alternating with `-`.
  Asterisk,
  /// `+`, alternating with `-`.
  Plus,
}

/// The delimiter of emphasis. `_` falls back to `*` where it can't work: inside a word and inside
/// other emphasis. Either becomes `_` right after another `*`, where the two would merge into one
/// run that nests differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emphasis {
  Keep,
  Underscore,
  Asterisk,
}

/// The delimiter of strong emphasis. `__` falls back to `**` inside a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strong {
  Keep,
  Asterisk,
  Underscore,
}

/// The numbers of ordered list items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Numbering {
  Keep,
  /// Count up from the first number, unless the second item repeats a number (`1.`, `1.`, `1.`),
  /// in which case every item after the first keeps it. Delimiters alternate between adjacent
  /// sibling lists.
  Auto,
}

/// Thematic breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
  Keep,
  /// `---`, or `***` inside a list whose marker could be mistaken for it.
  Dash,
  Asterisk,
  Underscore,
}

/// Fences of code blocks, always longer than any run of the same character inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fence {
  Keep,
  Backtick,
  Tilde,
}

/// The style of headings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
  Keep,
  /// `#` prefixes, which setext headings turn into.
  Atx,
}

/// How to format a document. [`Options::default`] is the [`Preset::Prettier`] preset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
  /// The line width that [`Wrap::Always`] fills to, and that tables and code blocks are told about.
  pub width: usize,
  pub wrap: Wrap,
  pub bullet: Bullet,
  pub emphasis: Emphasis,
  pub strong: Strong,
  pub numbering: Numbering,
  pub rule: Rule,
  pub fence: Fence,
  pub heading: Heading,
  /// The name of the ignore directive, an HTML comment on its own line:
  /// `<!-- name -->` keeps the next block as written, `<!-- name-start -->` and `<!-- name-end -->`
  /// keep everything between them, and `<!-- name-file -->` at the top keeps the whole file.
  pub ignore: String,
}

impl Default for Options {
  fn default() -> Self {
    Self::from(Preset::default())
  }
}

impl From<Preset> for Options {
  fn from(preset: Preset) -> Self {
    match preset {
      Preset::Prettier => Self {
        width: 80,
        wrap: Wrap::Preserve,
        bullet: Bullet::Dash,
        emphasis: Emphasis::Underscore,
        strong: Strong::Asterisk,
        numbering: Numbering::Auto,
        rule: Rule::Dash,
        fence: Fence::Backtick,
        heading: Heading::Atx,
        ignore: "prettier-ignore".into(),
      },
      Preset::Preserve => Self {
        width: 80,
        wrap: Wrap::Preserve,
        bullet: Bullet::Keep,
        emphasis: Emphasis::Keep,
        strong: Strong::Keep,
        numbering: Numbering::Keep,
        rule: Rule::Keep,
        fence: Fence::Keep,
        heading: Heading::Keep,
        ignore: "mdtidy-ignore".into(),
      },
    }
  }
}

/// Read an option's value from its lowercase name, as the CLI and the Node binding take it.
macro_rules! names {
  ($($kind:ident { $($name:literal => $variant:ident),+ $(,)? })+) => {$(
    impl std::str::FromStr for $kind {
      type Err = String;

      fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
          $($name => Ok(Self::$variant),)+
          _ => Err(format!(
            "unknown {} `{text}`, expected one of: {}",
            stringify!($kind).to_lowercase(),
            [$($name),+].join(", ")
          )),
        }
      }
    }
  )+};
}

names! {
  Preset { "prettier" => Prettier, "preserve" => Preserve }
  Wrap { "preserve" => Preserve, "always" => Always, "never" => Never }
  Bullet { "keep" => Keep, "dash" => Dash, "asterisk" => Asterisk, "plus" => Plus }
  Emphasis { "keep" => Keep, "underscore" => Underscore, "asterisk" => Asterisk }
  Strong { "keep" => Keep, "asterisk" => Asterisk, "underscore" => Underscore }
  Numbering { "keep" => Keep, "auto" => Auto }
  Rule { "keep" => Keep, "dash" => Dash, "asterisk" => Asterisk, "underscore" => Underscore }
  Fence { "keep" => Keep, "backtick" => Backtick, "tilde" => Tilde }
  Heading { "keep" => Keep, "atx" => Atx }
}
