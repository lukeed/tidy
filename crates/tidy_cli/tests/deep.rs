//! Deeply nested text: oxc's parser recurses without a limit, so every path must format it on a big stack.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// 2000 nested parentheses need more than the 2 MiB of a default spawned thread, and the 1 MiB of Windows' main
/// thread.
fn deep() -> String {
  let depth = 2000;
  format!("export const x = {}a{};\n", "a + (".repeat(depth), ")".repeat(depth))
}

struct Folder(PathBuf);

impl Folder {
  fn new(name: &str) -> Self {
    let root = std::env::temp_dir().join(format!("tidy-deep-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("create folder");
    std::fs::write(root.join("src/deep.ts"), deep()).expect("write file");
    Self(root)
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
  // tidy exits before it reads stdin when it doesn't need it.
  match stdin.write_all(input.as_bytes()) {
    Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
    other => other.expect("write stdin"),
  }
  drop(stdin);
  child.wait_with_output().expect("wait for tidy")
}

/// Asserts tidy finished with `code`, or gave up with exit 2, as a debug build's bigger frames may make it. A stack
/// overflow aborts instead: a signal on Unix, and exit code `0xC00000FD` on Windows.
fn handled(output: &Output, code: i32) {
  let stderr = String::from_utf8_lossy(&output.stderr);
  let gave_up =
    output.status.code() == Some(2) && stderr.contains("the text nests too deeply to format");
  assert!(output.status.code() == Some(code) || gave_up, "{:?}: {stderr}", output.status);
}

#[test]
fn checks_a_folder() {
  let folder = Folder::new("check");
  let output = tidy(&["check", "src"], "", &folder.0);
  handled(&output, 1);
}

#[test]
fn rewrites_a_folder() {
  let folder = Folder::new("up");
  handled(&tidy(&["up", "src"], "", &folder.0), 0);
}

#[test]
fn prints_one_file() {
  let folder = Folder::new("print");
  handled(&tidy(&["src/deep.ts"], "", &folder.0), 0);
}

#[test]
fn formats_stdin() {
  let folder = Folder::new("stdin");
  handled(&tidy(&["--stdin=deep.ts"], &deep(), &folder.0), 0);
}
