//! The CLI as a user runs it, against folders made for each test.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A folder of Markdown files, removed when dropped.
struct Folder(PathBuf);

impl Folder {
  fn new(name: &str, files: &[(&str, &str)]) -> Self {
    let root = std::env::temp_dir().join(format!("mdtidy-cli-{name}-{}", std::process::id()));
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

fn mdtidy(args: &[&str], cwd: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_mdtidy"))
    .args(args)
    .current_dir(cwd)
    .output()
    .expect("run mdtidy")
}

#[test]
fn lists_files_to_format() {
  let folder = Folder::new("check", &[("docs/a.md", "* a\n"), ("docs/b.md", "- b\n")]);
  let output = mdtidy(&["docs", "--check"], &folder.0);
  assert_eq!(String::from_utf8_lossy(&output.stdout), "docs/a.md\n");
  assert_eq!(output.status.code(), Some(1));
}

#[cfg(unix)]
#[test]
fn reports_folders_it_cannot_read() {
  use std::os::unix::fs::PermissionsExt;
  let folder = Folder::new("locked", &[("docs/a.md", "- a\n"), ("docs/locked/b.md", "* b\n")]);
  let locked = folder.0.join("docs/locked");
  std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("lock folder");
  let output = mdtidy(&["docs", "--check"], &folder.0);
  std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("unlock folder");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("docs/locked"), "{stderr}");
  assert_eq!(output.status.code(), Some(2));
}
