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

  /// Runs git without the user's config, which can sign commits with a key that fails under load.
  fn git(&self, args: &[&str]) -> Output {
    let output = Command::new("git")
      .args(args)
      .current_dir(&self.0)
      .env("GIT_CONFIG_GLOBAL", std::env::temp_dir().join("tidy-git-no-config"))
      .env("GIT_CONFIG_NOSYSTEM", "1")
      .output()
      .expect("run git");
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    output
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

/// `--staged` is about the commit, not the folder, so it covers the whole
/// index wherever tidy runs. A file below that folder is named as it was
/// typed; one above it keeps a path that still resolves.
#[test]
fn names_each_file_from_the_folder_it_runs_in() {
  let repo = Repo::new("prefix", &[("src/a.ts", "const a = 1;\n")]);
  repo.write("src/b.ts", "const b   =2\n");
  repo.write("top.ts", "const c   =3\n");
  repo.git(&["add", "-A"]);

  let root = tidy(&["check", "--staged"], &repo.0);
  assert_eq!(lines(&root), ["src/b.ts", "top.ts"]);

  let inside = lines(&tidy(&["check", "--staged"], &repo.0.join("src")));
  assert_eq!(inside.len(), 2, "the whole index, not just this folder: {inside:?}");
  assert!(inside.contains(&"b.ts".to_owned()), "named from here: {inside:?}");
  assert!(inside.iter().any(|path| path.ends_with("top.ts")), "still named: {inside:?}");
}

/// A path given alongside the scope narrows it, which is how you format one folder.
#[test]
fn a_path_narrows_a_scope_from_a_subfolder() {
  let repo = Repo::new("narrow-sub", &[("src/a.ts", "const a = 1;\n")]);
  repo.write("src/b.ts", "const b   =2\n");
  repo.write("top.ts", "const c   =3\n");
  repo.git(&["add", "-A"]);

  let output = tidy(&["check", "--staged", "."], &repo.0.join("src"));
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
