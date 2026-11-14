//! Which paths git ignores, as git decides it: the `.gitignore` of each folder, the nearest first, then the
//! repository's `.git/info/exclude`, then the global excludes file. Outside a repository nothing is ignored, and a
//! nested repository starts over.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// The rules of a folder with a `.gitignore`, or of a repository's root.
pub(crate) struct Rules {
  own: Gitignore,
  /// The rules of the nearest folder above, in the same repository.
  parent: Option<Arc<Rules>>,
  repository: Arc<Repository>,
}

/// What applies in every folder of a repository.
struct Repository {
  exclude: Gitignore,
  global: Arc<Gitignore>,
}

/// The rules in effect in a folder: its own, or the nearest above it. `None` outside a repository.
pub(crate) type Here = Option<Arc<Rules>>;

impl Rules {
  /// Whether git ignores `path`, an entry of a folder these rules are in effect for, from the root.
  pub(crate) fn ignores(&self, path: &Path, folder: bool) -> bool {
    let mut rules = Some(self);
    while let Some(current) = rules {
      match current.own.matched(path, folder) {
        Match::Ignore(_) => return true,
        Match::Whitelist(_) => return false,
        Match::None => rules = current.parent.as_deref(),
      }
    }
    let Repository { exclude, global } = &*self.repository;
    match exclude.matched(path, folder) {
      Match::None => global.matched(path, folder).is_ignore(),
      other => other.is_ignore(),
    }
  }
}

/// The rules in effect in `folder`, from the root, given those of the folder above and what its listing holds: a
/// `.git` makes it the root of a repository of its own, and a `.gitignore` adds to the rules above.
pub(crate) fn enter(
  above: &Here,
  folder: &Path,
  git: bool,
  gitignore: bool,
  global: &Arc<Gitignore>,
) -> std::io::Result<Here> {
  if git {
    // A `.git` file, as in a submodule, points elsewhere, and has no exclude file here.
    let exclude = match folder.join(".git").is_dir() {
      true => read(folder, ".git/info/exclude")?,
      false => Gitignore::empty(),
    };
    let own = if gitignore { read(folder, ".gitignore")? } else { Gitignore::empty() };
    let repository = Arc::new(Repository { exclude, global: global.clone() });
    return Ok(Some(Arc::new(Rules { own, parent: None, repository })));
  }
  match above {
    Some(rules) if gitignore => Ok(Some(Arc::new(Rules {
      own: read(folder, ".gitignore")?,
      parent: Some(rules.clone()),
      repository: rules.repository.clone(),
    }))),
    _ => Ok(above.clone()),
  }
}

/// The rules in effect above `folder`, a folder a walk starts from, given from the root: those of its repository's
/// root and of every folder between.
pub(crate) fn above(folder: &Path, global: &Arc<Gitignore>) -> std::io::Result<Here> {
  let Some(root) = folder.ancestors().skip(1).find(|it| it.join(".git").exists()) else {
    return Ok(None);
  };
  let mut here = enter(&None, root, true, root.join(".gitignore").is_file(), global)?;
  let mut current = root.to_path_buf();
  let below = folder.parent().and_then(|parent| parent.strip_prefix(root).ok());
  for part in below.into_iter().flat_map(Path::components) {
    current.push(part);
    let (git, gitignore) = (current.join(".git").exists(), current.join(".gitignore").is_file());
    here = enter(&here, &current, git, gitignore, global)?;
  }
  Ok(here)
}

/// The ignore file `name` of `folder`. A missing file is empty, and git skips a line it can't parse, so only an I/O
/// error fails.
fn read(folder: &Path, name: &str) -> std::io::Result<Gitignore> {
  let path = folder.join(name);
  let mut builder = GitignoreBuilder::new(folder);
  if let Some(error) = builder.add(&path)
    && let Some(io) = io(&error)
    && io.kind() != std::io::ErrorKind::NotFound
  {
    return Err(std::io::Error::new(io.kind(), format!("{}: {io}", path.display())));
  }
  Ok(builder.build().unwrap_or_else(|_| Gitignore::empty()))
}

/// The I/O error inside an ignore error, if it is one.
fn io(error: &ignore::Error) -> Option<&std::io::Error> {
  match error {
    ignore::Error::Io(error) => Some(error),
    ignore::Error::WithPath { err, .. } | ignore::Error::WithLineNumber { err, .. } => io(err),
    ignore::Error::Partial(errors) => errors.iter().find_map(io),
    _ => None,
  }
}

/// The global excludes file from git's config, or an empty one.
pub(crate) fn global() -> Arc<Gitignore> {
  Arc::new(Gitignore::global().0)
}

/// `path` from the root, without `.` and `..`, as the rules are rooted.
pub(crate) fn absolute(path: &Path) -> PathBuf {
  let joined = match path.is_absolute() {
    true => path.to_path_buf(),
    false => std::env::current_dir().unwrap_or_default().join(path),
  };
  let mut absolute = PathBuf::new();
  for component in joined.components() {
    match component {
      Component::ParentDir => {
        absolute.pop();
      }
      Component::CurDir => {}
      other => absolute.push(other),
    }
  }
  absolute
}
