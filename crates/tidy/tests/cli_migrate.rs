//! `tidy init` with the config of Prettier or oxfmt here: the tidy config it writes, and what it says didn't carry
//! over.

#![cfg(all(feature = "cli", feature = "js"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Folder(PathBuf);

impl Folder {
  fn new(name: &str, files: &[(&str, &str)]) -> Self {
    let root = std::env::temp_dir().join(format!("tidy-migrate-{name}-{}", std::process::id()));
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

fn stderr(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}

fn url() -> String {
  format!("https://tidy.lukeed.com/{}/schema.json", env!("CARGO_PKG_VERSION"))
}

/// Runs `tidy init` with `args` among `files`, and returns what it says and the config it writes, which must be
/// formatted already.
fn migrate(name: &str, files: &[(&str, &str)], args: &[&str], config: &str) -> (String, String) {
  let folder = Folder::new(name, files);
  let output = tidy(&[&["init"], args].concat(), &folder.0);
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  let check = tidy(&["check", config], &folder.0);
  assert_eq!(check.status.code(), Some(0), "{config} is formatted: {}", stderr(&check));
  (stderr(&output), folder.read(config))
}

#[test]
fn migrates_a_prettier_config() {
  let prettierrc = r#"{
  "printWidth": 100,
  "semi": false,
  "singleQuote": true,
  "trailingComma": "es5",
  "proseWrap": "always",
  "plugins": ["@ianvs/prettier-plugin-sort-imports", "prettier-plugin-tailwindcss"],
  "importOrder": ["<TYPES>", "<THIRD_PARTY_MODULES>", "", "^@/"],
  "overrides": [
    { "files": "*.md", "options": { "printWidth": 72, "semi": true } },
    { "files": ["src/**"], "options": { "tabWidth": 4 } }
  ]
}
"#;
  let (said, config) =
    migrate("prettier", &[(".prettierrc.json", prettierrc)], &["--json"], "tidy.json");
  assert_eq!(
    said,
    "tidy: wrote tidy.json from .prettierrc.json
tidy: .prettierrc.json: `semi` in the override for markdown: it is for js only
tidy: .prettierrc.json: the override for `src/**`: tidy's sections are per language, not per file
tidy: .prettierrc.json: the plugin `prettier-plugin-tailwindcss`: tidy runs no plugins
"
  );
  let expected = format!(
    r#"{{
  "$schema": "{}",
  "width": 100,
  "quote": "single",
  "yaml": {{
    "wrap": "always"
  }},
  "markdown": {{
    "width": 72,
    "wrap": "always"
  }},
  "js": {{
    "trailingComma": "es5",
    "semi": false,
    "jsdoc": false,
    "sortImports": "ianvs",
    "importOrder": ["<TYPES>", "<THIRD_PARTY_MODULES>", "", "^@/"]
  }}
}}
"#,
    url()
  );
  assert_eq!(config, expected);
}

#[test]
fn formats_with_what_it_migrated() {
  let prettierrc =
    r#"{ "semi": false, "singleQuote": true, "plugins": ["@ianvs/prettier-plugin-sort-imports"] }"#;
  let folder = Folder::new("formats", &[(".prettierrc.json", prettierrc)]);
  assert_eq!(tidy(&["init"], &folder.0).status.code(), Some(0));
  let mut child = Command::new(env!("CARGO_BIN_EXE_tidy"))
    .args(["up", "--stdin=a.ts"])
    .current_dir(&folder.0)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .spawn()
    .expect("run tidy");
  use std::io::Write;
  let input = "import b from \"b\"\nimport a from \"a\"\nlet c = \"x\"\n";
  child.stdin.take().expect("stdin").write_all(input.as_bytes()).expect("write");
  let output = child.wait_with_output().expect("wait");
  assert_eq!(
    String::from_utf8_lossy(&output.stdout),
    "import a from 'a'\nimport b from 'b'\n\nlet c = 'x'\n"
  );
}

#[test]
fn migrates_yaml_into_toml() {
  let prettierrc = "printWidth: 120\nuseTabs: true\nendOfLine: auto\nhtmlWhitespaceSensitivity: ignore\noverrides:\n  - files: \"**/*.{yml,yaml}\"\n    options:\n      singleQuote: true\n";
  let (said, config) = migrate("yaml", &[(".prettierrc", prettierrc)], &["--toml"], "tidy.toml");
  assert_eq!(
    said,
    "tidy: wrote tidy.toml from .prettierrc
tidy: .prettierrc: `endOfLine: \"auto\"`: tidy writes the line ending that `eol` names
"
  );
  let expected = format!(
    "#:schema {}
# https://tidy.lukeed.com/docs/configuration/

width = 120
tabs = true

[html]
whitespace = \"ignore\"

[vue]
whitespace = \"ignore\"

[handlebars]
whitespace = \"ignore\"

[yaml]
quote = \"single\"

[js]
jsdoc = false
",
    url()
  );
  assert_eq!(config, expected);
}

