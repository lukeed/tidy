//! The JSON Schema of a `tidy.json`, for editors and for the playground.
//!
//! Every option here must be one the parser in [`crate::config`] accepts;
//! `cargo test --test schema` reads that parser and fails on a key that only
//! one of the two knows.

use serde_json::{Map, Value, json};

/// What a key accepts, and what tidy does without it.
pub struct Opt {
  pub name: &'static str,
  pub kind: Kind,
  pub default: Value,
  pub about: &'static str,
}

pub enum Kind {
  Flag,
  Count,
  Text,
  /// One of these strings.
  Choice(&'static [&'static str]),
  /// One of these strings, or a flag.
  ChoiceOrFlag(&'static [&'static str]),
  /// A list of strings.
  List,
}

const fn opt(name: &'static str, kind: Kind, default: Value, about: &'static str) -> Opt {
  Opt { name, kind, default, about }
}

fn flag(name: &'static str, default: bool, about: &'static str) -> Opt {
  opt(name, Kind::Flag, Value::Bool(default), about)
}

fn count(name: &'static str, default: u64, about: &'static str) -> Opt {
  opt(name, Kind::Count, Value::from(default), about)
}

fn choice(
  name: &'static str,
  options: &'static [&'static str],
  default: &'static str,
  about: &'static str,
) -> Opt {
  opt(name, Kind::Choice(options), Value::from(default), about)
}

/// The options every language takes, at the top level and in any section.
pub fn shared() -> Vec<Opt> {
  vec![
    count("width", 80, "The line width the printer fits to. 120 for Dockerfiles."),
    count("indent", 2, "The width of one indent."),
    flag("tabs", false, "Indent with tabs."),
    choice("eol", &["lf", "crlf", "cr"], "lf", "The line ending to write."),
    choice(
      "whitespace",
      &["strict", "auto", "ignore"],
      "auto",
      "Whitespace sensitivity in HTML, Vue, Handlebars and XML. `strict` for XML.",
    ),
    choice(
      "quote",
      &["double", "single"],
      "double",
      "Quotes in Handlebars, style sheets, JSON5, YAML and JS.",
    ),
    flag(
      "trailingComma",
      true,
      "Trailing commas in SCSS, JSONC, JSON5, YAML flow collections, TOML arrays and JS.",
    ),
    flag(
      "bracketSpacing",
      true,
      "Spaces inside the braces of JSON and JS objects, and YAML flow mappings.",
    ),
    flag("bracketSameLine", false, "Keep the `>` of a multi-line tag on the last line."),
    flag("singleAttributePerLine", false, "Put each attribute on its own line."),
    opt("ignore", Kind::Text, Value::from("prettier-ignore"), "The name of the ignore directive."),
    flag("embedded", true, "Format the code of other languages inside a file."),
  ]
}

