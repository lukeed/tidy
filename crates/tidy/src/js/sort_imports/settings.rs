//! The options as they appear in a configuration file.

use super::ianvs::{self, Matcher};
use super::organize::{self, TypeOrder};
use super::regex::Regex;
use super::trivago::{self, ByLength, Exclude, Group};
use super::{How, SortImports};
use crate::js::strings;
use serde_json::Value;
use std::sync::Arc;

/// The import-related options, by name, with their JSON values. `sortImports` picks the sorter; the other options
/// belong to one of them, and it's an error to set one for another.
///
/// | `sortImports` | options |
/// | --- | --- |
/// | `true`, or an object of `groups`, `customGroups`, `order`, `ignoreCase`, `newlinesBetween`, `internalPattern`, `sortSideEffects`, `partitionByNewline` and `partitionByComment` | `bun fmt`'s: oxfmt's sorter with side-effect imports as barriers, which then sorts the names in braces and merges the imports of a module |
/// | `"trivago"` and `"ianvs"` | `importOrder`, `importOrderParserPlugins` |
/// | `"trivago"`: `@trivago/prettier-plugin-sort-imports` | `importOrderSeparation`, `importOrderSortSpecifiers`, `importOrderGroupNamespaceSpecifiers`, `importOrderCaseInsensitive`, `importOrderSideEffects`, `importOrderSortByLength`, `importOrderImportAttributesKeyword`, `importOrderExclude` |
/// | `"ianvs"`: `@ianvs/prettier-plugin-sort-imports` | `importOrderTypeScriptVersion`, `importOrderCaseSensitive`, `importOrderSafeSideEffects` |
/// | `"organize-imports"`: `prettier-plugin-organize-imports` | `organizeImportsSkipDestructiveCodeActions`, `organizeImportsTypeOrder`, and from the library only, the `compilerOptions` that plugin reads from `tsconfig.json`: `tsconfig.jsx`, `tsconfig.jsxFactory`, `tsconfig.jsxFragmentFactory`, `tsconfig.reactNamespace` |
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Settings {
  entries: Vec<(String, Value)>,
}

/// The options of both Prettier plugins that sort imports.
const SHARED_OPTIONS: [&str; 2] = ["importOrder", "importOrderParserPlugins"];

const TRIVAGO_OPTIONS: [&str; 8] = [
  "importOrderSeparation",
  "importOrderSortSpecifiers",
  "importOrderGroupNamespaceSpecifiers",
  "importOrderCaseInsensitive",
  "importOrderSideEffects",
  "importOrderSortByLength",
  "importOrderImportAttributesKeyword",
  "importOrderExclude",
];

const IANVS_OPTIONS: [&str; 3] =
  ["importOrderTypeScriptVersion", "importOrderCaseSensitive", "importOrderSafeSideEffects"];

const ORGANIZE_OPTIONS: [&str; 6] = [
  "organizeImportsSkipDestructiveCodeActions",
  "organizeImportsTypeOrder",
  "tsconfig.jsx",
  "tsconfig.jsxFactory",
  "tsconfig.jsxFragmentFactory",
  "tsconfig.reactNamespace",
];

/// Which sorter `sortImports` picks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sorter {
  Bun,
  Trivago,
  Ianvs,
  Organize,
}

impl Sorter {
  fn name(self) -> &'static str {
    match self {
      Sorter::Bun => "true",
      Sorter::Trivago => "\"trivago\"",
      Sorter::Ianvs => "\"ianvs\"",
      Sorter::Organize => "\"organize-imports\"",
    }
  }

  /// The sorters that read the option `name`.
  fn of_option(name: &str) -> &'static [Sorter] {
    match name {
      _ if SHARED_OPTIONS.contains(&name) => &[Sorter::Trivago, Sorter::Ianvs],
      _ if TRIVAGO_OPTIONS.contains(&name) => &[Sorter::Trivago],
      _ if IANVS_OPTIONS.contains(&name) => &[Sorter::Ianvs],
      _ => &[Sorter::Organize],
    }
  }
}

fn invalid(name: &str, value: &Value) -> String {
  format!("Invalid {name} value: {value}.")
}

impl Settings {
  /// Stores the option `name` if it's import-related. A later value replaces an earlier one.
  pub fn set(&mut self, name: &str, value: Value) -> bool {
    let is_known = name == "sortImports"
      || SHARED_OPTIONS.contains(&name)
      || TRIVAGO_OPTIONS.contains(&name)
      || IANVS_OPTIONS.contains(&name)
      || ORGANIZE_OPTIONS.contains(&name);
    if is_known {
      self.entries.retain(|entry| entry.0 != name);
      self.entries.push((name.to_owned(), value));
    }
    is_known
  }

  /// The value of `name`, where `null` is the same as no value.
  fn get(&self, name: &str) -> Option<&Value> {
    self.entries.iter().find(|entry| entry.0 == name && !entry.1.is_null()).map(|entry| &entry.1)
  }

  fn text(&self, name: &str) -> Option<&str> {
    self.get(name).and_then(Value::as_str)
  }

