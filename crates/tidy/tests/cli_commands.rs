//! The commands, their help, and `tidy init`.

#![cfg(feature = "cli")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Folder(PathBuf);

impl Folder {
  fn new(name: &str, files: &[(&str, &str)]) -> Self {
    let root = std::env::temp_dir().join(format!("tidy-commands-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create folder");
    for (path, text) in files {
      std::fs::write(root.join(path), text).expect("write file");
    }
    Self(root)
  }

  fn read(&self, path: &str) -> String {
    std::fs::read_to_string(self.0.join(path)).expect("read file")
  }
}

impl Drop for Folder {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

fn tidy(args: &[&str], cwd: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_tidy")).args(args).current_dir(cwd).output().expect("run tidy")
}

fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}

const PARSERS: [&str; 23] = [
  "babel",
  "babel-ts",
  "css",
  "dockerfile",
  "gitconfig",
  "graphql",
  "handlebars",
  "html",
  "ini",
  "json",
  "json-stringify",
  "json5",
  "jsonc",
  "less",
  "markdown",
  "mjml",
  "scss",
  "tempura",
  "toml",
  "typescript",
  "vue",
  "xml",
  "yaml",
];

#[test]
fn prints_help_without_a_command() {
  let folder = Folder::new("help", &[]);
  for args in [&[][..], &["-h"][..], &["--help"][..]] {
    let output = tidy(args, &folder.0);
    let help = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{args:?}");
    assert!(help.contains("Documentation: https://tidy.lukeed.com/docs/cli/"), "{help}");
    for section in
      ["Usage", "Available Commands", "Options", "Formatting Options", "Parsers", "Examples"]
    {
      assert!(help.contains(&format!("\n  {section}\n")), "{section} in {help}");
    }
    for parser in PARSERS {
      assert!(help.contains(parser), "{parser} in {help}");
    }
    assert!(help.contains("tidy up --stdin --parser typescript"), "{help}");
    assert!(!help.contains("--staged"), "command options stay out of the global help");
    assert!(help.lines().all(|line| line.len() <= 100), "no long lines: {help}");
  }
}

#[test]
fn every_parser_in_the_help_works() {
  let folder = Folder::new("parsers", &[]);
  for parser in PARSERS {
    let output = tidy(&["check", "--stdin", "--parser", parser], &folder.0);
    assert!(!stderr(&output).contains("unknown parser"), "{parser}: {}", stderr(&output));
  }
}

#[test]
fn prints_each_commands_help() {
  let folder = Folder::new("command-help", &[]);
  for (command, flag) in
    [("check", "--staged"), ("up", "--stdin"), ("init", "tidy.json"), ("schema", "Schema")]
  {
    let output = tidy(&[command, "--help"], &folder.0);
    assert_eq!(output.status.code(), Some(0), "{command}");
    assert!(
      stdout(&output).contains(&format!("$ tidy {command}")),
      "{command}: {}",
      stdout(&output)
    );
    assert!(stdout(&output).contains(flag), "{command}: {}", stdout(&output));
  }
}

#[test]
fn rejects_what_is_not_a_command() {
  let folder = Folder::new("unknown", &[("a.css", "a{}")]);
  let output = tidy(&["a.css"], &folder.0);
  assert_eq!(stderr(&output), "tidy: unknown command `a.css`, see `tidy --help`\n");
  assert_eq!(output.status.code(), Some(2));
}

#[test]
fn init_writes_a_config_that_tidy_reads() {
  let folder = Folder::new("init", &[("a.css", "a{color:red}\n")]);
  let output = tidy(&["init"], &folder.0);
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert_eq!(stderr(&output), "tidy: wrote tidy.json\n");

  let config = folder.read("tidy.json");
  let schema =
    format!("\"$schema\": \"https://tidy.lukeed.com/{}/schema.json\"", env!("CARGO_PKG_VERSION"));
  assert!(config.contains(&schema), "{config}");
  for key in
    ["indent", "tabs", "eol", "quote", "trailingComma", "bracketSpacing", "ignore", "embedded"]
  {
    assert!(config.contains(&format!("\"{key}\":")), "{key} in {config}");
  }
  for key in ["width", "whitespace"] {
    assert!(!config.contains(&format!("\"{key}\":")), "{key} differs by language, so it stays out");
  }

  let output = tidy(&["check"], &folder.0);
  assert_eq!(
    (stdout(&output).as_str(), output.status.code()),
    ("a.css\n", Some(1)),
    "{}",
    stderr(&output)
  );
  let output = tidy(&["check", "tidy.json"], &folder.0);
  assert_eq!(
    output.status.code(),
    Some(0),
    "the config it writes is formatted: {}",
    stdout(&output)
  );
}

#[test]
fn init_leaves_a_config_alone() {
  for name in ["tidy.json", "tidy.jsonc", "tidy.toml"] {
    let folder = Folder::new("init-exists", &[(name, "{}\n")]);
    let output = tidy(&["init"], &folder.0);
    assert_eq!(stderr(&output), format!("tidy: {name} is here already\n"));
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(folder.read(name), "{}\n");
  }
}

#[test]
fn takes_a_schema_key_in_a_config() {
  let folder = Folder::new(
    "schema-key",
    &[
      ("tidy.json", "{ \"$schema\": \"https://tidy.lukeed.com/schema.json\", \"indent\": 4 }\n"),
      ("a.css", "a{color:red}"),
    ],
  );
  let output = tidy(&["up", "a.css"], &folder.0);
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert_eq!(folder.read("a.css"), "a {\n    color: red;\n}\n");
}

#[test]
fn prints_a_versioned_schema() {
  let folder = Folder::new("schema", &[]);
  let output = tidy(&["schema"], &folder.0);
  let id =
    format!("\"$id\": \"https://tidy.lukeed.com/{}/schema.json\"", env!("CARGO_PKG_VERSION"));
  assert!(stdout(&output).contains(&id), "{}", stdout(&output));
}
