#![cfg(feature = "js")]

use tidy::embed::{Request, Verbatim};
use tidy::{Language, Options};

fn format(text: &str, language: Language) -> String {
  tidy::format(text, language, &Options::default(), None, &mut Verbatim).unwrap()
}

#[test]
fn formats_scripts_and_event_handlers_in_html() {
  let source = "<body>\n<button onclick=\"doSomething({a:1}); return false\">x</button>\n<script>\n  const x = {a:1,b:2}\n  function f(){ return `\n  template` }\n</script>\n<script type=\"module\">\nimport a from \"b\"\nconsole.log( a )\n</script>\n</body>\n";
  let expected = "<body>\n  <button\n    onclick=\"\n      doSomething({ a: 1 });\n      return false;\n    \"\n  >\n    x\n  </button>\n  <script>\n    const x = { a: 1, b: 2 };\n    function f() {\n      return `\n  template`;\n    }\n  </script>\n  <script type=\"module\">\n    import a from \"b\";\n    console.log(a);\n  </script>\n</body>\n";
  assert_eq!(format(source, Language::Html), expected);
}

#[test]
fn formats_vue_expressions_and_bindings() {
  let source = "<template>\n  <ul>\n    <li v-for=\"(item,index) in items\" :key=\"item.id\" @click=\"select(item); close()\">{{ someFunction(argumentNumberOne, argumentNumberTwo, argumentNumberThree, four) }}</li>\n    <comp v-slot=\"{ item }\" @update=\"(v)=>value=v\" />\n  </ul>\n</template>\n\n<script setup lang=\"ts\" generic=\"T extends string,U\">\nconst count = ref<number>(0)\n</script>\n";
  let expected = "<template>\n  <ul>\n    <li\n      v-for=\"(item, index) in items\"\n      :key=\"item.id\"\n      @click=\"\n        select(item);\n        close();\n      \"\n    >\n      {{\n        someFunction(\n          argumentNumberOne,\n          argumentNumberTwo,\n          argumentNumberThree,\n          four,\n        )\n      }}\n    </li>\n    <comp v-slot=\"{ item }\" @update=\"(v) => (value = v)\" />\n  </ul>\n</template>\n\n<script setup lang=\"ts\" generic=\"T extends string, U\">\nconst count = ref<number>(0);\n</script>\n";
  assert_eq!(format(source, Language::Vue), expected);
}

#[test]
fn places_vue_attribute_values_after_their_names() {
  let source = "<template>\n  <div v-for=\"({ a, b, c }, index) of loooooooooooooooooooooooooooooooooooooooongList\" :class=\"['aaaaaaaaaaaaaaaaaaaaaaaaaa', 'bbbbbbbbbbbbbbbbbbbbbbbbbbb', 'cccccc']\" :key=\"index // hello\">{{ a // b\n }}</div>\n</template>\n";
  let expected = "<template>\n  <div\n    v-for=\"(\n      { a, b, c }, index\n    ) of loooooooooooooooooooooooooooooooooooooooongList\"\n    :class=\"[\n      'aaaaaaaaaaaaaaaaaaaaaaaaaa',\n      'bbbbbbbbbbbbbbbbbbbbbbbbbbb',\n      'cccccc',\n    ]\"\n    :key=\"\n      index // hello\n    \"\n  >\n    {{\n      a // b\n    }}\n  </div>\n</template>\n";
  assert_eq!(format(source, Language::Vue), expected);
}

#[test]
fn formats_code_blocks_in_markdown() {
  let source = "```js\nconst a = {b:1}\n```\n\n- item\n\n  ```tsx\n  const x = <div className=\"a\">{y}</div>\n  ```\n\n```jsx\n<Foo bar=\"baz\" />\n```\n";
  let expected = "```js\nconst a = { b: 1 };\n```\n\n- item\n\n  ```tsx\n  const x = <div className=\"a\">{y}</div>;\n  ```\n\n```jsx\n<Foo bar=\"baz\" />\n```\n";
  assert_eq!(format(source, Language::Markdown), expected);
}

#[test]
fn leaves_code_that_does_not_parse() {
  let source = "<p>{{ a + }}</p>\n<script>\nlet = ;\n</script>\n";
  assert_eq!(format(source, Language::Html), "<p>{{ a + }}</p>\n<script>\n  let = ;\n</script>\n");
}

#[test]
fn prefers_the_hook() {
  let mut hook = |request: Request<'_>| (request.language == "babel").then(|| "hook()".to_owned());
  let source = "<script>\nlet  a\n</script>\n";
  let formatted = tidy::format(source, Language::Html, &Options::default(), None, &mut hook);
  assert_eq!(formatted.unwrap(), "<script>\n  hook()\n</script>\n");
}

fn format_with(text: &str, language: Language, js: tidy::js::Options) -> String {
  let options = Options { js, ..Options::default() };
  tidy::format(text, language, &options, None, &mut Verbatim).unwrap()
}

#[test]
fn follows_the_js_options_in_vue() {
  let source = "<template>\n  <p @click=\"go(a);done()\">{{ list.map(x=>x.name) }}</p>\n</template>\n\n<script setup>\nconst a = {b:1}\nfoo(a, b)\n</script>\n";
  let expected = "<template>\n  <p\n    @click=\"\n      go(a)\n      done()\n    \"\n  >\n    {{ list.map(x => x.name) }}\n  </p>\n</template>\n\n<script setup>\nconst a = {b: 1}\nfoo(a, b)\n</script>\n";
  let js = tidy::js::Options {
    semi: false,
    bracket_spacing: false,
    arrow_parens: tidy::js::ArrowParens::Avoid,
    ..tidy::js::Options::default()
  };
  assert_eq!(format_with(source, Language::Vue, js), expected);
}

#[test]
fn follows_the_js_options_in_markdown() {
  let source = "```js\nconst a = \"b\"\nf({a, b: [1,2,]})\n```\n";
  let expected = "```js\nconst a = 'b';\nf({ a, b: [1, 2] });\n```\n";
  let js = tidy::js::Options {
    single_quote: true,
    trailing_comma: tidy::js::TrailingComma::None,
    ..tidy::js::Options::default()
  };
  assert_eq!(format_with(source, Language::Markdown, js), expected);
}

#[test]
fn follows_jsdoc_in_scripts() {
  let source = "<script>\n/**\n * @param {string} a   the name\n */\nfunction f(a) {}\n</script>\n";
  let on = "<script>\n  /** @param {string} a The name */\n  function f(a) {}\n</script>\n";
  let off =
    "<script>\n  /**\n   * @param {string} a   the name\n   */\n  function f(a) {}\n</script>\n";
  assert_eq!(format(source, Language::Html), on);
  let js = tidy::js::Options { jsdoc: false, ..tidy::js::Options::default() };
  assert_eq!(format_with(source, Language::Html, js), off);
}

#[cfg(feature = "config")]
#[test]
fn takes_the_js_section_of_a_config() {
  let config = tidy::config::Config::json(r#"{ "js": { "semi": false, "quote": "single" } }"#);
  let mut options = Options::default();
  config.unwrap().apply(&mut options);
  let source = "<script>\nlet a = \"b\";\n</script>\n";
  let formatted = tidy::format(source, Language::Html, &options, None, &mut Verbatim);
  assert_eq!(formatted.unwrap(), "<script>\n  let a = 'b'\n</script>\n");
}
