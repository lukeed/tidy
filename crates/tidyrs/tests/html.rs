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

#[test]
fn prints_attributes() {
  assert_eq!(
    html(
      "<div class=\"a  b\" id = \"x\" data-a='y' data-b=\"it&apos;s\" title=\"one\ntwo\" CLASS=\"c\" class=\" d\" Data-X=\"z\" onclick=\"go()\" style=\"color:red\" class=\"e\tf\" xlink:href=\"#q\"><svg viewBox=\"0 0 1 1\"></svg></div>\n"
    ),
    "<div\n  class=\"a b\"\n  id=\"x\"\n  data-a=\"y\"\n  data-b=\"it's\"\n  title=\"one\ntwo\"\n  class=\"c\"\n  class=\"d\"\n  Data-X=\"z\"\n  onclick=\"go()\"\n  style=\"color: red\"\n  class=\"e f\"\n  xlink:href=\"#q\"\n>\n  <svg viewBox=\"0 0 1 1\"></svg>\n</div>\n"
  );
}

#[test]
fn prints_quotes_and_entities() {
  assert_eq!(
    html(
      "<div data-a='{\"x\":1}' data-b='plain' data-c=\"a &amp; b\" data-d=\"say &quot;hi&quot;\" data-e='it&apos;s \"x\"' style=\"color:red\" onclick=\"go()\" title='\"q\" and &quot;r&quot;'><img srcset=\"a.png 1x, b.png 2x\"></div>\n"
    ),
    "<div\n  data-a='{\"x\":1}'\n  data-b=\"plain\"\n  data-c=\"a &amp; b\"\n  data-d='say \"hi\"'\n  data-e='it&apos;s \"x\"'\n  style=\"color: red\"\n  onclick=\"go()\"\n  title='\"q\" and \"r\"'\n>\n  <img srcset=\"a.png 1x, b.png 2x\" />\n</div>\n"
  );
}
