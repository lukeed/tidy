//! Prettier's options, and oxfmt's, which are a superset of them, as tidy's.

use serde_json::{Map, Value};
use tidy::Language;

use super::{Draft, Tool};

/// The plugins that tidy follows, and the value of `sortImports` that picks each sorter.
const SORTERS: [(&str, &str); 3] = [
  ("@trivago/prettier-plugin-sort-imports", "trivago"),
  ("@ianvs/prettier-plugin-sort-imports", "ianvs"),
  ("prettier-plugin-organize-imports", "organize-imports"),
];

/// `options`, a config of `tool`, as a tidy config, with a note for each option that doesn't carry over.
pub(super) fn migrate(options: &Map<String, Value>, tool: Tool) -> (Draft, Vec<String>) {
  let mut migration = Migration { draft: Draft::default(), notes: Vec::new() };
  let plugins = strings(options.get("plugins"));
  let sorter = (plugins.iter())
    .find_map(|plugin| SORTERS.iter().find(|it| plugin.ends_with(it.0)).map(|it| it.1));
  for (key, value) in options {
    migration.option(None, key, value, tool, sorter);
  }
  for plugin in &plugins {
    let known =
      SORTERS.iter().any(|it| plugin.ends_with(it.0)) || plugin.ends_with("prettier-plugin-jsdoc");
    if !known {
      migration.note(format!("the plugin `{plugin}`: tidy runs no plugins"));
    }
  }
  if let Some(sorter) = sorter {
    migration.draft.set(Some("js"), "sortImports", Value::from(sorter));
  }
  let jsdoc = match tool {
    Tool::Prettier => plugins.iter().any(|it| it.ends_with("prettier-plugin-jsdoc")),
    Tool::Oxfmt => options.get("jsdoc").is_some_and(|it| it != &Value::Bool(false)),
  };
  if !jsdoc {
    migration.draft.set(Some("js"), "jsdoc", Value::Bool(false));
  }
  if tool == Tool::Oxfmt {
    if !options.contains_key("printWidth") {
      migration.draft.set(None, "width", Value::from(100));
    }
    if options.get("sortPackageJson") != Some(&Value::Bool(false)) {
      migration
        .note("`sortPackageJson`: tidy keeps the keys of package.json in their order".into());
    }
  }
  (migration.draft, migration.notes)
}

struct Migration {
  draft: Draft,
  notes: Vec<String>,
}

impl Migration {
  fn note(&mut self, note: String) {
    self.notes.push(note);
  }

  /// Carries `key` over to `section`, or to the top level. `section` is set inside an override.
  fn option(
    &mut self,
    section: Option<&str>,
    key: &str,
    value: &Value,
    tool: Tool,
    sorter: Option<&str>,
  ) {
    let (name, language): (&str, Option<&str>) = match key {
      "$schema" | "plugins" => return,
      "printWidth" => ("width", None),
      "tabWidth" => ("indent", None),
      "useTabs" => ("tabs", None),
      "bracketSpacing" | "bracketSameLine" | "singleAttributePerLine" => (key, None),
      "jsxBracketSameLine" => ("bracketSameLine", None),
      "singleQuote" => {
        let quote = if value == &Value::Bool(true) { "single" } else { "double" };
        return self.set(section, None, "quote", Value::from(quote), key);
      }
      "endOfLine" if value == "auto" => {
        return self
          .note("`endOfLine: \"auto\"`: tidy writes the line ending that `eol` names".into());
      }
      "endOfLine" => ("eol", None),
      "htmlWhitespaceSensitivity" if value == "css" => return,
      "htmlWhitespaceSensitivity" => {
        return self.each(section, &["html", "vue", "handlebars"], "whitespace", value, key);
      }
      "embeddedLanguageFormatting" => {
        return self.set(section, None, "embedded", Value::Bool(value != "off"), key);
      }
      "trailingComma" if value == "none" => {
        return self.set(section, None, "trailingComma", Value::Bool(false), key);
      }
      "trailingComma" if value == "es5" => ("trailingComma", Some("js")),
      "trailingComma" => return,
      "semi"
      | "jsxSingleQuote"
      | "quoteProps"
      | "arrowParens"
      | "experimentalTernaries"
      | "experimentalOperatorPosition" => (key, Some("js")),
      "objectWrap" => return self.each(section, &["js", "json"], "objectWrap", value, key),
      "proseWrap" => return self.each(section, &["markdown", "yaml"], "wrap", value, key),
      "vueIndentScriptAndStyle" => ("indentBlocks", Some("vue")),
      "overrides" => return self.overrides(value, tool, sorter),
      "sortImports" if tool == Tool::Oxfmt && value == &Value::Bool(false) => return,
      "sortImports" if tool == Tool::Oxfmt => {
        self.note("`sortImports`: tidy sorts as `bun fmt` does: also the names inside braces, merging the imports of a module, and never across a side-effect import".into());
        ("sortImports", Some("js"))
      }
      "jsdoc" if tool == Tool::Oxfmt => {
        if value.is_object() {
          self.note("the options of `jsdoc`: tidy formats JSDoc one way".into());
        }
        return;
      }
      _ if key.starts_with("importOrder") || key.starts_with("organizeImports") => {
        let plugin = SORTERS.iter().find(|it| Some(it.1) == sorter);
        let reads = |name: &str| match key.starts_with("organizeImports") {
          true => name == "organize-imports",
          false => name != "organize-imports",
        };
        if plugin.is_none_or(|it| !reads(it.1)) {
          return self.note(format!("`{key}`: no plugin in `plugins` reads it"));
        }
        (key, Some("js"))
      }
      "sortPackageJson" if tool == Tool::Oxfmt => return,
      "insertFinalNewline" if tool == Tool::Oxfmt => {
        if value == &Value::Bool(false) {
          self.note("`insertFinalNewline: false`: tidy ends each file with a line break".into());
        }
        return;
      }
      "ignorePatterns" => {
        return self
          .note("`ignorePatterns`: tidy skips what .gitignore lists; move them there".into());
      }
      _ => return self.note(format!("`{key}`: tidy has no such option")),
    };
    self.set(section, language, name, value.clone(), key)
  }