  fn boolean(&self, name: &str, default: bool) -> Result<bool, String> {
    match self.get(name) {
      None => Ok(default),
      Some(Value::Bool(value)) => Ok(*value),
      Some(value) => Err(invalid(name, value)),
    }
  }

  /// An array of strings. A single string counts as an array of one.
  fn strings(&self, name: &str) -> Result<Option<Vec<Vec<u8>>>, String> {
    let Some(value) = self.get(name) else {
      return Ok(None);
    };
    if let Value::String(text) = value {
      return Ok(Some(vec![text.as_bytes().to_vec()]));
    }
    let Some(items) = value.as_array() else {
      return Err(invalid(name, value));
    };
    let strings: Option<Vec<Vec<u8>>> =
      items.iter().map(|item| item.as_str().map(|it| it.as_bytes().to_vec())).collect();
    strings.map(Some).ok_or_else(|| invalid(name, value))
  }

  fn trivago(&self, order: Option<Vec<Vec<u8>>>) -> Result<trivago::Options, String> {
    let mut order = order.unwrap_or_default();
    if !order.iter().any(|group| group == trivago::THIRD_PARTY_MODULES) {
      order.insert(0, trivago::THIRD_PARTY_MODULES.to_vec());
    }
    let groups: Result<Vec<Group>, String> = (order.iter().enumerate())
      .map(|(index, text)| {
        Ok(Group {
          regex: regex(text.strip_prefix(trivago::TYPES).unwrap_or(text))?,
          same_as: order.iter().position(|other| other == text).unwrap_or(index),
          text: text.clone().into_boxed_slice(),
        })
      })
      .collect();
    Ok(trivago::Options {
      order: groups?,
      is_case_insensitive: self.boolean("importOrderCaseInsensitive", false)?,
      separation: self.boolean("importOrderSeparation", false)?,
      group_namespace_specifiers: self.boolean("importOrderGroupNamespaceSpecifiers", false)?,
      sort_specifiers: self.boolean("importOrderSortSpecifiers", false)?,
      sort_by_length: match self.get("importOrderSortByLength") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value == "asc" => Some(ByLength::Ascending),
        Some(Value::String(value)) if value == "desc" => Some(ByLength::Descending),
        Some(value) => return Err(invalid("importOrderSortByLength", value)),
      },
      side_effects: self.boolean("importOrderSideEffects", true)?,
      attributes_keyword: match self.get("importOrderImportAttributesKeyword") {
        None => b"with",
        Some(value) => match value.as_str() {
          Some("with") => b"with",
          Some("assert") => b"assert",
          Some("with-legacy") => b"with-legacy",
          _ => return Err(invalid("importOrderImportAttributesKeyword", value)),
        },
      },
      exclude: (self.strings("importOrderExclude")?.unwrap_or_default().iter())
        .map(|pattern| Exclude {
          glob: pattern.clone().into_boxed_slice(),
          has_slash: strings::contains_char(pattern, b'/'),
        })
        .collect(),
    })
  }

  /// `examineAndNormalizePluginOptions`. Returns `None` if `importOrder` is empty, which turns the plugin off.
  fn ianvs(&self, order: Option<Vec<Vec<u8>>>) -> Result<Option<ianvs::Options>, String> {
    let is_separator = |group: &[u8]| group.trim_ascii().is_empty();
    let mut order = order.unwrap_or_else(|| {
      vec![ianvs::BUILTIN_MODULES.to_vec(), ianvs::THIRD_PARTY_MODULES.to_vec(), b"^[.]".to_vec()]
    });
    if order.is_empty() {
      return Ok(None);
    }
    let at = usize::from(is_separator(&order[0]));
    for word in [ianvs::THIRD_PARTY_MODULES, ianvs::BUILTIN_MODULES] {
      if !order.iter().any(|group| group == word) {
        order.insert(at, word.to_vec());
      }
    }
    let groups: Result<Vec<ianvs::Group>, String> = (order.iter().enumerate())
      .map(|(index, text)| {
        Ok(ianvs::Group {
          matcher: match &text[..] {
            text if is_separator(text) => Matcher::Separator,
            ianvs::THIRD_PARTY_MODULES => Matcher::ThirdParty,
            ianvs::BUILTIN_MODULES => Matcher::Builtin,
            ianvs::TYPES => Matcher::Types,
            text => match strings::index_of(text, ianvs::TYPES) {
              Some(at) => Matcher::Regex {
                regex: Box::new(regex(&[&text[..at], &text[at + ianvs::TYPES.len()..]].concat())?),
                is_for_types: true,
              },
              None => Matcher::Regex { regex: Box::new(regex(text)?), is_for_types: false },
            },
          },
          same_as: order.iter().position(|other| other == text).unwrap_or(index),
        })
      })
      .collect();
    let has_types = order.iter().any(|group| strings::contains(group, ianvs::TYPES));
    let has_typescript = self
      .strings("importOrderParserPlugins")?
      .is_none_or(|plugins| plugins.iter().any(|it| it == b"typescript"));
    let version = self
      .text("importOrderTypeScriptVersion")
      .and_then(|it| semver(it.as_bytes()))
      .unwrap_or(((1, 0, 0), false));
    let safe_side_effects: Result<Vec<Regex>, String> = self
      .strings("importOrderSafeSideEffects")?
      .unwrap_or_default()
      .iter()
      .map(|it| regex(it))
      .collect();
    Ok(Some(ianvs::Options {
      order: groups?,
      combine_type_and_value_imports: !has_types
        && !(has_typescript && version < ((4, 5, 0), true)),
      is_case_sensitive: self.boolean("importOrderCaseSensitive", false)?,
      safe_side_effects: safe_side_effects?,
    }))
  }

  /// Returns `None` if imports are left as they are, and `Err` with a message for the user if the settings are
  /// invalid.
  pub fn compile(&self) -> Result<Option<Arc<SortImports>>, String> {
    let settings = self.clone();
    let value = self.get("sortImports").unwrap_or(&Value::Bool(false));
    let sorter = match value {
      Value::Bool(false) => None,
      Value::Bool(true) | Value::Object(_) => Some(Sorter::Bun),
      Value::String(name) if name == "trivago" => Some(Sorter::Trivago),
      Value::String(name) if name == "ianvs" => Some(Sorter::Ianvs),
      Value::String(name) if name == "organize-imports" => Some(Sorter::Organize),
      _ => {
        return Err(format!(
          "`sortImports` must be true, false, \"trivago\", \"ianvs\", \"organize-imports\" or an object, not {value}"
        ));
      }
    };
    for (name, _) in self.entries.iter().filter(|it| it.0 != "sortImports" && !it.1.is_null()) {
      let sorters = Sorter::of_option(name);
      if !sorter.is_some_and(|it| sorters.contains(&it)) {
        let names: Vec<&str> = sorters.iter().map(|it| it.name()).collect();
        return Err(format!("`{name}` needs `sortImports` set to {}", names.join(" or ")));
      }
    }
    let how = match sorter {
      None => return Ok(None),
      Some(Sorter::Bun) => match super::oxfmt::compile(value)? {
        Some(options) => How::Bun(options),
        None => return Ok(None),
      },
      Some(Sorter::Organize) => How::Organize(self.organize()?),
      Some(Sorter::Trivago) => How::Trivago(self.trivago(self.strings("importOrder")?)?),
      Some(Sorter::Ianvs) => match self.ianvs(self.strings("importOrder")?)? {
        Some(options) => How::Ianvs(options),
        None => return Ok(None),
      },
    };
    Ok(Some(Arc::new(SortImports { how, settings })))
  }

  fn organize(&self) -> Result<organize::Options, String> {
    Ok(organize::Options {
      skips_destructive_code_actions: self
        .boolean("organizeImportsSkipDestructiveCodeActions", false)?,
      type_order: match self.get("organizeImportsTypeOrder") {
        None => None,
        Some(value) => match value.as_str() {
          Some("last") => Some(TypeOrder::Last),
          Some("first") => Some(TypeOrder::First),
          Some("inline") => Some(TypeOrder::Inline),
          _ => return Err(invalid("organizeImportsTypeOrder", value)),
        },
      },
      jsx_needs_import: matches!(self.text("tsconfig.jsx"), Some("react" | "react-native")),
      jsx_namespace: (self.text("tsconfig.jsxFactory").map(first_name))
        .or_else(|| self.text("tsconfig.reactNamespace"))
        .unwrap_or("React")
        .as_bytes()
        .into(),
      jsx_fragment_factory: self
        .text("tsconfig.jsxFragmentFactory")
        .map(|it| first_name(it).as_bytes().into()),
    })
  }
}

