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
  /// `null` where the default depends on other options.
  pub default: Value,
  pub about: &'static str,
  /// The values of `sortImports` that read this option, which it is an error to set without.
  pub needs: &'static [&'static str],
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
  /// This JSON Schema.
  Schema(Value),
}

const fn opt(name: &'static str, kind: Kind, default: Value, about: &'static str) -> Opt {
  Opt { name, kind, default, about, needs: &[] }
}

impl Opt {
  fn needs(self, needs: &'static [&'static str]) -> Opt {
    Opt { needs, ..self }
  }
}

const SORTERS: &[&str] = &["trivago", "ianvs"];
const TRIVAGO: &[&str] = &["trivago"];
const IANVS: &[&str] = &["ianvs"];
const ORGANIZE: &[&str] = &["organize-imports"];

/// `sortImports`: false, true, a Prettier plugin's sorter, or bun's with options.
fn sort_imports() -> Opt {
  let strings = json!({ "type": "array", "items": { "type": "string" } });
  let flag = |about: &str, default: bool| json!({ "type": "boolean", "description": about, "default": default });
  let selectors = [
    "type",
    "side_effect_style",
    "side_effect",
    "style",
    "index",
    "sibling",
    "parent",
    "subpath",
    "internal",
    "builtin",
    "external",
    "import",
  ];
  let modifiers = ["side_effect", "type", "value", "default", "wildcard", "named"];
  let options = json!({
    "type": "object",
    "title": "SortImports",
    "description": "Sort as `true` does, with these options.",
    "properties": {
      "groups": {
        "type": "array",
        "description": "The groups, in order: predefined ones, such as `builtin`, `external`, `internal`, `parent`, `sibling`, `index`, `style` and `unknown`, or the names of `customGroups`. An array sorts its groups as one. A `newlinesBetween` object between two groups overrides `newlinesBetween` there.",
        "items": { "anyOf": [
          { "type": "string" },
          strings,
          { "type": "object", "properties": { "newlinesBetween": { "type": "boolean" } }, "additionalProperties": false },
        ] },
        "default": ["builtin", "external", ["internal", "subpath"], ["parent", "sibling", "index"], "style", "unknown"],
      },
      "customGroups": {
        "type": "array",
        "description": "Groups of your own, which `groups` can name. An import goes to the first group it matches, before the predefined ones.",
        "items": {
          "type": "object",
          "title": "CustomGroup",
          "description": "A group of your own.",
          "properties": {
            "groupName": { "type": "string", "description": "The name that `groups` gives the group." },
            "elementNamePattern": { "type": "array", "items": { "type": "string" }, "description": "Globs that the module name must match." },
            "selector": { "enum": selectors, "description": "The kind of import." },
            "modifiers": { "type": "array", "items": { "enum": modifiers }, "description": "Properties that the import must all have." },
          },
          "additionalProperties": false,
        },
        "default": [],
      },
      "order": { "enum": ["asc", "desc"], "description": "Sort up or down.", "default": "asc" },
      "ignoreCase": flag("Ignore case when sorting.", true),
      "newlinesBetween": flag("A blank line between groups.", true),
      "internalPattern": { "type": "array", "items": { "type": "string" }, "description": "The prefixes of internal modules.", "default": ["~/", "@/", "#"] },
      "sortSideEffects": flag("Sort side-effect imports too. Otherwise, no import moves across one.", false),
      "partitionByNewline": flag("Sort the imports between blank lines apart.", false),
      "partitionByComment": flag("Sort the imports between comments apart.", false),
    },
    "additionalProperties": false,
  });
  opt(
    "sortImports",
    Kind::Schema(
      json!({ "anyOf": [{ "type": "boolean" }, { "enum": ["trivago", "ianvs", "organize-imports"] }, options] }),
    ),
    Value::Bool(false),
    "Sort imports: `true` as `bun fmt` does, or as `\"trivago\"` (`@trivago/prettier-plugin-sort-imports`), `\"ianvs\"` (`@ianvs/prettier-plugin-sort-imports`) or `\"organize-imports\"` (`prettier-plugin-organize-imports`) does. `true` never moves an import across a side-effect import.",
  )
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
    opt(
      "ignore",
      Kind::Text,
      Value::from("prettier-ignore"),
      "The name of the ignore directive. A comment with this name skips the next node. It can list the tools it applies to after the name, as `prettier-ignore format,lint`, and then applies only when `format` is one of them.",
    ),
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
      flag("jsdoc", true, "Format JSDoc comments, as `prettier-plugin-jsdoc` does."),
      flag(
        "preserveBreaks",
        false,
        "Keep argument lists, parameter lists, arrays and union types broken where the source breaks them.",
      ),
      sort_imports(),
      opt(
        "importOrder",
        Kind::List,
        Value::Null,
        "The order of the groups of imports, as regular expressions of module names. `<THIRD_PARTY_MODULES>` is the modules that match none. With `\"ianvs\"`, `<BUILTIN_MODULES>` is Node's modules, `<TYPES>` marks type imports, and `\"\"` puts a blank line between two groups.",
      )
      .needs(SORTERS),
      opt(
        "importOrderParserPlugins",
        Kind::List,
        json!(["typescript", "jsx"]),
        "The Babel plugins of the original plugin. With `\"ianvs\"`, type and value imports of a module combine only if it holds `typescript`.",
      )
      .needs(SORTERS),
      flag("importOrderSeparation", false, "A blank line between the groups of `importOrder`.").needs(TRIVAGO),
      flag("importOrderSortSpecifiers", false, "Sort the names inside braces.").needs(TRIVAGO),
      flag("importOrderGroupNamespaceSpecifiers", false, "Put namespace imports first in their group.").needs(TRIVAGO),
      flag("importOrderCaseInsensitive", false, "Ignore case when sorting.").needs(TRIVAGO),
      flag("importOrderSideEffects", true, "Sort side-effect imports too. Otherwise, no import moves across one.").needs(TRIVAGO),
      opt("importOrderSortByLength", Kind::Choice(&["asc", "desc"]), Value::Null, "Sort by the length of the import, not by name.").needs(TRIVAGO),
      choice(
        "importOrderImportAttributesKeyword",
        &["with", "assert", "with-legacy"],
        "with",
        "The keyword of import attributes.",
      )
      .needs(TRIVAGO),
      opt("importOrderExclude", Kind::List, json!([]), "Files whose imports stay as they are, as globs.").needs(TRIVAGO),
      opt(
        "importOrderTypeScriptVersion",
        Kind::Text,
        Value::from("1.0.0"),
        "The project's TypeScript version. Type and value imports of a module combine from 4.5.0.",
      )
      .needs(IANVS),
      flag("importOrderCaseSensitive", false, "Sort with case sensitivity.").needs(IANVS),
      opt(
        "importOrderSafeSideEffects",
        Kind::List,
        json!([]),
        "Side-effect imports that may move, as regular expressions of module names.",
      )
      .needs(IANVS),
      flag("organizeImportsSkipDestructiveCodeActions", false, "Keep the imports that are not used.").needs(ORGANIZE),
      opt(
        "organizeImportsTypeOrder",
        Kind::Choice(&["last", "inline", "first"]),
        Value::Null,
        "Where type imports go among the names inside braces. Default: as TypeScript does.",
      )
      .needs(ORGANIZE),
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

/// What a section formats, as its description names it.
fn title(section: &str) -> &str {
  match section {
    "html" => "HTML",
    "vue" => "Vue",
    "handlebars" => "Handlebars",
    "css" => "CSS, SCSS and Less",
    "json" => "JSON",
    "yaml" => "YAML",
    "toml" => "TOML",
    "xml" => "XML",
    "markdown" => "Markdown",
    "dockerfile" => "Dockerfiles",
    "ini" => "INI",
    "graphql" => "GraphQL",
    "js" => "JavaScript and TypeScript",
    other => other,
  }
}

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
    Kind::Schema(Value::Object(map)) => schema.extend(map.clone()),
    Kind::Schema(_) => {}
  }
  let needs: Vec<String> = option.needs.iter().map(|it| format!("`\"{it}\"`")).collect();
  let about = match needs.is_empty() {
    true => option.about.to_owned(),
    false => format!("{} Only with `sortImports` set to {}.", option.about, needs.join(" or ")),
  };
  schema.insert("description".into(), about.into());
  if !option.default.is_null() {
    schema.insert("default".into(), option.default.clone());
  }
  Value::Object(schema)
}

