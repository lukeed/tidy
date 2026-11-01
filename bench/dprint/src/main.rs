//! Formats the files under a folder with markup_fmt, the engine of dprint-plugin-markup, leaving embedded code as it
//! is written. Exits 1 if any file changes, like `--check`.

use std::path::{Path, PathBuf};

use markup_fmt::config::FormatOptions;
use markup_fmt::{Language, format_text};

fn main() {
  let mut files = Vec::new();
  for arg in std::env::args().skip(1) {
    walk(Path::new(&arg), &mut files);
  }
  let options = FormatOptions::default();
  let mut changed = 0;
  for path in files {
    let Ok(text) = std::fs::read_to_string(&path) else { continue };
    let language = match path.extension().and_then(|it| it.to_str()) {
      Some("vue") => Language::Vue,
      Some("svelte") => Language::Svelte,
      Some("astro") => Language::Astro,
      Some("xml" | "svg" | "xsd" | "xsl" | "xslt" | "wsdl") => Language::Xml,
      _ => Language::Html,
    };
    let output = format_text(&text, language, &options, |code, _| Ok::<_, ()>(code.into()));
    if output.is_ok_and(|output| output != text) {
      changed += 1;
    }
  }
  std::process::exit(i32::from(changed > 0));
}

fn walk(path: &Path, files: &mut Vec<PathBuf>) {
  if path.is_file() {
    files.push(path.to_path_buf());
    return;
  }
  let Ok(entries) = std::fs::read_dir(path) else { return };
  for entry in entries.flatten() {
    walk(&entry.path(), files);
  }
}