/// The options of one language, beyond the shared ones.
pub fn section(name: &str) -> Vec<Opt> {
  match name {
    "js" => vec![
      flag("semi", true, "End statements with a semicolon."),
      flag("jsxSingleQuote", false, "Single quotes in JSX attributes."),
      choice(
        "quoteProps",
        &["as-needed", "consistent", "preserve"],
        "as-needed",
        "When to quote the keys of an object.",
      ),
      opt(
        "trailingComma",
        Kind::ChoiceOrFlag(&["all", "es5", "none"]),
        Value::from("all"),
        "Where to leave a trailing comma.",
      ),
      choice(
        "arrowParens",
        &["always", "avoid"],
        "always",
        "Parentheses around a lone arrow function parameter.",
      ),
      choice(
        "objectWrap",
        &["preserve", "collapse"],
        "preserve",
        "Keep an object expanded when the source had a line break after `{`.",
      ),
      flag("experimentalTernaries", false, "Prettier's experimental ternary layout."),
      choice(
        "experimentalOperatorPosition",
        &["start", "end"],
        "end",
        "Which side of a line break a binary operator sits on.",
      ),
      flag("jsdoc", true, "Format JSDoc comments, as prettier-plugin-jsdoc does."),
      flag("sortImports", false, "Sort imports, with oxfmt's sorter."),
      opt(
        "plugins",
        Kind::List,
        json!([]),
        "Import sorters to follow: @trivago/prettier-plugin-sort-imports, @ianvs/prettier-plugin-sort-imports or prettier-plugin-organize-imports.",
      ),
    ],
    "json" => vec![
      choice(
        "quoteProps",
        &["as-needed", "consistent", "preserve"],
        "as-needed",
        "When to quote the keys of an object.",
      ),
      choice(
        "objectWrap",
        &["preserve", "collapse"],
        "preserve",
        "Keep an object expanded when the source had a line break after `{`.",
      ),
    ],
    "yaml" => {
      vec![choice("wrap", &["preserve", "always", "never"], "preserve", "How to wrap prose.")]
    }
    "vue" => vec![flag("indentBlocks", false, "Indent the contents of `<script>` and `<style>`.")],
    "handlebars" => vec![choice(
      "dialect",
      &["auto", "handlebars", "tempura"],
      "auto",
      "Which mustache grammar to parse.",
    )],
    "xml" => vec![
      flag("selfClosingSpace", true, "A space before the `/>` of a self-closing tag."),
      flag("sortAttributes", false, "Sort attributes by name."),
      choice(
        "attributeQuotes",
        &["preserve", "single", "double"],
        "preserve",
        "Which quotes to write around an attribute value.",
      ),
    ],
    "toml" => vec![
      flag("alignEntries", false, "Align the values of consecutive entries."),
      flag("alignComments", true, "Align the comments after consecutive entries."),
      flag("alignSingleComments", true, "Align a comment that stands alone."),
      flag("arrayAutoExpand", true, "Expand an array that does not fit the line width."),
      flag("arrayAutoCollapse", true, "Collapse an array that fits the line width."),
      flag("inlineTableExpand", true, "Expand an inline table onto several lines."),
      flag("compactArrays", true, "No spaces inside the brackets of an array."),
      flag("compactInlineTables", false, "No spaces inside the braces of an inline table."),
      flag("compactEntries", false, "No spaces around the `=` of an entry."),
      flag("indentTables", false, "Indent the entries of a table."),
      flag("indentEntries", false, "Indent entries under their table header."),
      flag("trailingNewline", true, "End the file with a line break."),
      flag("reorderKeys", false, "Sort the keys of each table."),
      flag("reorderArrays", false, "Sort the values of each array."),
      flag("reorderInlineTables", false, "Sort the keys of each inline table."),
      count("allowedBlankLines", 2, "How many blank lines to keep in a row."),
    ],
    "dockerfile" => vec![
      flag("indentStages", false, "Indent the instructions of each stage."),
      flag("healthcheckCmdNewLine", false, "Put a HEALTHCHECK's CMD on its own line."),
    ],
    "ini" => vec![flag("spaceAroundEquals", true, "Spaces around the `=` of an entry.")],
    "markdown" => vec![
      choice(
        "preset",
        &["prettier", "preserve"],
        "prettier",
        "A starting set of the marker options below.",
      ),
      choice("wrap", &["preserve", "always", "never"], "preserve", "How to wrap prose."),
      choice(
        "bullet",
        &["keep", "dash", "asterisk", "plus"],
        "dash",
        "The marker of a bullet list item.",
      ),
      choice(
        "emphasis",
        &["keep", "underscore", "asterisk"],
        "underscore",
        "The delimiter of emphasis.",
      ),
      choice(
        "strong",
        &["keep", "asterisk", "underscore"],
        "asterisk",
        "The delimiter of strong emphasis.",
      ),
      choice("numbering", &["keep", "auto"], "auto", "The numbers of ordered list items."),
      choice(
        "rule",
        &["keep", "dash", "asterisk", "underscore"],
        "dash",
        "The characters of a thematic break.",
      ),
      choice("fence", &["keep", "backtick", "tilde"], "backtick", "The fence of a code block."),
      choice("heading", &["keep", "atx"], "keep", "Turn setext headings into `#` prefixes."),
    ],
    _ => Vec::new(),
  }
}

/// Every language section, in the order the docs list them.
pub const SECTIONS: &[&str] = &[
  "html",
  "vue",
  "handlebars",
  "css",
  "json",
  "yaml",
  "toml",
  "xml",
  "markdown",
  "dockerfile",
  "ini",
  "graphql",
  "js",
];

fn property(option: &Opt) -> Value {
  let mut schema = Map::new();
  match &option.kind {
    Kind::Flag => {
      schema.insert("type".into(), "boolean".into());
    }
    Kind::Count => {
      schema.insert("type".into(), "integer".into());
      schema.insert("minimum".into(), 0.into());
    }
    Kind::Text => {
      schema.insert("type".into(), "string".into());
    }
    Kind::Choice(options) => {
      schema.insert("type".into(), "string".into());
      schema.insert("enum".into(), json!(options));
    }
    Kind::ChoiceOrFlag(options) => {
      schema.insert("anyOf".into(), json!([{ "enum": options }, { "type": "boolean" }]));
    }
    Kind::List => {
      schema.insert("type".into(), "array".into());
      schema.insert("items".into(), json!({ "type": "string" }));
    }
  }
  schema.insert("description".into(), option.about.into());
  schema.insert("default".into(), option.default.clone());
  Value::Object(schema)
}

fn properties(options: &[Opt]) -> Map<String, Value> {
  options.iter().map(|option| (option.name.to_owned(), property(option))).collect()
}

/// The JSON Schema of a `tidy.json`, as pretty-printed JSON.
///
/// # Example
/// ```
/// let text = tidy::schema::schema();
/// assert!(text.contains("\"printWidth\"") == false);
/// assert!(text.contains("\"width\""));
/// ```
pub fn schema() -> String {
  let mut root = properties(&shared());

  for name in SECTIONS {
    // A section takes the shared options too, with its own on top.
    let mut inner = properties(&shared());
    inner.extend(properties(&section(name)));
    root.insert(
      (*name).to_owned(),
      json!({
        "type": "object",
        "description": format!("Options for {name} only."),
        "properties": inner,
        "additionalProperties": false,
      }),
    );
  }

  let document = json!({
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$id": "https://tidy.lukeed.com/tidy.schema.json",
    "title": "tidy",
    "description": "Options for tidy, a code formatter with Prettier's style. https://tidy.lukeed.com/docs/options/",
    "type": "object",
    "properties": root,
    "additionalProperties": false,
  });

  serde_json::to_string_pretty(&document).unwrap_or_default()
}