fn properties(options: &[Opt]) -> Map<String, Value> {
  options.iter().map(|option| (option.name.to_owned(), property(option))).collect()
}

/// The `tidy.json` section that configures `language`.
pub fn of(language: crate::Language) -> &'static str {
  use crate::Language;
  match language {
    Language::Html | Language::Mjml => "html",
    Language::Vue => "vue",
    Language::Handlebars => "handlebars",
    Language::Css(_) => "css",
    Language::Json(_) => "json",
    Language::Toml => "toml",
    Language::Xml => "xml",
    Language::Yaml => "yaml",
    Language::Markdown => "markdown",
    Language::Dockerfile => "dockerfile",
    Language::Ini(_) => "ini",
    Language::Graphql => "graphql",
    #[cfg(feature = "js")]
    Language::Js(_) => "js",
  }
}

/// Where this version's schema lives. `https://tidy.lukeed.com/schema.json` redirects to the latest.
pub fn url() -> String {
  format!("https://tidy.lukeed.com/{}/schema.json", env!("CARGO_PKG_VERSION"))
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
  root.insert(
    "$schema".into(),
    json!({ "type": "string", "description": "The JSON Schema of this file, for editors." }),
  );

  for name in SECTIONS {
    // A section takes the shared options too, with its own on top.
    let mut inner = properties(&shared());
    inner.extend(properties(&section(name)));
    root.insert(
      (*name).to_owned(),
      json!({
        "type": "object",
        "description": format!("Options for {} only.", title(name)),
        "properties": inner,
        "additionalProperties": false,
      }),
    );
  }

  let document = json!({
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$id": url(),
    "title": "tidy",
    "description": "Options for tidy, a code formatter with Prettier's style. https://tidy.lukeed.com/docs/options/",
    "type": "object",
    "properties": root,
    "additionalProperties": false,
  });

  serde_json::to_string_pretty(&document).unwrap_or_default()
}
