use tidy::embed::Verbatim;
use tidy::html::{Options, format};

fn html(text: &str) -> String {
  let options = Options { filepath: Some("index.html".into()), ..Options::default() };
  format(text, &options, &mut Verbatim).unwrap()
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

#[test]
fn prints_borrowed_markers_once() {
  assert_eq!(html("<b>a</b><textarea> </textarea>"), "<b>a</b><textarea> </textarea>\n");
  assert_eq!(html("<p><br><textarea> </textarea></p>"), "<p><br /><textarea> </textarea></p>\n");
  assert_eq!(
    html("<b>a</b><textarea> </textarea><b>c</b>"),
    "<b>a</b><textarea> </textarea><b>c</b>\n"
  );
}

#[test]
fn leaves_attributes_without_embedded_formatting() {
  let options = Options { format_embedded: false, ..Options::default() };
  assert_eq!(
    format(
      "<img class=\"a   b\" srcset=\"a.png 1x,b.png   2x\"><p style=\"color:red\"></p>",
      &options,
      &mut Verbatim
    )
    .unwrap(),
    "<img class=\"a   b\" srcset=\"a.png 1x,b.png   2x\" />\n<p style=\"color:red\"></p>\n"
  );
}

#[test]
fn takes_another_ignore_directive() {
  let options = Options { ignore: "tidy-ignore".into(), ..Options::default() };
  let text = "<!-- tidy-ignore -->\n<p   a=b>x</p>\n<!-- tidy-ignore-attribute -->\n<p   a=b>x</p>\n<!-- prettier-ignore -->\n<p   a=b>x</p>\n";
  assert_eq!(
    format(text, &options, &mut Verbatim).unwrap(),
    "<!-- tidy-ignore -->\n<p   a=b>x</p>\n<!-- tidy-ignore-attribute -->\n<p a=b>x</p>\n<!-- prettier-ignore -->\n<p a=\"b\">x</p>\n"
  );
}

#[test]
fn tells_the_host_whether_a_script_is_a_module() {
  let mut modules = Vec::new();
  let mut hook = |request: tidy::embed::Request<'_>| {
    modules.push(request.module);
    Some("x = `</SCRIPT>`".to_owned())
  };
  let text = "<script>a</script>\n<script type=\"module\">a</script>\n<script type=\"text/babel\" data-type=\"module\">a</script>\n";
  let printed = format(text, &Options::default(), &mut hook).unwrap();
  assert_eq!(modules, [false, true, true]);
  assert_eq!(printed.matches(r"x = `<\/SCRIPT>`").count(), 3, "{printed}");
}
