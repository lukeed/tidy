use tidyrs::embed::Verbatim;
use tidyrs::html::{Settings, format};

fn html(text: &str) -> String {
  let settings = Settings { filepath: Some("index.html".into()), ..Settings::default() };
  format(text, &settings, &mut Verbatim).unwrap()
}

#[test]
fn formats_a_document() {
  assert_eq!(
    html(
      "<!DOCTYPE html><html><head><title>Hi</title></head><body><p class='a   b'>Hello <b>world</b></p></body></html>"
    ),
    "<!doctype html>\n<html>\n  <head>\n    <title>Hi</title>\n  </head>\n  <body>\n    <p class=\"a b\">Hello <b>world</b></p>\n  </body>\n</html>\n"
  );
}