  /// Sets `name` in `section`, the override's language, or in `language`, the language that the option is for.
  fn set(
    &mut self,
    section: Option<&str>,
    language: Option<&str>,
    name: &str,
    value: Value,
    key: &str,
  ) {
    match (section, language) {
      (Some(section), Some(language)) if section != language => {
        self.note(format!("`{key}` in the override for {section}: it is for {language} only"));
      }
      (section, language) => self.draft.set(section.or(language), name, value),
    }
  }

  /// Sets `name` in each of `languages`, or only in `section` inside an override for one of them.
  fn each(
    &mut self,
    section: Option<&str>,
    languages: &[&str],
    name: &str,
    value: &Value,
    key: &str,
  ) {
    match section {
      None => languages.iter().for_each(|it| self.draft.set(Some(it), name, value.clone())),
      Some(section) if languages.contains(&section) => {
        self.draft.set(Some(section), name, value.clone())
      }
      Some(section) => {
        self.note(format!(
          "`{key}` in the override for {section}: it is for {} only",
          languages.join(" and ")
        ));
      }
    }
  }

  /// Each override whose files are of one language, as that language's section.
  fn overrides(&mut self, value: &Value, tool: Tool, sorter: Option<&str>) {
    for item in value.as_array().into_iter().flatten() {
      let files = strings(item.get("files"));
      let shown = files.join(", ");
      if item.get("excludeFiles").is_some() {
        self.note(format!("the override for `{shown}`: tidy has no `excludeFiles`"));
        continue;
      }
      let sections: Vec<Option<&str>> = files.iter().map(|it| section_of(it)).collect();
      let Some(Some(section)) =
        sections.first().copied().filter(|first| sections.iter().all(|it| it == first))
      else {
        self.note(format!(
          "the override for `{shown}`: tidy's sections are per language, not per file"
        ));
        continue;
      };
      for (key, value) in item.get("options").and_then(Value::as_object).into_iter().flatten() {
        self.option(Some(section), key, value, tool, sorter);
      }
    }
  }
}

/// The section of the files `pattern` matches, if they are all of one language: `*.md`, `**/*.{yml,yaml}`.
fn section_of(pattern: &str) -> Option<&'static str> {
  let name = pattern.strip_prefix("**/").unwrap_or(pattern).strip_prefix("*.")?;
  let extensions: Vec<&str> = match name.strip_prefix('{').and_then(|it| it.strip_suffix('}')) {
    Some(list) => list.split(',').collect(),
    None => vec![name],
  };
  let sections: Vec<&str> = extensions
    .iter()
    .map(|it| Language::of_path(&format!("file.{it}")).map(section))
    .collect::<Option<_>>()?;
  sections.first().copied().filter(|first| sections.iter().all(|it| it == first))
}

fn section(language: Language) -> &'static str {
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

/// A string, or an array of strings.
fn strings(value: Option<&Value>) -> Vec<String> {
  match value {
    Some(Value::String(it)) => vec![it.clone()],
    Some(Value::Array(items)) => {
      items.iter().filter_map(Value::as_str).map(str::to_owned).collect()
    }
    _ => Vec::new(),
  }
}
