#![cfg(feature = "walk")]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use tidy::walk::{Error, Options, Output, Report, run};

/// A folder of files for one test, removed when dropped.
struct Tree(PathBuf);

impl Tree {
  fn new(name: &str, files: &[(&str, &str)]) -> Tree {
    let root = std::env::temp_dir().join(format!("tidy-walk-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (path, text) in files {
      let path = root.join(path);
      std::fs::create_dir_all(path.parent().unwrap()).unwrap();
      std::fs::write(path, text).unwrap();
    }
    Tree(root)
  }

  fn path(&self, path: &str) -> PathBuf {
    self.0.join(path)
  }

  /// The path of `path` relative to the root, as a string with `/` between its parts on every system.
  fn name(&self, path: &Path) -> String {
    let relative = path.strip_prefix(&self.0).unwrap_or(path);
    let parts: Vec<_> =
      relative.components().map(|part| part.as_os_str().to_string_lossy()).collect();
    parts.join("/")
  }
}

impl Drop for Tree {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

fn markdown(_: &Path, name: &OsStr) -> bool {
  Path::new(name).extension().is_some_and(|it| it == "md")
}

/// Both shapes of a walk: I/O threads that run the work, and I/O threads that feed workers.
fn shapes() -> [Options<'static>; 2] {
  [
    Options { wanted: &markdown, io: 3, workers: 0, ..Options::default() },
    Options { wanted: &markdown, io: 2, workers: 3, ..Options::default() },
  ]
}

/// Walks `paths`, with work that returns each file's text, and the names of the results and errors.
fn walk(tree: &Tree, paths: &[PathBuf], options: &Options) -> (Vec<String>, Vec<String>) {
  let report: Report<String> = run(
    paths,
    options,
    |_, _| {},
    || (),
    |(), file| {
      Some(Output { value: String::from_utf8_lossy(&file.bytes).into_owned(), write: None })
    },
  );
  let results = report.results.iter().map(|(path, _)| tree.name(path)).collect();
  let errors = report.errors.iter().map(|(path, _)| tree.name(path)).collect();
  (results, errors)
}

#[test]
fn walks_wanted_files_in_order_without_hidden_folders() {
  let tree = Tree::new(
    "order",
    &[
      ("b.md", ""),
      ("a.md", ""),
      ("a.txt", ""),
      ("docs/c.md", ""),
      (".github/d.md", ""),
      ("node_modules/x/e.md", ""),
    ],
  );
  for options in shapes() {
    let (results, errors) = walk(&tree, std::slice::from_ref(&tree.0), &options);
    assert_eq!(results, ["a.md", "b.md", "docs/c.md"]);
    assert!(errors.is_empty());
  }
}

#[test]
fn works_on_named_files_and_reports_missing_paths() {
  let tree = Tree::new("named", &[("a.txt", "x")]);
  for options in shapes() {
    let (results, errors) = walk(&tree, &[tree.path("a.txt"), tree.path("missing.md")], &options);
    assert_eq!(results, ["a.txt"]);
    assert_eq!(errors, ["missing.md"]);
  }
}

#[test]
fn counts_each_file_once() {
  let tree = Tree::new("once", &[("docs/a.md", ""), ("docs/sub/b.md", "")]);
  let paths =
    [tree.path("docs"), tree.path("./docs"), tree.path("docs/a.md"), tree.path("docs/sub")];
  for options in shapes() {
    let (results, _) = walk(&tree, &paths, &options);
    assert_eq!(results, ["docs/a.md", "docs/sub/b.md"]);
  }
  if tree.path("DOCS").exists() {
    let (results, _) = walk(&tree, &[tree.path("docs"), tree.path("DOCS")], &Options::default());
    assert_eq!(results.len(), 2, "{results:?}");
  }
}

#[test]
fn handles_arguments_in_folders_that_the_walk_skips() {
  let tree = Tree::new(
    "skipped",
    &[("a.md", ""), (".github/b.md", ""), (".github/c.md", ""), ("node_modules/x/readme.md", "")],
  );
  for options in shapes() {
    let paths = [tree.0.clone(), tree.path(".github/b.md"), tree.path("node_modules/x/readme.md")];
    let (results, _) = walk(&tree, &paths, &options);
    assert_eq!(results, [".github/b.md", "a.md", "node_modules/x/readme.md"]);
    let (results, _) = walk(&tree, &[tree.0.clone(), tree.path(".github")], &options);
    assert_eq!(results, [".github/b.md", ".github/c.md", "a.md"]);
  }
}

#[cfg(unix)]
#[test]
fn follows_links_it_is_given_and_no_others() {
  let tree = Tree::new("links", &[("readme.md", ""), ("other/x.md", ""), ("docs/a.md", "")]);
  std::os::unix::fs::symlink(tree.path("other"), tree.path("docs/linked")).unwrap();
  std::os::unix::fs::symlink(tree.path("readme.md"), tree.path("docs/readme")).unwrap();
  for options in shapes() {
    let (results, _) = walk(&tree, &[tree.path("docs")], &options);
    assert_eq!(results, ["docs/a.md"]);
    let (results, _) = walk(&tree, &[tree.path("docs/linked")], &options);
    assert_eq!(results, ["docs/linked/x.md"]);
    let (results, _) = walk(&tree, &[tree.0.clone(), tree.path("docs/readme")], &options);
    assert_eq!(results, ["docs/a.md", "docs/readme", "other/x.md"]);
  }
}

#[cfg(unix)]
#[test]
fn reports_unreadable_folders_and_goes_on() {
  use std::os::unix::fs::PermissionsExt;
  let tree = Tree::new("unreadable", &[("a.md", ""), ("locked/b.md", "")]);
  let locked = tree.path("locked");
  std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
  if std::fs::read_dir(&locked).is_ok() {
    return;
  }
  for options in shapes() {
    let (results, errors) = walk(&tree, std::slice::from_ref(&tree.0), &options);
    assert_eq!((results, errors), (vec!["a.md".to_owned()], vec!["locked".to_owned()]));
  }
  std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn reports_a_panic_for_its_file_and_keeps_the_rest() {
  let files: Vec<(String, &str)> = (0..20).map(|index| (format!("{index:02}.md"), "")).collect();
  let files: Vec<(&str, &str)> = files.iter().map(|(name, text)| (name.as_str(), *text)).collect();
  let tree = Tree::new("panic", &files);
  for options in shapes() {
    let report: Report<()> = run(
      std::slice::from_ref(&tree.0),
      &options,
      |_, _| {},
      || (),
      |(), file| {
        assert!(!file.path.ends_with("07.md"), "boom");
        Some(Output { value: (), write: None })
      },
    );
    assert_eq!(report.results.len(), 19);
    assert!(
      matches!(&report.errors[..], [(path, Error::Panic(message))] if path.ends_with("07.md") && message == "boom")
    );
  }
}

#[test]
fn writes_what_the_work_returns() {
  let files: Vec<(String, String)> =
    (0..50).map(|index| (format!("{index:02}.md"), "x".repeat(100))).collect();
  let files: Vec<(&str, &str)> =
    files.iter().map(|(name, text)| (name.as_str(), text.as_str())).collect();
  let tree = Tree::new("write", &files);
  let tight = Options { wanted: &markdown, io: 2, workers: 3, bytes: 1, ..Options::default() };
  for options in shapes().into_iter().chain([tight]) {
    let report: Report<usize> = run(
      std::slice::from_ref(&tree.0),
      &options,
      |_, _| {},
      || (),
      |(), file| {
        let mut bytes = file.bytes.clone();
        bytes.push(b'!');
        Some(Output { value: bytes.len(), write: Some(bytes) })
      },
    );
    assert_eq!(report.results.len(), 50);
    assert!(report.errors.is_empty());
  }
  assert_eq!(
    std::fs::read_to_string(tree.path("07.md")).unwrap(),
    format!("{}!!!", "x".repeat(100))
  );
}

#[cfg(unix)]
#[test]
fn works_on_a_hard_linked_file_once() {
  let tree = Tree::new("hard", &[("a.md", ""), ("c.md", "")]);
  std::fs::hard_link(tree.path("a.md"), tree.path("b.md")).unwrap();
  for options in shapes() {
    let (results, _) = walk(&tree, std::slice::from_ref(&tree.0), &options);
    assert_eq!(results, ["a.md", "c.md"]);
    let (results, _) = walk(&tree, &[tree.0.clone(), tree.path("b.md")], &options);
    assert_eq!(results, ["b.md", "c.md"]);
  }
}

#[test]
fn sees_each_folder_before_its_files() {
  let tree = Tree::new("listing", &[("a.md", ""), ("docs/b.md", ""), ("docs/tidy.json", "")]);
  let listed = std::sync::Mutex::new(Vec::new());
  let report: Report<bool> = run(
    std::slice::from_ref(&tree.0),
    &shapes()[1],
    |dir, entries| {
      let names: Vec<String> =
        entries.iter().map(|it| it.name.to_string_lossy().into_owned()).collect();
      listed.lock().unwrap().push((dir.to_owned(), names));
    },
    || (),
    |(), file| {
      let dir = file.path.parent().unwrap();
      Some(Output { value: listed.lock().unwrap().iter().any(|(it, _)| it == dir), write: None })
    },
  );
  assert!(report.results.iter().all(|(_, seen)| *seen));
  let listed = listed.into_inner().unwrap();
  let docs = listed.iter().find(|(dir, _)| dir.ends_with("docs")).unwrap();
  assert!(docs.1.contains(&"tidy.json".to_owned()));
}

/// A git repository's files: a `.git` folder marks its root.
fn repository(name: &str, files: &[(&str, &str)]) -> Tree {
  let mut all = vec![(".git/HEAD", "ref: refs/heads/main\n")];
  all.extend_from_slice(files);
  Tree::new(name, &all)
}

#[test]
fn leaves_out_what_gitignore_ignores() {
  let tree = repository(
    "gitignore",
    &[
      (".gitignore", "dist/\n*.gen.md\n"),
      ("a.md", ""),
      ("b.gen.md", ""),
      ("dist/c.md", ""),
      ("docs/d.md", ""),
      ("docs/e.gen.md", ""),
    ],
  );
  for options in shapes() {
    assert_eq!(walk(&tree, std::slice::from_ref(&tree.0), &options).0, ["a.md", "docs/d.md"]);
  }
}

#[test]
fn applies_each_folders_gitignore_below_it_with_negations() {
  let tree = repository(
    "nested-gitignore",
    &[
      (".gitignore", "*.gen.md\n"),
      ("a.gen.md", ""),
      ("docs/.gitignore", "!keep.gen.md\ndrafts.md\n"),
      ("docs/keep.gen.md", ""),
      ("docs/other.gen.md", ""),
      ("docs/drafts.md", ""),
      ("drafts.md", ""),
    ],
  );
  for options in shapes() {
    assert_eq!(
      walk(&tree, std::slice::from_ref(&tree.0), &options).0,
      ["docs/keep.gen.md", "drafts.md"]
    );
  }
}

#[test]
fn applies_the_gitignore_above_a_folder_it_is_given() {
  let tree = repository(
    "parent-gitignore",
    &[(".gitignore", "build/\n"), ("src/a.md", ""), ("src/build/b.md", "")],
  );
  for options in shapes() {
    assert_eq!(walk(&tree, &[tree.path("src")], &options).0, ["src/a.md"]);
  }
}

#[test]
fn reads_the_repositorys_exclude_file() {
  let tree =
    repository("exclude", &[(".git/info/exclude", "local.md\n"), ("a.md", ""), ("local.md", "")]);
  for options in shapes() {
    assert_eq!(walk(&tree, std::slice::from_ref(&tree.0), &options).0, ["a.md"]);
  }
}

#[test]
fn ignores_gitignore_outside_a_repository() {
  let tree = Tree::new("no-repository", &[(".gitignore", "*.md\n"), ("a.md", "")]);
  for options in shapes() {
    assert_eq!(walk(&tree, std::slice::from_ref(&tree.0), &options).0, ["a.md"]);
  }
}

#[test]
fn starts_over_in_a_nested_repository() {
  let tree = repository(
    "nested-repository",
    &[
      (".gitignore", "*.gen.md\n"),
      ("a.gen.md", ""),
      ("vendor/.git/HEAD", ""),
      ("vendor/b.gen.md", ""),
    ],
  );
  for options in shapes() {
    assert_eq!(walk(&tree, std::slice::from_ref(&tree.0), &options).0, ["vendor/b.gen.md"]);
  }
}

#[test]
fn works_on_ignored_files_it_is_given() {
  let tree = repository("named-ignored", &[(".gitignore", "*.gen.md\n"), ("a.gen.md", "")]);
  for options in shapes() {
    assert_eq!(walk(&tree, &[tree.path("a.gen.md")], &options).0, ["a.gen.md"]);
  }
}

#[test]
fn can_leave_gitignore_alone() {
  let tree = repository("no-gitignore", &[(".gitignore", "*.gen.md\n"), ("a.gen.md", "")]);
  for options in shapes() {
    let options = Options { gitignore: false, ..options };
    assert_eq!(walk(&tree, std::slice::from_ref(&tree.0), &options).0, ["a.gen.md"]);
  }
}
