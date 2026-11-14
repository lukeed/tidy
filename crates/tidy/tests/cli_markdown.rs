//! The CLI as a user runs it, against folders made for each test.

#![cfg(feature = "cli")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A folder of Markdown files, removed when dropped.
struct Folder(PathBuf);

impl Folder {
  fn new(name: &str, files: &[(&str, &str)]) -> Self {
    let root = std::env::temp_dir().join(format!("tidy-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (path, text) in files {
      let path = root.join(path);
      std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create folder");
      std::fs::write(path, text).expect("write file");
    }
    Self(root)
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

#[test]
fn lists_files_to_format() {
  let folder = Folder::new("check", &[("docs/a.md", "* a\n"), ("docs/b.md", "- b\n")]);
  let output = tidy(&["check", "docs"], &folder.0);
  assert_eq!(String::from_utf8_lossy(&output.stdout), "docs/a.md\n");
  assert_eq!(output.status.code(), Some(1));
}

#[test]
fn lists_each_file_once() {
  let folder = Folder::new("once", &[("docs/a.md", "* a\n"), (".github/b.md", "* b\n")]);
  let output = tidy(&["check", "docs", "./docs", "docs/a.md", ".github/b.md"], &folder.0);
  assert_eq!(String::from_utf8_lossy(&output.stdout), ".github/b.md\ndocs/a.md\n");
  assert_eq!(output.status.code(), Some(1));
}

#[test]
fn writes_files_in_place() {
  let folder = Folder::new("write", &[("docs/a.md", "* a\n"), ("docs/b.md", "- b\n")]);
  let output = tidy(&["up", "docs"], &folder.0);
  assert_eq!(String::from_utf8_lossy(&output.stdout), "docs/a.md\n");
  assert_eq!(std::fs::read_to_string(folder.0.join("docs/a.md")).expect("read"), "- a\n");
  assert_eq!(output.status.code(), Some(0));
}

#[cfg(unix)]
#[test]
fn reports_folders_it_cannot_read() {
  use std::os::unix::fs::PermissionsExt;
  let folder = Folder::new("locked", &[("docs/a.md", "- a\n"), ("docs/locked/b.md", "* b\n")]);
  let locked = folder.0.join("docs/locked");
  std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("lock folder");
  let output = tidy(&["check", "docs"], &folder.0);
  std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("unlock folder");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("docs/locked"), "{stderr}");
  assert_eq!(output.status.code(), Some(2));
}

#[test]
fn names_files_from_the_folder_when_given_no_paths() {
  let folder = Folder::new("implicit", &[("a.md", "* a\n"), ("docs/b.md", "* b\n")]);
  let output = tidy(&["check"], &folder.0);
  assert_eq!(String::from_utf8_lossy(&output.stdout), "a.md\ndocs/b.md\n");

  let output = tidy(&["check", "."], &folder.0);
  assert_eq!(
    String::from_utf8_lossy(&output.stdout),
    "./a.md\n./docs/b.md\n",
    "a path given keeps its spelling"
  );
}
