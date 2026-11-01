//! Formatting must keep what a document means and settle after one pass: re-parsing the output
//! gives the same events as the input, and formatting the output again changes nothing.

use std::path::{Path, PathBuf};

use mdtidy::{Options, Preset, Wrap};
use pulldown_cmark::{CodeBlockKind, Event, Options as Flags, Parser, Tag};

fn flags() -> Flags {
  Flags::ENABLE_TABLES
    | Flags::ENABLE_FOOTNOTES
    | Flags::ENABLE_STRIKETHROUGH
    | Flags::ENABLE_TASKLISTS
    | Flags::ENABLE_YAML_STYLE_METADATA_BLOCKS
    | Flags::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
}

/// The events of `text`, with where lines break inside text not counting. Link labels match
/// with their whitespace collapsed, so they compare that way.
fn events(text: &str) -> Vec<String> {
  let mut out = Vec::new();
  let mut pending = String::new();
  for event in Parser::new_ext(text, flags()) {
    match &event {
      Event::Text(text) => {
        pending.push_str(text);
        continue;
      }
      Event::SoftBreak => {
        pending.push(' ');
        continue;
      }
      _ => {}
    }
    close(&mut out, &mut pending);
    out.push(match event {
      Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
        format!("Fenced({:?})", words(&info))
      }
      Event::Html(html) => format!("Html({:?})", html.trim_end()),
      Event::InlineHtml(html) => format!("InlineHtml({:?})", html.as_ref()),
      Event::Code(code) => format!("Code({:?})", code.as_ref()),
      Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
        format!(
          "Link({link_type:?}, {:?}, {:?}, {:?})",
          dest_url.as_ref(),
          title.as_ref(),
          words(&id)
        )
      }
      Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
        format!(
          "Image({link_type:?}, {:?}, {:?}, {:?})",
          dest_url.as_ref(),
          title.as_ref(),
          words(&id)
        )
      }
      event => format!("{event:?}"),
    });
  }
  close(&mut out, &mut pending);
  out
}

/// Close a run of text, whose whitespace only matters as a separator. Whitespace alone renders
/// as nothing.
fn close(out: &mut Vec<String>, pending: &mut String) {
  let text = words(pending);
  if !text.is_empty() {
    out.push(format!("Text({text:?})"));
  }
  pending.clear();
}

fn words(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn files() -> Vec<PathBuf> {
  let mut files = Vec::new();
  let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
  walk(&fixtures, &mut files);
  if let Ok(corpus) = std::env::var("MDTIDY_CORPUS") {
    walk(Path::new(&corpus), &mut files);
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
