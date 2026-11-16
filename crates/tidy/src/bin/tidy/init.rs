//! `tidy init`: a config with the default options, as `tidy.jsonc`, `tidy.json` or `tidy.toml`.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

use serde_json::Value;
use tidy::embed::Verbatim;
use tidy::{Language, Options};

const DOCS: &str = "https://tidy.lukeed.com/docs/configuration/";

/// Options whose default differs by language: a value in the config would override it for every language.
const VARIES: [&str; 2] = ["width", "whitespace"];

/// The kind of file to write.
pub(crate) enum Format {
  Jsonc,
  Json,
  Toml,
}

impl Format {
  /// The format the flags of `tidy init` pick: JSONC unless `--json` or `--toml`.
  pub(crate) fn of(args: &[OsString]) -> Result<Format, String> {
    let mut format = Format::Jsonc;
    for arg in args {
      let next = match arg.to_str() {
        Some("--json") => Format::Json,
        Some("--toml") => Format::Toml,
        _ => {
          let arg = arg.to_string_lossy();
          return Err(format!("unknown flag `{arg}`, see `tidy init --help`"));
        }
      };
      if !matches!(format, Format::Jsonc) {
        return Err("pick one of --json and --toml".into());
      }
      format = next;
    }
    Ok(format)
  }

  fn name(&self) -> &'static str {
    match self {
      Format::Jsonc => "tidy.jsonc",
      Format::Json => "tidy.json",
      Format::Toml => "tidy.toml",
    }
  }
}

/// Writes the config in the current folder, unless one is there already.
pub(crate) fn run(format: Format) -> Result<(), String> {
  for name in ["tidy.json", "tidy.jsonc", "tidy.toml"] {
    if Path::new(name).exists() {
      return Err(format!("{name} is here already"));
    }
  }
  let options: Vec<(&str, Value)> = tidy::schema::shared()
    .into_iter()
    .filter(|option| !VARIES.contains(&option.name))
    .map(|option| (option.name, option.default))
    .collect();
  let url = tidy::schema::url();
  let raw = match format {
    Format::Jsonc => format!("// {DOCS}\n{}", json(&url, &options)),
    Format::Json => json(&url, &options),
    Format::Toml => toml(&url, &options),
  };
  let name = format.name();
  let language = Language::of_path(name).ok_or_else(|| format!("{name}: no language"))?;
  let text = tidy::format(&raw, language, &Options::default(), Some(name), &mut Verbatim)
    .map_err(|error| format!("{name}: {error}"))?;
  let mut file = std::fs::File::create_new(name).map_err(|error| format!("{name}: {error}"))?;
  file.write_all(text.as_bytes()).map_err(|error| format!("{name}: {error}"))?;
  eprintln!("tidy: wrote {name}");
  Ok(())
}

/// The options as JSON, in the order the schema lists them, which serde_json's sorted maps would lose.
fn json(url: &str, options: &[(&str, Value)]) -> String {
  let mut lines = vec![format!("\"$schema\": {}", Value::from(url))];
  lines.extend(options.iter().map(|(name, value)| format!("\"{name}\": {value}")));
  format!("{{\n{}\n}}\n", lines.join(",\n"))
}

/// The options as TOML, with the schema in taplo's `#:schema` directive, which must come first.
fn toml(url: &str, options: &[(&str, Value)]) -> String {
  let lines = options.iter().map(|(name, value)| format!("{name} = {value}\n"));
  format!("#:schema {url}\n# {DOCS}\n\n{}", lines.collect::<String>())
}
