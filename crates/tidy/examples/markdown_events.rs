//! Print the events the semantics oracle compares, one per line, for each file given. The
//! `oracle/shrink.ts` script compares them before and after formatting.

#[path = "../tests/support/events.rs"]
mod support;

fn main() {
  for path in std::env::args().skip(1) {
    match std::fs::read_to_string(&path) {
      Ok(text) => support::events(&text).iter().for_each(|event| println!("{event}")),
      Err(error) => {
        eprintln!("events: {path}: {error}");
        std::process::exit(1);
      }
    }
  }
}