#[test]
fn reads_a_script_without_running_it() {
  let script = "/** @type {import(\"prettier\").Config} */\nconst config = {\n  semi: false,\n  arrowParens: `avoid`,\n  tabWidth: -(-4),\n  plugins: [\"prettier-plugin-jsdoc\"],\n};\nexport default config;\n";
  let (said, config) = migrate("script", &[("prettier.config.mjs", script)], &[], "tidy.jsonc");
  assert_eq!(
    said,
    "tidy: wrote tidy.jsonc from prettier.config.mjs
tidy: prettier.config.mjs: `tabWidth`: line 5: tidy can't read a computed number without running the file; set it by hand
"
  );
  assert!(
    config.contains("\"js\": {\n    \"semi\": false,\n    \"arrowParens\": \"avoid\",\n  },"),
    "{config}"
  );

  let script = "module.exports = { ...require(\"./base\"), semi: false };\n";
  let (said, config) = migrate("spread", &[(".prettierrc.cjs", script)], &[], "tidy.jsonc");
  assert!(said.ends_with(
    "tidy: .prettierrc.cjs: line 1: tidy can't read a spread without running the file; set it by hand\n"
  ));
  assert!(config.contains("\"semi\": false"), "{config}");
}

#[test]
fn migrates_an_oxfmt_config() {
  let oxfmtrc = r#"{
  "$schema": "./node_modules/oxfmt/configuration_schema.json",
  "sortImports": { "groups": ["builtin", "external"] },
  "ignorePatterns": ["dist"]
}
"#;
  let (said, config) = migrate("oxfmt", &[(".oxfmtrc.json", oxfmtrc)], &[], "tidy.jsonc");
  assert_eq!(
    said,
    "tidy: wrote tidy.jsonc from .oxfmtrc.json
tidy: .oxfmtrc.json: `ignorePatterns`: tidy skips what .gitignore lists; move them there
tidy: .oxfmtrc.json: `sortImports`: tidy sorts as `bun fmt` does: also the names inside braces, merging the imports of a module, and never across a side-effect import
tidy: .oxfmtrc.json: `sortPackageJson`: tidy keeps the keys of package.json in their order
"
  );
  let expected = format!(
    r#"// https://tidy.lukeed.com/docs/configuration/
{{
  "$schema": "{}",
  "width": 100,
  "js": {{
    "jsdoc": false,
    "sortImports": {{ "groups": ["builtin", "external"] }},
  }},
}}
"#,
    url()
  );
  assert_eq!(config, expected);

  let script = "import { defineConfig } from \"oxfmt\";\nexport default defineConfig({ printWidth: 80, jsdoc: true, sortPackageJson: false } satisfies object);\n";
  let (said, config) =
    migrate("oxfmt-ts", &[("oxfmt.config.ts", script)], &["--toml"], "tidy.toml");
  assert_eq!(said, "tidy: wrote tidy.toml from oxfmt.config.ts\n");
  assert!(config.ends_with("\n\nwidth = 80\n"), "{config}");
}

#[test]
fn reads_the_prettier_key_of_package_json() {
  let package =
    r#"{ "name": "a", "prettier": { "tabWidth": 4, "vueIndentScriptAndStyle": true } }"#;
  let (_, config) = migrate("package", &[("package.json", package)], &["--json"], "tidy.json");
  assert!(
    config.contains("    \"indent\": 4,\n    \"vue\": {\n        \"indentBlocks\": true\n    },"),
    "{config}"
  );

  let (said, _) =
    migrate("no-prettier", &[("package.json", r#"{ "name": "a" }"#)], &[], "tidy.jsonc");
  assert_eq!(said, "tidy: wrote tidy.jsonc\n", "a package.json without `prettier` is no config");

  let folder =
    Folder::new("shared", &[("package.json", r#"{ "prettier": "@acme/prettier-config" }"#)]);
  let output = tidy(&["init"], &folder.0);
  assert_eq!(
    stderr(&output),
    "tidy: package.json: it names the shared config `@acme/prettier-config`: pass that config's file with `--from`\n"
  );
  assert_eq!(output.status.code(), Some(2));
}

#[test]
fn asks_which_config_when_there_are_two() {
  let files =
    [(".prettierrc.json", r#"{ "semi": false }"#), (".oxfmtrc.json", r#"{ "printWidth": 90 }"#)];
  let folder = Folder::new("two", &files);
  let output = tidy(&["init"], &folder.0);
  assert_eq!(
    stderr(&output),
    "tidy: found .prettierrc.json, .oxfmtrc.json: pick one with `tidy init --from <file>`\n"
  );
  assert_eq!(output.status.code(), Some(2));
  let output = tidy(&["init", "--from", ".prettierrc.json"], &folder.0);
  assert_eq!(stderr(&output), "tidy: wrote tidy.jsonc from .prettierrc.json\n");
  assert!(folder.read("tidy.jsonc").contains("\"semi\": false"));
  let output = tidy(&["init", "--from=missing.json"], &Folder::new("missing", &[]).0);
  assert!(stderr(&output).starts_with("tidy: missing.json: "), "{}", stderr(&output));
}

#[test]
fn says_what_an_override_loses() {
  let json5 = "// comment\n{ semi: false, overrides: [{ files: \"*.scss\", excludeFiles: \"vendor/*\", options: { tabWidth: 4 } }] }\n";
  let (said, config) = migrate("json5", &[(".prettierrc.json5", json5)], &[], "tidy.jsonc");
  assert!(
    said.ends_with(
      "tidy: .prettierrc.json5: the override for `*.scss`: tidy has no `excludeFiles`\n"
    ),
    "{said}"
  );
  assert!(!config.contains("\"css\""), "{config}");
}
