use tidy::embed::Verbatim;
use tidy::handlebars::{Dialect, Settings, format};

fn hbs(text: &str) -> String {
  format(text, &Settings::default(), &mut Verbatim).unwrap()
}

#[test]
fn formats_a_template() {
  assert_eq!(
    hbs(
      "<div   class=\"a  b\">\n{{#if   foo}}\n<p>{{bar baz=1   qux=\"x\"}}</p>\n{{else}}\n  nope\n{{/if}}</div>\n"
    ),
    "<div class=\"a b\">\n  {{#if foo}}\n    <p>{{bar baz=1 qux=\"x\"}}</p>\n  {{else}}\n    nope\n  {{/if}}</div>"
  );
}

fn with(text: &str, settings: Settings) -> String {
  format(text, &settings, &mut Verbatim).unwrap()
}

#[test]
fn formats_tempura() {
  assert_eq!(
    hbs(
      "{{#expect  items,\n  title }}\n{{#var count=items.length;}}\n<ul>\n{{#each items as item,i}}\n<li{{#if item.done}} class=\"done\"{{/if}}>{{item.text}}</li>\n{{/each}}\n</ul>\n{{#if count == 0}}\n<p>Done</p>\n{{#elif count > 9}}\n<p>{{{ count }}} left</p>\n{{#else}}\n{{#partial   src=\"nav\"  }}\n{{/if}}\n"
    ),
    "{{#expect items, title}}\n{{#var count = items.length}}\n<ul>\n  {{#each items as item, i}}\n    <li {{#if item.done}} class=\"done\"{{/if}}>{{ item.text }}</li>\n  {{/each}}\n</ul>\n{{#if count == 0}}\n  <p>Done</p>\n{{#elif count > 9}}\n  <p>{{{ count }}} left</p>\n{{#else}}\n  {{#partial src=\"nav\"}}\n{{/if}}"
  );
}

#[test]
fn keeps_the_text_of_other_languages() {
  let settings = Settings { filepath: Some("robots.txt.hbs".into()), ..Settings::default() };
  assert_eq!(
    with(
      "User-agent: *\n      {{#if   live}}\nAllow: /\n   {{#else}}\nDisallow: /\n{{/if}}\nSitemap: {{$site.url}}sitemap.xml\n",
      settings
    ),
    "User-agent: *\n{{#if live}}\nAllow: /\n{{#else}}\nDisallow: /\n{{/if}}\nSitemap: {{ $site.url }}sitemap.xml\n"
  );
}

#[test]
fn spaces_values_in_tempura_only() {
  let handlebars = Settings { dialect: Dialect::Handlebars, ..Settings::default() };
  let tempura = Settings { dialect: Dialect::Tempura, ..Settings::default() };
  assert_eq!(with("<p>{{ name }}</p>", handlebars), "<p>{{name}}</p>");
  assert_eq!(with("<p>{{name}}</p>", tempura), "<p>{{ name }}</p>");
  assert_eq!(hbs("<p>{{name}}</p>"), "<p>{{name}}</p>");
}

#[test]
fn keeps_tempura_comments_as_written() {
  let tempura = Settings { dialect: Dialect::Tempura, ..Settings::default() };
  assert_eq!(
    with("{{!-- todo --}}\n{{!\n  many\n  lines\n!}}\n<p>{{x}}</p>", tempura),
    "{{!-- todo --}}\n{{!\n  many\n  lines\n!}}\n<p>{{ x }}</p>"
  );
}

#[test]
fn takes_another_ignore_directive() {
  let settings = Settings { ignore: "tidy-ignore".into(), ..Settings::default() };
  let text = "{{! tidy-ignore }}\n<p   a=\"b\">x</p>\n{{! prettier-ignore }}\n<p   a=\"b\">x</p>\n";
  assert_eq!(
    format(text, &settings, &mut Verbatim).unwrap(),
    "{{! tidy-ignore }}\n<p   a=\"b\">x</p>\n{{! prettier-ignore }}\n<p a=\"b\">x</p>"
  );
}
