use tidy::css::{Dialect, Settings, format};
use tidy::embed::Verbatim;

fn css(text: &str, dialect: Dialect) -> String {
  format(text, &Settings { dialect, ..Settings::default() }, &mut Verbatim).unwrap()
}

#[test]
fn formats_each_dialect() {
  assert_eq!(
    css(
      "a{color:RED;margin:0 0 0 0}@media (max-width:100px){.b,.c>d{background:url(x.png)}}",
      Dialect::Css
    ),
    "a {\n  color: RED;\n  margin: 0 0 0 0;\n}\n@media (max-width: 100px) {\n  .b,\n  .c > d {\n    background: url(x.png);\n  }\n}\n"
  );
  assert_eq!(
    css("$map:(key:value,other:1);.a{&:hover{color:red}}", Dialect::Scss),
    "$map: (\n  key: value,\n  other: 1,\n);\n.a {\n  &:hover {\n    color: red;\n  }\n}\n"
  );
  assert_eq!(
    css("@c:red;.a{.mixin();color:@c}", Dialect::Less),
    "@c: red;\n.a {\n  .mixin();\n  color: @c;\n}\n"
  );
}

#[test]
fn keeps_a_byte_order_mark() {
  assert_eq!(css("\u{feff}a{b:c}", Dialect::Css), "\u{feff}a {\n  b: c;\n}\n");
}

#[test]
fn takes_another_ignore_directive() {
  let settings =
    Settings { dialect: Dialect::Scss, ignore: "tidy-ignore".into(), ..Settings::default() };
  assert_eq!(
    format("// tidy-ignore\na{b:c}\n/* prettier-ignore */\nd{e:f}", &settings, &mut Verbatim)
      .unwrap(),
    "// tidy-ignore\na{b:c}\n/* prettier-ignore */\nd {\n  e: f;\n}\n"
  );
}
