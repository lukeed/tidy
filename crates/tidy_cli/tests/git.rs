//! `--staged` and `--changed`, which ask git which files to format.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A git repository with one commit, removed when dropped.
struct Repo(PathBuf);

impl Repo {
  fn new(name: &str, files: &[(&str, &str)]) -> Self {
    let root = std::env::temp_dir().join(format!("tidy-git-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create folder");
    let repo = Self(root);
    repo.git(&["init", "-q", "-b", "main", "."]);
    for (path, text) in files {
      repo.write(path, text);
    }
    repo.git(&["add", "-A"]);
    repo.commit("start");
    repo
  }

  fn write(&self, path: &str, text: &str) {
    let path = self.0.join(path);
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create folder");
    std::fs::write(path, text).expect("write file");
  }

  fn read(&self, path: &str) -> String {
    std::fs::read_to_string(self.0.join(path)).expect("read file")
  }

  fn git(&self, args: &[&str]) -> Output {
    Command::new("git").args(args).current_dir(&self.0).output().expect("run git")
  }

  fn commit(&self, message: &str) {
    self.git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", message]);
  }

  /// The paths git reports for one of its lists, as a sorted vector.
  fn listed(&self, args: &[&str]) -> Vec<String> {
    let output = self.git(args);
    let text = String::from_utf8_lossy(&output.stdout);
    let mut paths: Vec<String> = text.lines().map(str::to_owned).collect();
    paths.sort();
    paths
  }
}

impl Drop for Repo {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

fn tidy(args: &[&str], cwd: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_tidy")).args(args).current_dir(cwd).output().expect("run tidy")
}

/// The paths tidy printed, sorted, so the order of a walk doesn't matter.
fn lines(output: &Output) -> Vec<String> {
  let text = String::from_utf8_lossy(&output.stdout);
  let mut paths: Vec<String> = text.lines().map(str::to_owned).collect();
  paths.sort();
  paths
}

#[test]
fn checks_only_staged_files() {
  let repo = Repo::new("staged", &[("a.ts", "const a = 1;\n")]);
  repo.write("staged.ts", "const b   =2\n");
  repo.write("loose.ts", "const c   =3\n");
  repo.git(&["add", "staged.ts"]);

  let output = tidy(&["check", "--staged"], &repo.0);
  assert_eq!(lines(&output), ["staged.ts"]);
  assert_eq!(output.status.code(), Some(1));
}

#[test]
fn stages_what_it_rewrote() {
  let repo = Repo::new("restage", &[("a.ts", "const a = 1;\n")]);
  repo.write("staged.ts", "const b   =2\n");
  repo.write("loose.ts", "const c   =3\n");
  repo.git(&["add", "staged.ts"]);

  let output = tidy(&["up", "--staged"], &repo.0);
  assert!(output.status.success());
  assert_eq!(repo.read("staged.ts"), "const b = 2;\n");
  // The file it left alone is untouched, and the rewrite is staged, not left behind.
  assert_eq!(repo.read("loose.ts"), "const c   =3\n");
  assert_eq!(repo.listed(&["diff", "--cached", "--name-only"]), ["staged.ts"]);
  assert!(repo.listed(&["diff", "--name-only"]).is_empty());
}

#[test]
fn checks_what_differs_from_a_branch() {
  let repo = Repo::new("changed", &[("a.ts", "const a = 1;\n")]);
  repo.git(&["checkout", "-qb", "feature"]);
  repo.write("committed.ts", "const b   =2\n");
  repo.git(&["add", "-A"]);
  repo.commit("work");
  repo.write("untracked.ts", "const c   =3\n");

  // A new file that is not yet added still differs from the branch.
  let output = tidy(&["check", "--changed=main"], &repo.0);
  assert_eq!(lines(&output), ["committed.ts", "untracked.ts"]);
  assert_eq!(output.status.code(), Some(1));
}

#[test]
fn a_path_narrows_what_git_named() {
  let repo = Repo::new("narrow", &[("a.ts", "const a = 1;\n")]);
  repo.git(&["checkout", "-qb", "feature"]);
  repo.write("src/b.ts", "const b   =2\n");
  repo.write("other/c.ts", "const c   =3\n");

  let output = tidy(&["check", "--changed=main", "src"], &repo.0);
  assert_eq!(lines(&output), ["src/b.ts"]);
}

#[test]
fn nothing_changed_is_not_a_failure() {
  let repo = Repo::new("empty", &[("a.ts", "const a = 1;\n")]);
  let output = tidy(&["check", "--changed=main"], &repo.0);
  assert!(output.status.success());
  assert!(lines(&output).is_empty());
}

#[test]
fn leaves_out_files_it_cannot_format() {
  let repo = Repo::new("other", &[("a.ts", "const a = 1;\n")]);
  repo.write("notes.rs", "fn  main( ){}\n");
  repo.write("b.ts", "const b   =2\n");
  repo.git(&["add", "-A"]);

  let output = tidy(&["check", "--staged"], &repo.0);
  assert_eq!(lines(&output), ["b.ts"]);
}

#[test]
fn the_two_scopes_are_exclusive() {
  let repo = Repo::new("both", &[("a.ts", "const a = 1;\n")]);
  let output = tidy(&["up", "--staged", "--changed=main"], &repo.0);
  assert_eq!(output.status.code(), Some(2));
  assert!(String::from_utf8_lossy(&output.stderr).contains("can't be used together"));
}

#[test]
fn a_scope_needs_up_or_check() {
  let repo = Repo::new("scope", &[("a.ts", "const a = 1;\n")]);
  let output = tidy(&["--staged"], &repo.0);
  assert_eq!(output.status.code(), Some(2));
  assert!(String::from_utf8_lossy(&output.stderr).contains("`tidy up` or `tidy check`"));
}
