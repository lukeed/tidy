#![cfg(feature = "js")]

use tidy::embed::Verbatim;
use tidy::js::{self, ArrowParens, Dialect, Options, TrailingComma};
use tidy::json::QuoteProps;
use tidy::{Language, doc};

fn format(text: &str, path: &str, options: &Options) -> String {
  js::format(text, path, options).unwrap()
}

#[test]
fn formats_javascript() {
  let text = "const a = {b:1, \"c-d\": 2}\nlet f = x => x*2\nfoo(function() { return 1 })\n";
  let expected =
    "const a = { b: 1, \"c-d\": 2 };\nlet f = (x) => x * 2;\nfoo(function () {\n  return 1;\n});\n";
  assert_eq!(format(text, "a.js", &Options::default()), expected);
}

#[test]
fn breaks_arguments_with_a_trailing_comma() {
  let text = "const veryLongVariableName = someFunction(argumentNumberOne, argumentNumberTwo, argumentNumberThree)\n";
  let expected = "const veryLongVariableName = someFunction(\n  argumentNumberOne,\n  argumentNumberTwo,\n  argumentNumberThree,\n);\n";
  assert_eq!(format(text, "a.js", &Options::default()), expected);
}

#[test]
fn follows_the_options() {
  let options = Options {
    semi: false,
    single_quote: true,
    arrow_parens: ArrowParens::Avoid,
    trailing_comma: TrailingComma::None,
    quote_props: QuoteProps::Consistent,
    ..Options::default()
  };
  let text = "const a = {b:1, \"c-d\": 2}\nlet f = x => x*2\nfoo(function() { return 1 })\n";
  let expected =
    "const a = { 'b': 1, 'c-d': 2 }\nlet f = x => x * 2\nfoo(function () {\n  return 1\n})\n";
  assert_eq!(format(text, "a.js", &options), expected);
}

#[test]
fn formats_jsx() {
  let options = Options { jsx_single_quote: true, ..Options::default() };
  let text = "const el = <div className=\"a\" id={x}>hi</div>\n";
  let expected = "const el = (\n  <div className='a' id={x}>\n    hi\n  </div>\n);\n";
  assert_eq!(format(text, "a.jsx", &options), expected);
}

#[test]
fn formats_typescript_by_dialect() {
  let text = "interface A { b: string; c?: number }\ntype U = \"a\" | \"b\"\nfunction f<T>(x: T): T { return x as T }\nconst g = <T,>(x: T) => x\n";
  let expected = "interface A {\n  b: string;\n  c?: number;\n}\ntype U = \"a\" | \"b\";\nfunction f<T>(x: T): T {\n  return x as T;\n}\nconst g = <T,>(x: T) => x;\n";
  let options = Options { dialect: Dialect::Ts, ..Options::default() };
  assert_eq!(format(text, "", &options), expected);
  assert_eq!(format(text, "a.tsx", &options), expected);
  assert!(js::format(text, "a.ts", &Options::default()).is_err());
}

#[test]
fn keeps_the_bom_and_prints_nothing_for_white_space() {
  let options = Options::default();
  assert_eq!(format("\u{FEFF} let  x = 1\r\n", "a.js", &options), "\u{FEFF}let x = 1;\n");
  assert_eq!(format("  \n\n ", "a.js", &options), "");
  let options = Options { eol: doc::Eol::Crlf, ..options };
  assert_eq!(format("a\nb\n", "a.js", &options), "a;\r\nb;\r\n");
}

#[test]
fn finds_scripts_by_name() {
  let of = |path| Language::of_path(path);
  assert_eq!(of("src/a.mjs"), Some(Language::Js(Dialect::Js)));
  assert_eq!(of("Jakefile"), Some(Language::Js(Dialect::Js)));
  assert_eq!(of("a.start.frag"), Some(Language::Js(Dialect::Js)));
  assert_eq!(of("a.d.ts"), Some(Language::Js(Dialect::Ts)));
  assert_eq!(of("A.TSX"), Some(Language::Js(Dialect::Ts)));
  assert_eq!(of("a.js.flow"), None);
}