/// `semver.valid(version)`: the three numbers, and true for a release, false for a prerelease.
fn semver(version: &[u8]) -> Option<((u64, u64, u64), bool)> {
  let version = version.trim_ascii();
  let version = version.strip_prefix(b"=").unwrap_or(version);
  let version = version.strip_prefix(b"v").unwrap_or(version);
  let version = strings::split_once_char(version, b'+').map_or(version, |it| it.0);
  let (numbers, prerelease) =
    strings::split_once_char(version, b'-').map_or((version, None), |it| (it.0, Some(it.1)));
  if prerelease.is_some_and(<[u8]>::is_empty) {
    return None;
  }
  let mut parts = strings::split(numbers, b".").map(|part| {
    let is_number = !part.is_empty()
      && part.iter().all(u8::is_ascii_digit)
      && (part.len() == 1 || part[0] != b'0');
    std::str::from_utf8(part).ok().filter(|_| is_number)?.parse::<u64>().ok()
  });
  let numbers = (parts.next()??, parts.next()??, parts.next()??);
  parts.next().is_none().then_some((numbers, prerelease.is_none()))
}

/// The `a` in `a.b.c`.
fn first_name(entity: &str) -> &str {
  entity.split('.').next().unwrap_or(entity)
}

/// `new RegExp(pattern)`
fn regex(pattern: &[u8]) -> Result<Regex, String> {
  Regex::new(pattern).map_err(|error| format!("Invalid regular expression in importOrder: {error}"))
}
