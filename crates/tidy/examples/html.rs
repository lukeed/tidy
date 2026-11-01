//! Formats the files given as arguments and prints any that panic, for debugging.

use tidy::embed::Verbatim;
use tidy::html::{Options, Parser, format};

fn main() {
  let quiet = std::env::args().any(|arg| arg == "--quiet");
  for path in std::env::args().skip(1).filter(|arg| !arg.starts_with("--")) {
    let Ok(text) = std::fs::read_to_string(&path) else { continue };
    let parser = if path.ends_with(".vue") { Parser::Vue } else { Parser::Html };
    let options = Options { parser, filepath: Some(path.clone()), ..Options::default() };
    match std::panic::catch_unwind(|| format(&text, &options, &mut Verbatim)) {
      Ok(Ok(output)) if !quiet => print!("{output}"),
      Ok(Ok(_)) => {}
      Ok(Err(error)) if !quiet => eprintln!("{path}: {error}"),
      Ok(Err(_)) => {}
      Err(_) => eprintln!("PANIC {path}"),
    }
  }
}
