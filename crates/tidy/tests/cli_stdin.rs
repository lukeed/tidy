//! `--stdin`, `--parser` and `--skip-unknown`, as an editor runs them.

#![cfg(feature = "cli")]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// A folder of files, removed when dropped.
struct Folder(PathBuf);

impl Folder {
  fn new(name: &str, files: &[(&str, &str)]) -> Self {
    let root = std::env::temp_dir().join(format!("tidy-stdin-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create folder");
    for (path, text) in files {
      let path = root.join(path);
      std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create folder");
      std::fs::write(path, text).expect("write file");
    }
    Self(root)
  }

  fn path(&self, path: &str) -> PathBuf {
    self.0.join(path)
  }
}

impl Drop for Folder {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

fn tidy(args: &[&str], input: &str, cwd: &Path) -> Output {
  let mut child = Command::new(env!("CARGO_BIN_EXE_tidy"))
    .args(args)
    .current_dir(cwd)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("run tidy");
  let mut stdin = child.stdin.take().expect("stdin is piped");
  // tidy exits before it reads stdin when it rejects its arguments.
  match stdin.write_all(input.as_bytes()) {
    Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
    other => other.expect("write stdin"),
  }
  drop(stdin);
  child.wait_with_output().expect("wait for tidy")
}

fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}

const LONG: &str = "<p class=\"aaaa bbbb\">hi there friend</p>";
const NARROW: &str = "<p\n  class=\"aaaa bbbb\"\n>\n  hi there friend\n</p>\n";
const WIDE: &str = "<p class=\"aaaa bbbb\">hi there friend</p>\n";

/// Two sibling folders with different configs, and none above them.
fn siblings(name: &str) -> Folder {
  Folder::new(name, &[("narrow/tidy.json", "{ \"width\": 20 }\n"), ("wide/tidy.json", "{}\n")])
}

#[test]
fn takes_the_config_next_to_the_path() {
  let folder = siblings("config");
  let output = tidy(&["--stdin=narrow/x.html"], LONG, &folder.0);
  assert_eq!(stdout(&output), NARROW, "{}", stderr(&output));
  assert_eq!(output.status.code(), Some(0));
}

#[test]
fn takes_the_config_of_the_path_not_of_the_folder_it_runs_in() {
  let folder = siblings("cwd");
  let output = tidy(&["--stdin=../narrow/x.html"], LONG, &folder.path("wide"));
  assert_eq!(stdout(&output), NARROW, "{}", stderr(&output));

  let absolute = folder.path("wide/x.html");
  let output = tidy(&[&format!("--stdin={}", absolute.display())], LONG, &folder.path("narrow"));
  assert_eq!(stdout(&output), WIDE, "{}", stderr(&output));
}

#[test]
fn takes_the_language_from_the_path() {
  let folder = Folder::new("language", &[]);
  let output = tidy(&["--stdin=a.css"], "a{color:red}", &folder.0);
  assert_eq!(stdout(&output), "a {\n  color: red;\n}\n");
}

#[test]
fn parser_wins_over_the_path() {
  let folder = Folder::new("parser-wins", &[]);
  let output = tidy(&["--stdin=a.html", "--parser", "css"], "a{color:red}", &folder.0);
  assert_eq!(stdout(&output), "a {\n  color: red;\n}\n");
}

#[test]
fn bare_stdin_takes_a_parser() {
  let folder = Folder::new("bare", &[("tidy.json", "{ \"js\": { \"semi\": false } }\n")]);
  let output = tidy(&["--stdin", "--parser", "typescript"], "const a   =1", &folder.0);
  assert_eq!(stdout(&output), "const a = 1;\n", "no path, so no config: {}", stderr(&output));
}

#[test]
fn bare_stdin_takes_a_named_config() {
  let folder = Folder::new("named", &[("cfg.json", "{ \"js\": { \"semi\": false } }\n")]);
  let args = ["--stdin", "--parser", "typescript", "--config", "cfg.json"];
  let output = tidy(&args, "const a   =1", &folder.0);
  assert_eq!(stdout(&output), "const a = 1\n", "{}", stderr(&output));
}

#[test]
fn reads_stdin_without_the_flag() {
  let folder = Folder::new("implicit", &[]);
  let output = tidy(&["--parser", "json"], "{\"a\":1}", &folder.0);
  assert_eq!(stdout(&output), "{ \"a\": 1 }\n");
}

#[test]
fn reads_stdin_for_a_dash() {
  let folder = Folder::new("dash", &[]);
  let output = tidy(&["-", "--parser", "json"], "{\"a\":1}", &folder.0);
  assert_eq!(stdout(&output), "{ \"a\": 1 }\n");

  let output = tidy(&["check", "-", "--parser", "json"], "{\"a\":1}", &folder.0);
  assert_eq!((stdout(&output).as_str(), output.status.code()), ("", Some(1)));

  let output = tidy(&["up", "-", "--parser", "json"], "{\"a\":1}", &folder.0);
  assert_eq!(stderr(&output), "tidy: `tidy up` can't rewrite stdin, use `tidy --stdin`\n");
}

#[test]
fn check_exits_1_when_not_formatted() {
  let folder = Folder::new("check", &[]);
  let output = tidy(&["check", "--stdin=a.ts"], "const a   =1", &folder.0);
  assert_eq!((stdout(&output).as_str(), output.status.code()), ("", Some(1)));

  let output = tidy(&["check", "--stdin", "--parser", "typescript"], "const a = 1;\n", &folder.0);
  assert_eq!((stdout(&output).as_str(), output.status.code()), ("", Some(0)));
}

#[test]
fn check_with_stdin_ignores_the_folder() {
  let folder = Folder::new("check-folder", &[("a.ts", "const a   =1\n")]);
  let output = tidy(&["check", "--stdin=b.ts"], "const b = 1;\n", &folder.0);
  assert_eq!((stdout(&output).as_str(), output.status.code()), ("", Some(0)));
}

#[test]
fn fails_on_an_unknown_language() {
  let folder = Folder::new("unknown", &[]);
  let output = tidy(&["--stdin=a.unknown"], "a{color:red}", &folder.0);
  assert_eq!(stdout(&output), "");
  assert_eq!(
    stderr(&output),
    "tidy: a.unknown: can't tell the language of this file, pass --parser\n"
  );
  assert_eq!(output.status.code(), Some(2));
}

#[test]
fn fails_without_a_language() {
  let folder = Folder::new("no-language", &[]);
  for args in [&[][..], &["--stdin"][..], &["check", "--stdin"][..]] {
    let output = tidy(args, "<p>hi</p>", &folder.0);
    assert_eq!(
      stderr(&output),
      "tidy: can't tell the language of stdin, pass --parser or --stdin=<path>\n",
      "{args:?}"
    );
    assert_eq!(output.status.code(), Some(2), "{args:?}");
  }
}

#[test]
fn skip_unknown_prints_the_text_unchanged() {
  let folder = Folder::new("skip", &[]);
  let output = tidy(&["--stdin=a.unknown", "--skip-unknown"], "a{color:red}", &folder.0);
  assert_eq!((stdout(&output).as_str(), output.status.code()), ("a{color:red}", Some(0)));

  let output = tidy(&["--stdin", "--skip-unknown"], "a{color:red}", &folder.0);
  assert_eq!((stdout(&output).as_str(), output.status.code()), ("a{color:red}", Some(0)));

  let output = tidy(&["check", "--stdin=a.unknown", "--skip-unknown"], "a{color:red}", &folder.0);
  assert_eq!((stdout(&output).as_str(), output.status.code()), ("", Some(0)));
}

#[test]
fn skip_unknown_skips_named_files() {
  let folder = Folder::new("skip-files", &[("a.unknown", "a"), ("b.ts", "const b   =1\n")]);
  let output = tidy(&["check", "a.unknown", "b.ts"], "", &folder.0);
  assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));

  let output = tidy(&["check", "a.unknown", "b.ts", "--skip-unknown"], "", &folder.0);
  assert_eq!((stdout(&output).as_str(), stderr(&output).as_str()), ("b.ts\n", ""));
  assert_eq!(output.status.code(), Some(1));

  let output = tidy(&["a.unknown", "--skip-unknown"], "", &folder.0);
  assert_eq!((stdout(&output).as_str(), output.status.code()), ("a", Some(0)));
}

