fn main() {
  let source = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
  for (event, range) in pulldown_cmark::Parser::new_ext(
    &source,
    pulldown_cmark::Options::ENABLE_TABLES
      | pulldown_cmark::Options::ENABLE_FOOTNOTES
      | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
      | pulldown_cmark::Options::ENABLE_TASKLISTS,
  )
  .into_offset_iter()
  {
    println!("{range:?} {event:?}");
  }
}
