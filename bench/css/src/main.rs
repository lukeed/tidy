//! Formats the style sheets under folders with one engine, the way `tidy check` does: exits 1 if any file changes.
//!
//!   css-bench <oxc|malva|lightningcss> <paths...> [--threads <n>] [--write <dir>]
//!
//! With `--write`, each output goes to the same path under `<dir>`, relative to the path it was found under.
//!
//! lightningcss isn't a formatter: it parses CSS and prints it back, so it shows how fast a style sheet can be read and
//! written. It has no SCSS or Less.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn main() {
  let mut args: Vec<String> = std::env::args().skip(1).collect();
  let threads = match args.iter().position(|arg| arg == "--threads") {
    Some(at) => {
      let count = args.get(at + 1).and_then(|it| it.parse().ok()).unwrap_or(1);
      args.drain(at..at + 2);
      count
    }
    None => std::thread::available_parallelism().map_or(1, |count| count.get()),
  };
  let out = match args.iter().position(|arg| arg == "--write") {
    Some(at) => args.drain(at..at + 2).nth(1).map(PathBuf::from),
    None => None,
  };
  let engine = args.remove(0);
  let mut files = Vec::new();
  for arg in &args {
    let root = Path::new(arg);
    let mut found = Vec::new();
    walk(root, &mut found);
    files.extend(found.into_iter().map(|path| (root.to_path_buf(), path)));
  }
  let next = AtomicUsize::new(0);
  let changed = AtomicUsize::new(0);
  std::thread::scope(|scope| {
    for _ in 0..threads.clamp(1, files.len().max(1)) {
      scope.spawn(|| {
        while let Some((root, path)) = files.get(next.fetch_add(1, Ordering::Relaxed)) {
          let Ok(text) = std::fs::read_to_string(path) else { continue };
          let output = format(&engine, path, &text);
          if let (Some(out), Some(output)) = (&out, &output) {
            let to = out.join(path.strip_prefix(root).unwrap_or(path));
            let _ = std::fs::create_dir_all(to.parent().unwrap_or(out));
            let _ = std::fs::write(to, output);
          }
          if output.is_some_and(|output| output != text) {
            changed.fetch_add(1, Ordering::Relaxed);
          }
        }
      });
    }
  });
  std::process::exit(i32::from(changed.load(Ordering::Relaxed) > 0));
}

fn format(engine: &str, path: &Path, text: &str) -> Option<String> {
  let extension = path.extension().and_then(|it| it.to_str()).unwrap_or("css");
  match engine {
    "oxc" => {
      use oxc_formatter_css::{CssFormatOptions, CssVariant};
      let variant = match extension {
        "scss" => CssVariant::Scss,
        "less" => CssVariant::Less,
        _ => CssVariant::Css,
      };
      let options =
        CssFormatOptions { variant, line_width: 80.try_into().ok()?, ..CssFormatOptions::default() };
      let allocator = oxc_allocator::Allocator::new();
      let formatted = oxc_formatter_css::format(&allocator, text, options).ok()?;
      Some(formatted.print().ok()?.into_code())
    }
    "malva" => {
      let syntax = match extension {
        "scss" => malva::Syntax::Scss,
        "less" => malva::Syntax::Less,
        _ => malva::Syntax::Css,
      };
      malva::format_text(text, syntax, &malva::config::FormatOptions::default()).ok()
    }
    "lightningcss" if extension == "css" => {
      use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
      let sheet = StyleSheet::parse(text, ParserOptions::default()).ok()?;
      Some(sheet.to_css(PrinterOptions::default()).ok()?.code)
    }
    _ => None,
  }
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
