//! `tidy init`: a config as `tidy.jsonc`, `tidy.json` or `tidy.toml`, with the options of a Prettier or oxfmt config
//! here, or with the default options.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tidy::embed::Verbatim;
use tidy::{Language, Options};

use crate::migrate::{self, Draft};

const DOCS: &str = "https://tidy.lukeed.com/docs/configuration/";

/// Options whose default differs by language: a value in the config would override it for every language.
const VARIES: [&str; 2] = ["width", "whitespace"];

/// The kind of file to write.
enum Format {
  Jsonc,
  Json,
  Toml,
}

impl Format {
  fn name(&self) -> &'static str {
    match self {
      Format::Jsonc => "tidy.jsonc",
      Format::Json => "tidy.json",
      Format::Toml => "tidy.toml",
    }
  }
}

/// The flags of `tidy init`.
pub(crate) struct Args {
  format: Format,
  from: Option<PathBuf>,
}

impl Args {
  /// The flags in `args`: JSONC unless `--json` or `--toml`, and the config to migrate after `--from`.
  pub(crate) fn of(args: &[OsString]) -> Result<Args, String> {
    let (mut format, mut from) = (Format::Jsonc, None);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
      let text = arg.to_string_lossy();
      let next = match text.as_ref() {
        "--json" => Format::Json,
        "--toml" => Format::Toml,
        "--from" => {
          from = Some(PathBuf::from(args.next().ok_or("`--from` needs a file")?));
          continue;
        }
        _ if text.starts_with("--from=") => {
          from = Some(PathBuf::from(&text["--from=".len()..]));
          continue;
        }
        _ => return Err(format!("unknown flag `{text}`, see `tidy init --help`")),
      };
      if !matches!(format, Format::Jsonc) {
        return Err("pick one of --json and --toml".into());
      }
      format = next;
    }
    Ok(Args { format, from })
  }
}

/// Writes the config in the current folder, unless one is there already.
pub(crate) fn run(args: Args) -> Result<(), String> {
  for name in tidy::config::NAMES {
    if Path::new(name).exists() {
      return Err(format!("{name} is here already"));
    }
  }
  let source = match args.from {
    Some(path) => Some(path),
    None => migrate::find(Path::new("."))?,
  };
  let (draft, notes) = match &source {
    Some(path) => {
      let migration = migrate::read(path)?;
      (migration.draft, migration.notes)
    }
    None => (defaults(), Vec::new()),
  };
  let url = tidy::schema::url();
  let raw = match args.format {
    Format::Jsonc => format!("// {DOCS}\n{}", json(&url, &draft)),
    Format::Json => json(&url, &draft),
    Format::Toml => toml(&url, &draft),
  };
  let name = args.format.name();
  let language = Language::of_path(name).ok_or_else(|| format!("{name}: no language"))?;
  // The file is formatted as the options in it say, as `tidy check` will read it.
  let mut options = Options::default();
  tidy::config::Config::from_value(&draft.to_value())
    .map_err(|error| format!("{name}: {error}"))?
    .apply(&mut options);
  let text = tidy::format(&raw, language, &options, Some(name), &mut Verbatim)
    .map_err(|error| format!("{name}: {error}"))?;
  let mut file = std::fs::File::create_new(name).map_err(|error| format!("{name}: {error}"))?;
  file.write_all(text.as_bytes()).map_err(|error| format!("{name}: {error}"))?;
  match &source {
    Some(path) => {
      let shown = path.strip_prefix(".").unwrap_or(path);
      eprintln!("tidy: wrote {name} from {}", shown.display());
    }
    None => eprintln!("tidy: wrote {name}"),
  }
  for note in notes {
    eprintln!("tidy: {note}");
  }
  Ok(())
}

/// The shared options at their defaults, in the order the schema lists them.
fn defaults() -> Draft {
  let shared = (tidy::schema::shared().into_iter())
    .filter(|option| !VARIES.contains(&option.name))
    .map(|option| (option.name.to_owned(), option.default))
    .collect();
  Draft { shared, sections: Vec::new() }
}

/// The config as JSON, in the draft's order, which serde_json's sorted maps would lose.
fn json(url: &str, draft: &Draft) -> String {
  let pair = |name: &str, value: String| format!("{}: {value}", Value::from(name));
  let mut lines = vec![pair("$schema", Value::from(url).to_string())];
  lines.extend(draft.shared.iter().map(|(name, value)| pair(name, value.to_string())));
  for (name, entries) in &draft.sections {
    let inner: Vec<String> =
      entries.iter().map(|(key, value)| pair(key, value.to_string())).collect();
    lines.push(pair(name, format!("{{\n{}\n}}", inner.join(",\n"))));
  }
  format!("{{\n{}\n}}\n", lines.join(",\n"))
}

/// The config as TOML, with the schema in taplo's `#:schema` directive, which must come first.
fn toml(url: &str, draft: &Draft) -> String {
  let lines = |entries: &[(String, Value)]| -> String {
    entries.iter().map(|(name, value)| format!("{} = {}\n", key(name), literal(value))).collect()
  };
  let mut text = format!("#:schema {url}\n# {DOCS}\n\n{}", lines(&draft.shared));
  for (name, entries) in &draft.sections {
    text.push_str(&format!("\n[{}]\n{}", key(name), lines(entries)));
  }
  text
}

/// A TOML key, quoted unless it is bare.
fn key(name: &str) -> String {
  let bare = name.bytes().all(|it| it.is_ascii_alphanumeric() || matches!(it, b'_' | b'-'));
  if bare { name.to_owned() } else { Value::from(name).to_string() }
}

/// A TOML value. JSON's strings, numbers and booleans are TOML's too.
fn literal(value: &Value) -> String {
  match value {
    Value::Array(items) => {
      format!("[{}]", items.iter().map(literal).collect::<Vec<_>>().join(", "))
    }
    Value::Object(map) => {
      let pairs: Vec<String> =
        map.iter().map(|(name, value)| format!("{} = {}", key(name), literal(value))).collect();
      format!("{{ {} }}", pairs.join(", "))
    }
    value => value.to_string(),
  }
}
