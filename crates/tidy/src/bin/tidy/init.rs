//! `tidy init`: a tidy.json with the default options.

use std::io::Write;
use std::path::Path;

use tidy::embed::Verbatim;
use tidy::{Language, Options, json};

/// Options whose default differs by language: a value in the config would override it for every language.
const VARIES: [&str; 2] = ["width", "whitespace"];

/// Writes `tidy.json` in the current folder, unless a config is there already.
pub(crate) fn run() -> Result<(), String> {
  for name in ["tidy.json", "tidy.jsonc", "tidy.toml"] {
    if Path::new(name).exists() {
      return Err(format!("{name} is here already"));
    }
  }
  // In the order the schema lists them, which serde_json's sorted maps would lose.
  let mut lines = vec![format!("\"$schema\": {}", serde_json::Value::from(tidy::schema::url()))];
  for option in tidy::schema::shared() {
    if !VARIES.contains(&option.name) {
      lines.push(format!("\"{}\": {}", option.name, option.default));
    }
  }
  let raw = format!("{{\n{}\n}}\n", lines.join(",\n"));
  let json = Language::Json(json::Dialect::Json);
  let text = tidy::format(&raw, json, &Options::default(), Some("tidy.json"), &mut Verbatim)
    .map_err(|error| error.to_string())?;
  let mut file =
    std::fs::File::create_new("tidy.json").map_err(|error| format!("tidy.json: {error}"))?;
  file.write_all(text.as_bytes()).map_err(|error| format!("tidy.json: {error}"))?;
  eprintln!("tidy: wrote tidy.json");
  Ok(())
}
