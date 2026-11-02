//! GraphQL, with outputs that Prettier 3.9.9 prints, where tidy agrees with it. `tests/oracle/graphql.ts` compares the
//! rest with Prettier.

use tidy::doc::Eol;
use tidy::graphql::{Options, format, is_graphql_path};

fn tidy(text: &str) -> String {
  tidy_with(text, &Options::default())
}

fn tidy_with(text: &str, options: &Options) -> String {
  let formatted = format(text, options).unwrap();
  assert_eq!(format(&formatted, options).unwrap(), formatted, "formatted again: {text:?}");
  formatted
}

#[test]
fn knows_graphql_paths() {
  for path in ["a/schema.graphql", "Query.GQL", "api.graphqls"] {
    assert!(is_graphql_path(path), "{path}");
  }
  for path in ["graphql.json", "a.graphql.md", "gql"] {
    assert!(!is_graphql_path(path), "{path}");
  }
}

#[test]
fn formats_operations() {
  let text = "query Q($id: ID!, $n: Int = 10) @live { user(id: $id) { name, friends(first: $n) { ...F } } }\n";
  let expected = "query Q($id: ID!, $n: Int = 10) @live {\n  user(id: $id) {\n    name\n    friends(first: $n) {\n      ...F\n    }\n  }\n}\n";
  assert_eq!(tidy(text), expected);
}

#[test]
fn breaks_arguments_past_the_width() {
  let options = Options { width: 30, ..Options::default() };
  let text = "{ search(text: \"needle\", first: 10, after: $cursor) { id } }\n";
  let expected = "{\n  search(\n    text: \"needle\"\n    first: 10\n    after: $cursor\n  ) {\n    id\n  }\n}\n";
  assert_eq!(tidy_with(text, &options), expected);
}

#[test]
fn formats_schemas() {
  let text = "\"A user\" type User implements Node&Actor @key(fields:\"id\") { id: ID!\n\n  name(full: Boolean = false): String }\nunion Result = | User | Error\n";
  let expected = "\"A user\"\ntype User implements Node & Actor @key(fields: \"id\") {\n  id: ID!\n\n  name(full: Boolean = false): String\n}\nunion Result = User | Error\n";
  assert_eq!(tidy(text), expected);
}

#[test]
fn keeps_comments_and_ignored_nodes() {
  let text = "# top\nquery {\n  a # after a\n  # prettier-ignore\n  b(x:   1)\n}\n";
  let expected = "# top\nquery {\n  a # after a\n  # prettier-ignore\n  b(x:   1)\n}\n";
  assert_eq!(tidy(text), expected);
}

#[test]
fn keeps_the_value_of_block_strings() {
  let cases = [
    // Prettier trims a block string of one line, which changes its value.
    ("\"\"\" padded \"\"\"\nscalar A\n", "\"\"\" padded \"\"\"\nscalar A\n"),
    // Prettier drops the spaces at the end of a line, which are part of the value.
    (
      "\"\"\"\nends in a space \nlast\n\"\"\"\nscalar A\n",
      "\"\"\"\nends in a space \nlast\n\"\"\"\nscalar A\n",
    ),
    ("\"\"\"\n    one\n  two\n\"\"\"\nscalar A\n", "\"\"\"\n  one\ntwo\n\"\"\"\nscalar A\n"),
  ];
  for (text, expected) in cases {
    assert_eq!(tidy(text), expected, "{text:?}");
  }
}

#[test]
fn writes_the_options() {
  let options =
    Options { bracket_spacing: false, tabs: true, eol: Eol::Crlf, ..Options::default() };
  assert_eq!(
    tidy_with("{ a(b: {c: 1}) { d } }", &options),
    "{\r\n\ta(b: {c: 1}) {\r\n\t\td\r\n\t}\r\n}\r\n"
  );
}

#[test]
fn rejects_what_graphql_js_rejects() {
  for text in ["query {", "{ a(b: 01) }", "extend type A", "# only a comment\n"] {
    assert!(format(text, &Options::default()).is_err(), "{text:?}");
  }
  assert_eq!(tidy(" \n\t\n"), "");
}

#[test]
fn formats_graphql_in_markdown() {
  let text = "# API\n\n```graphql\nquery{a}\n```\n";
  let expected = "# API\n\n```graphql\nquery {\n  a\n}\n```\n";
  let options = tidy::Options::default();
  let printed =
    tidy::format(text, tidy::Language::Markdown, &options, None, &mut tidy::embed::Verbatim)
      .unwrap();
  assert_eq!(printed, expected);
}
