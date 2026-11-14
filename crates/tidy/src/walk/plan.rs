//! What to do with the paths a walk is given: which are files and which folders, which are the same, and which lie
//! inside a folder that is walked.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub(crate) struct Argument {
  /// As the user spelled it.
  pub(crate) path: PathBuf,
  pub(crate) folder: bool,
  /// Only set when some argument is a folder that others may lie in.
  pub(crate) canonical: Option<PathBuf>,
}

pub(crate) struct Plan {
  pub(crate) errors: Vec<(PathBuf, std::io::Error)>,
  pub(crate) arguments: Vec<Argument>,
  /// The arguments that lie in no folder argument.
  pub(crate) top: Vec<usize>,
  /// The arguments that lie in a folder argument, by canonical path.
  pub(crate) nested: HashMap<PathBuf, usize>,
}

pub(crate) fn plan(paths: &[PathBuf]) -> Plan {
  let mut plan =
    Plan { errors: Vec::new(), arguments: Vec::new(), top: Vec::new(), nested: HashMap::new() };
  let mut seen = HashSet::new();
  for path in paths {
    let metadata = match std::fs::metadata(path) {
      Ok(metadata) => metadata,
      Err(error) => {
        plan.errors.push((path.clone(), error));
        continue;
      }
    };
    if !metadata.is_dir() && !metadata.is_file() {
      let error = std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a file or a folder");
      plan.errors.push((path.clone(), error));
      continue;
    }
    if seen.insert(identity(path, &metadata)) {
      plan.arguments.push(Argument {
        path: path.clone(),
        folder: metadata.is_dir(),
        canonical: None,
      });
    }
  }
  if plan.arguments.len() < 2 || !plan.arguments.iter().any(|it| it.folder) {
    plan.top = (0..plan.arguments.len()).collect();
    return plan;
  }
  for argument in &mut plan.arguments {
    argument.canonical = std::fs::canonicalize(&argument.path).ok();
  }
  let roots: Vec<&Path> =
    plan.arguments.iter().filter(|it| it.folder).filter_map(|it| it.canonical.as_deref()).collect();
  for (index, argument) in plan.arguments.iter().enumerate() {
    let inside = argument.canonical.as_deref().is_some_and(|canonical| {
      roots.iter().any(|root| canonical != *root && canonical.starts_with(root))
    });
    match (inside, &argument.canonical) {
      (true, Some(canonical)) => {
        plan.nested.insert(canonical.clone(), index);
      }
      _ => plan.top.push(index),
    }
  }
  plan
}

/// What makes two paths the same file or folder: `(dev, ino)`, or elsewhere the canonical path.
#[cfg(unix)]
fn identity(_: &Path, metadata: &std::fs::Metadata) -> (u64, u64, PathBuf) {
  use std::os::unix::fs::MetadataExt;
  (metadata.dev(), metadata.ino(), PathBuf::new())
}

#[cfg(not(unix))]
fn identity(path: &Path, _: &std::fs::Metadata) -> (u64, u64, PathBuf) {
  (0, 0, std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned()))
}
