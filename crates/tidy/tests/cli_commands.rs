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

/// The names the help lists for `--lang`: those of code fences, and extensions.
const LANGUAGES: [&str; 23] = [
  "css",
  "scss",
  "less",
  "html",
  "vue",
  "mjml",
  "hbs",
  "tempura",
  "js",
  "jsx",
  "ts",
  "tsx",
  "json",
  "jsonc",
  "json5",
  "yaml",
  "toml",
  "md",
  "graphql",
  "xml",
  "dockerfile",
  "ini",
  "gitconfig",
];
#[test]
fn prints_help_without_a_command() {
  let folder = Folder::new("help", &[]);
  for args in [&[][..], &["-h"][..], &["--help"][..]] {
    let output = tidy(args, &folder.0);
    let help = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{args:?}");
    assert!(help.contains("Documentation: https://tidy.lukeed.com/docs/cli/"), "{help}");
    for section in ["Usage", "Available Commands", "Options", "Examples"] {
      assert!(help.contains(&format!("\n  {section}\n")), "{section} in {help}");
    }
    for only_up_and_check in
      ["Formatting Options", "Languages", "--lang <name>", "--width", "--staged"]
    {
      assert!(
        !help.contains(only_up_and_check),
        "{only_up_and_check} stays out of the global help"
      );
    }
    assert!(help.contains("tidy up --stdin --lang ts"), "{help}");
    assert!(help.lines().all(|line| line.len() <= 100), "no long lines: {help}");
  }
}

#[test]
fn up_and_check_list_everything_they_take() {
  let folder = Folder::new("format-help", &[]);
  for command in ["up", "check"] {
    let output = tidy(&[command, "--help"], &folder.0);
    let help = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{command}");
    assert!(help.contains("Documentation: https://tidy.lukeed.com/docs/cli/"), "{help}");
    for section in ["Usage", "Options", "Formatting Options", "Languages", "Examples"] {
      assert!(help.contains(&format!("\n  {section}\n")), "{command}: {section} in {help}");
    }
    for flag in [
      "--stdin",
      "--staged",
      "--lang <name>",
      "--config",
      "--skip-unknown",
      "--width",
      "--no-embedded",
    ] {
      assert!(help.contains(flag), "{command}: {flag} in {help}");
    }
    for language in LANGUAGES {
      assert!(help.contains(language), "{command}: {language} in {help}");
    }
    assert!(help.contains(&format!("tidy {command} --stdin --lang ts")), "{help}");
    assert!(help.lines().all(|line| line.len() <= 100), "no long lines: {help}");
  }
}

#[test]
fn every_language_in_the_help_works() {
  let folder = Folder::new("languages", &[]);
  for language in LANGUAGES {
    let output = tidy(&["check", "--stdin", "--lang", language], &folder.0);
    assert!(!stderr(&output).contains("unknown language"), "{language}: {}", stderr(&output));
  }
}

#[test]
fn takes_the_names_of_code_fences_and_extensions() {
  let folder = Folder::new("lang-names", &[]);
  let cases = [
    ("typescript", "const a   =1", "const a = 1;\n"),
    ("javascript", "const a   =1", "const a = 1;\n"),
    ("yml", "a:   1", "a: 1\n"),
    ("markdown", "*  a", "- a\n"),
    ("svg", "<svg  />", "<svg />\n"),
    ("mts", "export   const a=1", "export const a = 1;\n"),
  ];
  for (name, input, output) in cases {
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_tidy"))
      .args(["up", "--stdin", "--lang", name])
      .current_dir(&folder.0)
      .stdin(std::process::Stdio::piped())
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .expect("run tidy");
    use std::io::Write;
    child.stdin.take().expect("stdin").write_all(input.as_bytes()).expect("write");
    let result = child.wait_with_output().expect("wait");
    assert_eq!(
      String::from_utf8_lossy(&result.stdout),
      output,
      "{name}: {}",
      String::from_utf8_lossy(&result.stderr)
    );
  }
  // Handlebars prints `{{value}}`, and tempura `{{ value }}`; neither adds a final newline, as in Prettier.
  let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_tidy"))
    .args(["up", "--stdin", "--lang", "tempura"])
    .current_dir(&folder.0)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .spawn()
    .expect("run tidy");
  use std::io::Write;
  child.stdin.take().expect("stdin").write_all(b"<p>{{value}}</p>").expect("write");
  let result = child.wait_with_output().expect("wait");
  assert_eq!(String::from_utf8_lossy(&result.stdout), "<p>{{ value }}</p>");

  for name in ["cobol", "json.stringify", "babel"] {
    let output = tidy(&["check", "--stdin", "--lang", name], &folder.0);
    assert_eq!(
      stderr(&output),
      format!("tidy: unknown language `{name}`, see `tidy check --help`\n")
    );
  }
  let output = tidy(&["check", "--stdin", "--parser", "css"], &folder.0);
  assert_eq!(stderr(&output), "tidy: unknown flag `--parser`, see `tidy check --help`\n");
}