#[test]
fn formats_through_the_language() {
  let mut options = tidy::Options::default();
  options.share(&tidy::Shared {
    tabs: Some(true),
    trailing_comma: Some(false),
    ..Default::default()
  });
  let text = "foo(function() { return [aaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccc] })\n";
  let expected = "foo(function () {\n\treturn [\n\t\taaaaaaaaaaaaaaaaaaaa,\n\t\tbbbbbbbbbbbbbbbbbbbbbbbb,\n\t\tcccccccccccccccccccccccc\n\t];\n});\n";
  let language = Language::Js(Dialect::Js);
  let output = tidy::format(text, language, &options, Some("a.js"), &mut Verbatim).unwrap();
  assert_eq!(output, expected);
}

#[test]
fn formats_what_prettier_parsers_recover_from() {
  let options = Options { dialect: Dialect::Ts, ..Options::default() };
  let text = "declare module m { class C { field = \"f\" } }\n";
  let expected = "declare module m {\n  class C {\n    field = \"f\";\n  }\n}\n";
  assert_eq!(format(text, "a.ts", &options), expected);
  let text = "class C { public abstract readonly readonly x }\n";
  assert_eq!(format(text, "a.ts", &options), "class C {\n  public abstract readonly x;\n}\n");
  assert_eq!(
    format("const a\nlet b; let b\n", "a.js", &Options::default()),
    "const a;\nlet b;\nlet b;\n"
  );
}

#[test]
fn rejects_what_the_tree_cannot_hold() {
  let options = Options { dialect: Dialect::Ts, ..Options::default() };
  assert!(js::format("class C { declare accessor a: number }\n", "a.ts", &options).is_err());
  assert!(js::format("class C { abstract m() {} }\n", "a.ts", &options).is_err());
}

#[test]
fn prints_prettier_fixed_point_for_comments_in_parentheses() {
  let cases = [
    ("x = (a ? b : c /* a */);\n", "x = a ? b : c; /* a */\n"),
    (
      "f((a & (16 /* b */ | 32 /* c */)) !== 32);\n",
      "f((a & (16 /* b */ | 32)) /* c */ !== 32);\n",
    ),
    ("if ((0, 0 /* c */)) {\n}\n", "if ((0, 0) /* c */) {\n}\n"),
    ("!(x /* c */);\n", "!(x /* c */);\n"),
    ("const x = (a, b /* c */);\n", "const x = (a, b /* c */);\n"),
  ];
  for (text, expected) in cases {
    let once = format(text, "a.js", &Options::default());
    assert_eq!(once, expected);
    assert_eq!(format(&once, "a.js", &Options::default()), once);
  }
}

#[test]
fn prints_prettier_fixed_point_for_comments_before_links() {
  let ts = Options { dialect: Dialect::Ts, ..Options::default() };
  let cases = [
    (
      "a = b\n  .c()[0]\n  .d\n\n  // e\n  .f(g)\n  .h(i);\n",
      "a = b\n  .c()[0]\n  .d // e\n  .f(g)\n  .h(i);\n",
    ),
    ("foo\n// a\n<string>(1);\n", "foo // a\n<string>(1);\n"),
    ("foo/* b */\n<string>(1);\n", "foo/* b */ <string>(1);\n"),
    ("foo\n/* c */\n<string>();\n", "foo/* c */ <string>();\n"),
  ];
  for (text, expected) in cases {
    let once = format(text, "a.ts", &ts);
    assert_eq!(once, expected);
    assert_eq!(format(&once, "a.ts", &ts), once);
  }
}

#[test]
fn prints_prettier_fixed_point_for_comments_before_branches() {
  let text = "y = a ? b : (\n  /* c */\n  d\n);\n";
  let once = format(text, "a.js", &Options::default());
  assert_eq!(once, "y = a ? b : /* c */ d;\n");
  assert_eq!(format(&once, "a.js", &Options::default()), once);
}