#[test]
fn rejects_stdin_where_it_cannot_work() {
  let folder = Folder::new("rejects", &[("a.ts", "const a = 1;\n")]);
  let cases: [(&[&str], &str); 5] = [
    (&["up", "--stdin=a.ts"], "tidy: `tidy up` can't rewrite stdin, use `tidy --stdin`\n"),
    (&["--stdin", "a.ts"], "tidy: --stdin takes no paths, name the text with --stdin=<path>\n"),
    (&["check", "--stdin", "--staged"], "tidy: --stdin can't be used with --staged or --changed\n"),
    (
      &["check", "--stdin=a.ts", "--changed"],
      "tidy: --stdin can't be used with --staged or --changed\n",
    ),
    (&["--stdin="], "tidy: --stdin= needs a path\n"),
  ];
  for (args, message) in cases {
    let output = tidy(args, "const a = 1;\n", &folder.0);
    assert_eq!(stderr(&output), message, "{args:?}");
    assert_eq!(output.status.code(), Some(2), "{args:?}");
  }
}

#[test]
fn reports_syntax_errors_at_the_path() {
  let folder = Folder::new("syntax", &[]);
  let output = tidy(&["--stdin=src/a.css"], "a {", &folder.0);
  assert_eq!(stderr(&output), "tidy: src/a.css:1:1: Unclosed block\n");
  assert_eq!(output.status.code(), Some(2));

  let output = tidy(&["--stdin", "--parser", "css"], "a {", &folder.0);
  assert_eq!(stderr(&output), "tidy: stdin:1:1: Unclosed block\n");
}
