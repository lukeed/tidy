//! What a hook is told about each piece of embedded code: its language, what it is, the framework whose rules it
//! follows, and whether it sits in an attribute.

use tidy::embed::{Kind, Request};
use tidy::{Language, Options};

/// The requests a hook that keeps each piece as written sees while `text` is formatted as the file at `path`, each as
/// `lang/kind[/host][/attribute]`, in order, once each.
fn seen(text: &str, path: &str) -> Vec<String> {
  let mut seen: Vec<String> = Vec::new();
  let mut hook = |request: Request<'_>| -> Option<String> {
    let kind = match request.kind {
      Kind::Block => "block",
      Kind::Expression => "expression",
      Kind::Event => "event",
      Kind::Bindings => "bindings",
      Kind::For => "for",
      Kind::Generic => "generic",
      Kind::Directive => "directive",
      Kind::FrontMatter => "frontmatter",
    };
    let mut name = format!("{}/{kind}", request.lang);
    if let Some(host) = request.host {
      name = format!("{name}/{host}");
    }
    if request.attribute {
      name.push_str("/attribute");
    }
    if !seen.contains(&name) {
      seen.push(name);
    }
    Some(request.code.to_owned())
  };
  let language = Language::of_path(path).expect("a known language");
  let _ = tidy::format(text, language, &Options::default(), Some(path), &mut hook);
  seen
}

#[test]
fn names_the_code_of_a_vue_file() {
  let vue = "<template>\n<div :a=\"x | f\" @click=\"go()\" v-for=\"(item, i) in list\" v-slot=\"{ y }\">{{ msg }}</div>\n</template>\n<script setup lang=\"ts\" generic=\"T\">const a = 1</script>\n<style scoped>a{color:red}</style>\n";
  assert_eq!(
    seen(vue, "a.vue"),
    [
      "ts/expression/vue/attribute",
      "ts/expression/attribute",
      "ts/for/vue/attribute",
      "ts/bindings/vue/attribute",
      "ts/expression/vue",
      "ts/generic/vue/attribute",
      "ts/block",
      "css/block",
    ]
  );
}

#[test]
fn names_javascript_in_a_vue_file() {
  let vue =
    "<template><p @click=\"go()\">{{ msg }}</p></template>\n<script>export default {}</script>\n";
  assert_eq!(seen(vue, "b.vue"), ["js/expression/attribute", "js/expression/vue", "js/block"]);
}

#[test]
fn names_the_statements_of_a_vue_event_handler() {
  let mut seen = Vec::new();
  let mut hook = |request: Request<'_>| {
    seen.push((request.lang.to_owned(), request.kind, request.host));
    (request.kind == Kind::Event).then(|| request.code.to_owned())
  };
  let vue = "<template><p @click=\"a(); b()\"></p></template>\n";
  let output = tidy::format(vue, Language::Vue, &Options::default(), Some("e.vue"), &mut hook);
  assert_eq!(output.expect("formats"), "<template><p @click=\"a(); b()\"></p></template>\n");
  assert_eq!(seen.last(), Some(&("js".to_owned(), Kind::Event, Some("vue"))));
}

#[test]
fn names_the_code_of_an_html_file() {
  let html = "<div style=\"color:red\" onclick=\"go()\"></div><script>let a=1</script><script type=\"application/json\">{\"a\":1}</script>\n";
  assert_eq!(
    seen(html, "c.html"),
    ["css/block/attribute", "js/block/attribute", "js/block", "json/block"]
  );
}

#[test]
fn names_the_code_blocks_of_markdown() {
  let markdown = "---\ntitle: x\n---\n\n```ts\nlet a=1\n```\n\n```css\na{}\n```\n";
  assert_eq!(seen(markdown, "d.md"), ["yaml/block", "ts/block", "css/block"]);
}

#[test]
fn a_hook_can_replace_what_it_is_given() {
  let mut hook = |request: Request<'_>| (request.lang == "css").then(|| "/* hooked */".to_owned());
  let html = "<style>a{}</style>\n";
  let output = tidy::format(html, Language::Html, &Options::default(), Some("a.html"), &mut hook);
  assert_eq!(output.expect("formats"), "<style>\n  /* hooked */\n</style>\n");
}

#[test]
fn never_asks_the_hook_when_embedded_formatting_is_off() {
  let cases = [
    ("<div onclick=\"go()\" style=\"color:red\"><script>let  a</script></div>\n", "a.html"),
    (
      "<template><p :a=\"b\" @click=\"c()\">{{ d }}</p></template>\n<script>let  a</script>\n",
      "a.vue",
    ),
    ("---\na:  1\n---\n{{x}}\n", "a.hbs"),
    ("---\na:  1\n---\na{}\n", "a.css"),
    ("<svg><style type=\"text/css\">a{}</style></svg>\n", "a.xml"),
    ("```js\nlet  a\n```\n", "a.md"),
  ];
  let mut off = Options::default();
  off.share(&tidy::Shared { format_embedded: Some(false), ..Default::default() });
  let asked = |text: &str, path: &str, options: &Options| {
    let mut asked = Vec::new();
    let mut hook = |request: Request<'_>| {
      asked.push(request.code.to_owned());
      None
    };
    let language = Language::of_path(path).expect("a known language");
    let output = tidy::format(text, language, options, Some(path), &mut hook);
    assert!(output.is_ok(), "{path}: {output:?}");
    asked
  };
  for (text, path) in cases {
    assert_ne!(asked(text, path, &Options::default()), Vec::<String>::new(), "{path}, on");
    assert_eq!(asked(text, path, &off), Vec::<String>::new(), "{path}, off");
  }
}