#[test]
fn prints_each_commands_help() {
  let folder = Folder::new("command-help", &[]);
  for (command, flag) in
    [("check", "--staged"), ("up", "--stdin"), ("init", "--toml"), ("schema", "Schema")]
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

/// Runs `tidy init` with `args`, and checks the file it writes: its name, its first lines, the options it holds,
/// that tidy reads it, and that it is formatted already.
fn init(args: &[&str], name: &str, head: &[String], key: impl Fn(&str) -> String) {
  let folder = Folder::new(&format!("init-{name}"), &[("a.css", "a{color:red}\n")]);
  let output = tidy(&[&["init"], args].concat(), &folder.0);
  assert_eq!(output.status.code(), Some(0), "{args:?}: {}", stderr(&output));
  assert_eq!(stderr(&output), format!("tidy: wrote {name}\n"));

  let config = folder.read(name);
  let lines: Vec<&str> = config.lines().collect();
  for (index, line) in head.iter().enumerate() {
    assert_eq!(lines[index], line, "{args:?}: line {} of {config}", index + 1);
  }
  for option in
    ["indent", "tabs", "eol", "quote", "trailingComma", "bracketSpacing", "ignore", "embedded"]
  {
    assert!(config.contains(&key(option)), "{option} in {config}");
  }
  for option in ["width", "whitespace"] {
    assert!(!config.contains(&key(option)), "{option} differs by language, so it stays out");
  }

  let output = tidy(&["check"], &folder.0);
  assert_eq!(
    (stdout(&output).as_str(), output.status.code()),
    ("a.css\n", Some(1)),
    "{}",
    stderr(&output)
  );
  let output = tidy(&["check", name], &folder.0);
  assert_eq!(
    output.status.code(),
    Some(0),
    "the config it writes is formatted: {}",
    stdout(&output)
  );
}

const DOCS: &str = "https://tidy.lukeed.com/docs/configuration/";

fn schema_url() -> String {
  format!("https://tidy.lukeed.com/{}/schema.json", env!("CARGO_PKG_VERSION"))
}

#[test]
fn init_writes_jsonc_with_a_link_to_the_docs() {
  let head = [format!("// {DOCS}"), "{".into(), format!("  \"$schema\": \"{}\",", schema_url())];
  init(&[], "tidy.jsonc", &head, |option| format!("\"{option}\":"));
}

#[test]
fn init_writes_json_when_asked() {
  let head = ["{".into(), format!("  \"$schema\": \"{}\",", schema_url())];
  init(&["--json"], "tidy.json", &head, |option| format!("\"{option}\":"));
}

#[test]
fn init_writes_toml_when_asked() {
  let head = [format!("#:schema {}", schema_url()), format!("# {DOCS}")];
  init(&["--toml"], "tidy.toml", &head, |option| format!("{option} = "));
}

#[test]
fn init_takes_one_format() {
  let folder = Folder::new("init-both", &[]);
  let output = tidy(&["init", "--json", "--toml"], &folder.0);
  assert_eq!(stderr(&output), "tidy: pick one of --json and --toml\n");
  assert_eq!(output.status.code(), Some(2));
  let output = tidy(&["init", "--yaml"], &folder.0);
  assert_eq!(stderr(&output), "tidy: unknown flag `--yaml`, see `tidy init --help`\n");
}

#[test]
fn init_leaves_a_config_alone() {
  for name in ["tidy.json", "tidy.jsonc", "tidy.toml"] {
    let folder = Folder::new("init-exists", &[(name, "{}\n")]);
    let output = tidy(&["init", "--toml"], &folder.0);
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
