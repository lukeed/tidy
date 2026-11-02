#![cfg(feature = "js")]

use tidy::js::{self, Options};

fn format(text: &str) -> String {
  js::format(text, "a.js", &Options { jsdoc: false, ..Options::default() }).unwrap()
}

fn format_ts(text: &str) -> String {
  js::format(text, "a.ts", &Options { jsdoc: false, ..Options::default() }).unwrap()
}

#[test]
fn formats_css_in_styled_components() {
  let text = "const Button = styled.button`\n  color:   ${props => props.color};\n  padding:4px  8px;\n\n\n  &:hover { background:red }\n`;\n";
  let expected = "const Button = styled.button`\n  color: ${(props) => props.color};\n  padding: 4px 8px;\n\n  &:hover {\n    background: red;\n  }\n`;\n";
  assert_eq!(format(text), expected);
}

#[test]
fn indents_css_where_the_template_is() {
  let text = "export default {\n  render() {\n    return wrap(css`display:flex;margin:0 auto;font-family:\"Helvetica Neue\", Arial, sans-serif;`);\n  },\n};\n";
  let expected = "export default {\n  render() {\n    return wrap(css`\n      display: flex;\n      margin: 0 auto;\n      font-family: \"Helvetica Neue\", Arial, sans-serif;\n    `);\n  },\n};\n";
  assert_eq!(format(text), expected);
}

#[test]
fn keeps_templates_without_embedded_formatting() {
  let text = "const a = css`color:red;`;\nconst b = gql`query { a }`;\n";
  let options = Options { format_embedded: false, ..Options::default() };
  assert_eq!(js::format(text, "a.js", &options).unwrap(), text);
}

#[test]
fn formats_graphql_between_substitutions() {
  let text = "const query = gql`\n  query User($id: ID!) { user(id: $id) { name, ...Details } }\n\n  ${DETAILS}\n  # trailing comment\n`;\nconst escaped = graphql`\n  query { field(arg: \"a \\` b\") }\n`;\n";
  let expected = "const query = gql`\n  query User($id: ID!) {\n    user(id: $id) {\n      name\n      ...Details\n    }\n  }\n\n  ${DETAILS}\n  # trailing comment\n`;\nconst escaped = graphql`\n  query {\n    field(arg: \"a \\` b\")\n  }\n`;\n";
  assert_eq!(format(text), expected);
}

#[test]
fn formats_markdown_with_and_without_indentation() {
  let text = "const doc = md`\n  # Title\n  * one\n  * two\n\n  Some \\`code\\` here.\n`;\nconst flat = markdown`\n# Heading\n+ item\n`;\n";
  let expected = "const doc = md`\n  # Title\n\n  - one\n  - two\n\n  Some \\`code\\` here.\n`;\nconst flat = markdown`\n# Heading\n\n- item\n`;\n";
  assert_eq!(format(text), expected);
}

#[test]
fn formats_html_and_its_substitutions() {
  let text = "const short = html`<p>${name}</p>`;\nconst page = html`\n  <ul class=\"list\">${items.map((item) => html`<li>${item}</li>`)}</ul>\n  <button @click=${onClick}   disabled>Click me please, this text is long enough to wrap the line</button>\n`;\n";
  let expected = "const short = html`<p>${name}</p>`;\nconst page = html`\n  <ul class=\"list\">\n    ${items.map((item) => html`<li>${item}</li>`)}\n  </ul>\n  <button @click=${onClick} disabled>\n    Click me please, this text is long enough to wrap the line\n  </button>\n`;\n";
  assert_eq!(format(text), expected);
}

#[test]
fn leaves_a_template_that_does_not_parse() {
  let text = "const a = css`color: {`;\nconst b = gql`query {`;\n";
  assert_eq!(format(text), text);
}

#[test]
fn indents_script_lines_in_html_where_the_template_is() {
  let text = "function f() {\n  return html`<div><pre>\n  a\n</pre></div><script>\n  const a = \\`x\ny\\`; foo(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)\n</script>`;\n}\n";
  let expected = "function f() {\n  return html`<div>\n      <pre>\n  a\n</pre>\n    </div>\n    <script>\n      const a = \\`x\ny\\`;\n      foo(\n        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\n        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,\n      );\n    </script>`;\n}\n";
  assert_eq!(format(text), expected);
}

#[test]
fn measures_a_substitution_of_an_outer_template_as_its_code() {
  let text = "const nestedFun = /* HTML */ `${outerExpr(1)}\n  <script>\n    const tpl = html\\`<div>\\${innerExpr(1)} ${outerExpr(2)}</div>\\`;\n  </script>`;\n";
  let expected = "const nestedFun = /* HTML */ `${outerExpr(1)}\n  <script>\n    const tpl = html\\`<div>\\${innerExpr(1)} ${outerExpr(2)}</div>\\`;\n  </script>`;\n";
  assert_eq!(format(text), expected);
}

#[test]
fn formats_angular_microsyntax() {
  let text = "@Component({\n  template: `<ng-container *ngComponentOutlet=\"componentOutlet; injector: componentInjector; content: componentContent\"></ng-container><li *ngFor=\"let item  of items;index as i; let e=even\"></li>`,\n})\nclass A {}\n";
  let expected = "@Component({\n  template: `<ng-container\n      *ngComponentOutlet=\"\n        componentOutlet;\n        injector: componentInjector;\n        content: componentContent\n      \"\n    ></ng-container>\n    <li *ngFor=\"let item of items; index as i; let e = even\"></li>`,\n})\nclass A {}\n";
  assert_eq!(format_ts(text), expected);
}

#[test]
fn keeps_line_breaks_in_texts_apart_with_crlf() {
  let text = "function f() {\r\n  return html`<pre>\r\n  a\r\n</pre><script>\r\n  const a = \\`x\r\ny\\`; foo(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)\r\n</script>`;\r\n}\r\n";
  let expected = "function f() {\r\n  return html`<pre>\r\n  a\r\n</pre>\r\n    <script>\r\n      const a = \\`x\r\ny\\`;\r\n      foo(\r\n        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\r\n        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,\r\n      );\r\n    </script>`;\r\n}\r\n";
  let options = Options { jsdoc: false, eol: tidy::doc::Eol::Crlf, ..Options::default() };
  assert_eq!(js::format(text, "a.js", &options).unwrap(), expected);
}
