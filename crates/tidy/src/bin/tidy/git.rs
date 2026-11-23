//! The files git names for `--staged` and `--changed`.

use std::path::{Path, PathBuf};

use tidy::Language;

/// Runs git in the current folder and returns its stdout.
pub(crate) fn git(args: &[&str]) -> Result<String, String> {
  let output = std::process::Command::new("git")
    .args(args)
    .output()
    .map_err(|error| format!("can't run git: {error}"))?;
  if !output.status.success() {
    let message = String::from_utf8_lossy(&output.stderr);
    return Err(format!("git {}: {}", args[0], message.trim()));
  }
  String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

/// The files that git names, in a language tidy formats and still on disk.
///
/// git writes every path from the root of the repository, with `/` on every
/// system. Taking the path apart as text, rather than against the filesystem,
/// keeps that the same on Windows, where the two disagree about separators.
pub(crate) fn git_listed(lists: &[&[&str]]) -> Result<Vec<PathBuf>, String> {
  let root = git(&["rev-parse", "--show-toplevel"])?.trim().to_owned();
  // Where tidy runs, below the root: `src/`, or empty at the root itself.
  let prefix = git(&["rev-parse", "--show-prefix"])?.trim().to_owned();

  let mut paths = Vec::new();
  for args in lists {
    for line in git(args)?.lines() {
      if line.is_empty() || Language::file(line).is_none() {
        continue;
      }
      // Name it as it was typed when it is below this folder, else from the root.
      let path = match line.strip_prefix(&prefix) {
        Some(below) => PathBuf::from(below),
        None => PathBuf::from(&root).join(line),
      };
      if path.is_file() && !paths.contains(&path) {
        paths.push(path);
      }
    }
  }
  Ok(paths)
}

/// The branch that `--changed` compares against when none is named.
pub(crate) fn default_branch() -> Result<String, String> {
  if let Ok(head) = git(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) {
    let head = head.trim();
    if !head.is_empty() {
      return Ok(head.to_owned());
    }
  }
  for name in ["main", "master"] {
    if git(&["rev-parse", "--verify", "--quiet", name]).is_ok() {
      return Ok(name.to_owned());
    }
  }
  Err("can't tell which branch is the default one, pass --changed=<ref>".into())
}

/// Stages `paths` again, after `tidy up --staged` rewrote them.
pub(crate) fn stage(paths: &[&Path]) -> Result<(), String> {
  let mut command = std::process::Command::new("git");
  command.arg("add").arg("--").args(paths);
  let status = command.status().map_err(|error| format!("can't run git: {error}"))?;
  match status.success() {
    true => Ok(()),
    false => Err("git add failed for the files tidy rewrote".into()),
  }
}
