//! Formats the files under a folder the way dprint does with its plugins, leaving embedded code as it is written:
//! markup_fmt (dprint-plugin-markup) for markup and XML, malva (dprint-plugin-malva) for style sheets,
//! dprint-plugin-json for JSON, dprint-plugin-toml for TOML, pretty_yaml (dprint-plugin-yaml) for YAML,
//! dprint-plugin-dockerfile for Dockerfiles, pretty_graphql (dprint-plugin-graphql) for GraphQL, and
//! dprint-plugin-markdown for Markdown. Exits 1 if any file changes,
//! like `--check`.

use std::path::{Path, PathBuf};

use markup_fmt::config::FormatOptions;

fn main() {
  let mut files = Vec::new();
  for arg in std::env::args().skip(1) {
    walk(Path::new(&arg), &mut files);
  }
  let options = FormatOptions::default();
  let markdown = dprint_plugin_markdown::configuration::ConfigurationBuilder::new().build();
  let json = dprint_plugin_json::configuration::ConfigurationBuilder::new().build();
  let toml = dprint_plugin_toml::configuration::ConfigurationBuilder::new().build();
  let dockerfile = dprint_plugin_dockerfile::configuration::ConfigurationBuilder::new().build();
  let mut changed = 0;
  for path in files {
    let Ok(text) = std::fs::read_to_string(&path) else { continue };
    if tidy_dockerfile_name(&path) {
      let output = dprint_plugin_dockerfile::format_text(&path, &text, &dockerfile);
      changed += usize::from(matches!(output, Ok(Some(output)) if output != text));
      continue;
    }
    let output = match path.extension().and_then(|it| it.to_str()) {
      Some("md" | "markdown") => {
        dprint_plugin_markdown::format_text(&text, &markdown, |_, _, _| Ok(None))
          .ok()
          .map(|output| output.unwrap_or_else(|| text.clone()))
      }
      Some("json" | "jsonc" | "json5") => dprint_plugin_json::format_text(&path, &text, &json)
        .ok()
        .map(|output| output.unwrap_or_else(|| text.clone())),
      Some("graphql" | "gql" | "graphqls") => {
        pretty_graphql::format_text(&text, &pretty_graphql::config::FormatOptions::default()).ok()
      }
      Some("yml" | "yaml") => {
        pretty_yaml::format_text(&text, &pretty_yaml::config::FormatOptions::default()).ok()
      }
      Some("toml") => dprint_plugin_toml::format_text(&path, &text, &toml)
        .ok()
        .map(|output| output.unwrap_or_else(|| text.clone())),
      Some(extension @ ("css" | "scss" | "less")) => {
        let syntax = match extension {
          "scss" => malva::Syntax::Scss,
          "less" => malva::Syntax::Less,
          _ => malva::Syntax::Css,
        };
        malva::format_text(&text, syntax, &malva::config::FormatOptions::default()).ok()
      }
      _ => markup_fmt::detect_language(&path).and_then(|language| {
        markup_fmt::format_text(&text, language, &options, |code, _| Ok::<_, ()>(code.into())).ok()
      }),
    };
    if output.is_some_and(|output| output != text) {
      changed += 1;
    }
  }
  std::process::exit(i32::from(changed > 0));
}

/// The names that tidy formats as Dockerfiles.
fn tidy_dockerfile_name(path: &Path) -> bool {
  let name = path.file_name().and_then(|it| it.to_str()).unwrap_or_default();
  let lower = name.to_ascii_lowercase();
  name.starts_with("Dockerfile")
    || name.starts_with("Containerfile")
    || lower.ends_with(".dockerfile")
    || lower.ends_with(".containerfile")
}

fn walk(path: &Path, files: &mut Vec<PathBuf>) {
  if path.is_file() {
    files.push(path.to_path_buf());
    return;
  }
  let Ok(entries) = std::fs::read_dir(path) else { return };
  for entry in entries.flatten() {
    walk(&entry.path(), files);
  }
}
