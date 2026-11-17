//! The config of another formatter, Prettier or oxfmt, as a tidy config, for `tidy init`.

mod options;
#[cfg(feature = "js")]
mod script;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The formatter a config belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
  Prettier,
  Oxfmt,
}

/// The config files of each formatter, in the order Prettier looks for its own.
const PRETTIER: [&str; 19] = [
  "package.json",
  ".prettierrc",
  ".prettierrc.json",
  ".prettierrc.yaml",
  ".prettierrc.yml",
  ".prettierrc.json5",
  ".prettierrc.js",
  ".prettierrc.ts",
  ".prettierrc.mjs",
  ".prettierrc.mts",
  ".prettierrc.cjs",
  ".prettierrc.cts",
  "prettier.config.js",
  "prettier.config.ts",
  "prettier.config.mjs",
  "prettier.config.mts",
  "prettier.config.cjs",
  "prettier.config.cts",
  ".prettierrc.toml",
];
const OXFMT: [&str; 4] = [".oxfmtrc.json", ".oxfmtrc.jsonc", "oxfmt.config.ts", "oxfmt.config.mts"];

/// A tidy config, as `tidy init` writes it: the shared options, then each language's section.
#[derive(Default)]
pub(crate) struct Draft {
  pub(crate) shared: Vec<(String, Value)>,
  pub(crate) sections: Vec<(String, Vec<(String, Value)>)>,
}

impl Draft {
  /// Sets `key` at the top level, or in `section`.
  pub(crate) fn set(&mut self, section: Option<&str>, key: &str, value: Value) {
    let entries = match section {
      None => &mut self.shared,
      Some(name) => match self.sections.iter().position(|it| it.0 == name) {
        Some(at) => &mut self.sections[at].1,
        None => {
          self.sections.push((name.to_owned(), Vec::new()));
          &mut self.sections.last_mut().expect("just pushed").1
        }
      },
    };
    entries.retain(|it| it.0 != key);
    entries.push((key.to_owned(), value));
  }

  /// The keys in the order of the schema, which is the order of the docs.
  pub(crate) fn sort(&mut self) {
    let rank = |section: Option<&str>, key: &str| {
      let mut names: Vec<&str> = tidy::schema::shared().iter().map(|it| it.name).collect();
      names.extend(section.map(tidy::schema::section).unwrap_or_default().iter().map(|it| it.name));
      names.iter().position(|it| *it == key).unwrap_or(usize::MAX)
    };
    self.shared.sort_by_key(|it| rank(None, &it.0));
    for (name, entries) in &mut self.sections {
      entries.sort_by_key(|it| rank(Some(name), &it.0));
    }
    let order = |name: &str| tidy::schema::SECTIONS.iter().position(|it| *it == name);
    self.sections.sort_by_key(|it| order(&it.0));
  }

  pub(crate) fn to_value(&self) -> Value {
    let mut map: Map<String, Value> = self.shared.iter().cloned().collect();
    for (name, entries) in &self.sections {
      map.insert(name.clone(), Value::Object(entries.iter().cloned().collect()));
    }
    Value::Object(map)
  }
}

/// A config of another formatter, as a tidy config, with what didn't carry over.
pub(crate) struct Migration {
  pub(crate) draft: Draft,
  pub(crate) notes: Vec<String>,
}

/// The config of another formatter in `dir`, if there is one. More than one is an error: `--from` picks.
pub(crate) fn find(dir: &Path) -> Result<Option<PathBuf>, String> {
  let found: Vec<PathBuf> = PRETTIER
    .iter()
    .chain(&OXFMT)
    .map(|name| dir.join(name))
    .filter(|path| path.is_file())
    .filter(|path| !path.ends_with("package.json") || package_config(path).is_some())
    .collect();
  match found.len() {
    0 | 1 => Ok(found.into_iter().next()),
    _ => {
      let names: Vec<String> = found.iter().map(|it| name(it)).collect();
      Err(format!("found {}: pick one with `tidy init --from <file>`", names.join(", ")))
    }
  }
}

/// The config at `path`, as a tidy config.
pub(crate) fn read(path: &Path) -> Result<Migration, String> {
  let file = name(path);
  let fail = |error: String| format!("{file}: {error}");
  let tool = if file.starts_with(".oxfmtrc") || file.starts_with("oxfmt.config") {
    Tool::Oxfmt
  } else {
    Tool::Prettier
  };
  let text = std::fs::read_to_string(path).map_err(|error| fail(error.to_string()))?;
  let (value, unread) = match file.as_str() {
    "package.json" => {
      let value = package_config(path).ok_or_else(|| fail("it has no `prettier` key".into()))?;
      (value, Vec::new())
    }
    _ => parse(&text, path).map_err(fail)?,
  };
  let options = match value {
    Value::Object(options) => options,
    Value::String(name) => {
      return Err(fail(format!(
        "it names the shared config `{name}`: pass that config's file with `--from`"
      )));
    }
    _ => return Err(fail("it isn't an object of options".into())),
  };
  let (mut draft, notes) = options::migrate(&options, tool);
  let notes = unread.into_iter().chain(notes).map(|note| format!("{file}: {note}")).collect();
  draft.sort();
  tidy::config::Config::from_value(&draft.to_value())
    .map_err(|error| fail(format!("the options don't carry over: {error}")))?;
  Ok(Migration { draft, notes })
}

/// The value of the config, and a note for each part of it that tidy can't tell.
fn parse(text: &str, path: &Path) -> Result<(Value, Vec<String>), String> {
  let extension = path.extension().and_then(|it| it.to_str()).unwrap_or_default();
  let error = |error: tidy::Error| error.to_string();
  let value = match extension {
    "json" | "json5" | "jsonc" => tidy::json::value(text).map_err(error),
    "yaml" | "yml" => tidy::yaml::value(text).map_err(error),
    "toml" => tidy::toml::value(text).map_err(error),
    "js" | "mjs" | "cjs" | "ts" | "mts" | "cts" => return script(text, path),
    _ => tidy::json::value(text).or_else(|_| tidy::yaml::value(text)).map_err(error),
  };
  value.map(|it| (it, Vec::new()))
}

#[cfg(feature = "js")]
fn script(text: &str, path: &Path) -> Result<(Value, Vec<String>), String> {
  script::value(text, path)
}

#[cfg(not(feature = "js"))]
fn script(_: &str, _: &Path) -> Result<(Value, Vec<String>), String> {
  Err("this tidy reads no JavaScript: build it with the `js` feature".into())
}

/// The `prettier` key of a `package.json`.
fn package_config(path: &Path) -> Option<Value> {
  let text = std::fs::read_to_string(path).ok()?;
  let mut value = tidy::json::value(&text).ok()?;
  value.get_mut("prettier").map(Value::take)
}

fn name(path: &Path) -> String {
  path
    .file_name()
    .map_or_else(|| path.display().to_string(), |it| it.to_string_lossy().into_owned())
}
