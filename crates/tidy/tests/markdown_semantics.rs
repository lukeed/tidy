//! Formatting must keep what a document means and settle after one pass: re-parsing the output
//! gives the same events as the input, and formatting the output again changes nothing.
//! `MDTIDY_CORPUS` adds folders of Markdown files to check, separated like `PATH`.

use std::path::{Path, PathBuf};

use mdtidy::{Options, Preset, Wrap};

#[path = "support/events.rs"]
mod support;

use support::events;

fn files() -> Vec<PathBuf> {
  let mut files = Vec::new();
  let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
  walk(&fixtures, &mut files);
  if let Some(corpora) = std::env::var_os("MDTIDY_CORPUS") {
    std::env::split_paths(&corpora).for_each(|corpus| walk(&corpus, &mut files));
  }
  files.sort();
  files
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
  let Ok(entries) = std::fs::read_dir(dir) else { return };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      walk(&path, files);
    } else if path.extension().is_some_and(|ext| ext == "md") {
      files.push(path);
    }
  }
}

fn verify(options: &Options) {
  let mut failures = Vec::new();
  for path in files() {
    let Ok(source) = std::fs::read_to_string(&path) else { continue };
    let once = mdtidy::format(&source, options);
    let terminated = if source.ends_with('\n') { source.clone() } else { format!("{source}\n") };
    let (before, after) = (events(&terminated), events(&once));
    if before != after {
      let index = before.iter().zip(&after).position(|(a, b)| a != b).unwrap_or(0);
      failures.push(format!(
        "{}: changes meaning\n  in:  {}\n  out: {}",
        path.display(),
        before.get(index).map_or("<end>", String::as_str),
        after.get(index).map_or("<end>", String::as_str),
      ));
    }
    if mdtidy::format(&once, options) != once {
      failures.push(format!("{}: changes again on a second pass", path.display()));
    }
  }
  assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn prettier_keeps_meaning() {
  verify(&Options::default());
}

#[test]
fn preserve_keeps_meaning() {
  verify(&Options::from(Preset::Preserve));
}

#[test]
fn wrapping_keeps_meaning() {
  verify(&Options { wrap: Wrap::Always, width: 40, ..Options::default() });
  verify(&Options { wrap: Wrap::Never, ..Options::default() });
}
