use tidyrs::embed::Verbatim;
use tidyrs::handlebars::{Settings, format};

fn hbs(text: &str) -> String {
  format(text, &Settings::default(), &mut Verbatim).unwrap()
}

#[test]
fn formats_a_template() {
  assert_eq!(
    hbs("<div   class=\"a  b\">\n{{#if   foo}}\n<p>{{bar baz=1   qux=\"x\"}}</p>\n{{else}}\n  nope\n{{/if}}</div>\n"),
    "<div class=\"a b\">\n  {{#if foo}}\n    <p>{{bar baz=1 qux=\"x\"}}</p>\n  {{else}}\n    nope\n  {{/if}}</div>"
  );
}
