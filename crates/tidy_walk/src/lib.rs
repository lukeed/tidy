//! A parallel walk of the files under the paths a user gives. It reads each wanted file, runs the caller's work on
//! it, and writes back what the work returns. The results come back sorted by path, with every error.
//!
//! ```text
//! paths ─ plan ─→ I/O threads: one queue of Folder, File and Write tasks ─→ workers: work(state, &File)
//!                    Folder: list, call `listing`, queue its folders and wanted files        │
//!                    File:   read, then send to the workers                                  │
//!                    Write:  write in place ◄────────────────────────────────────────────────┘
//! ```
//!
//! Opens, reads and writes only happen on the I/O threads, so their number limits how many files are open at once.
//! On macOS, opening files stops scaling at a few threads, system-wide, so there 6 I/O threads feed workers on
//! every core. Elsewhere, opening scales, and the I/O threads run the work themselves (`workers: 0`).
//!
//! Every argument, and every file in a walked folder, is worked on at most once:
//! - Arguments that are the same file or folder, by `(dev, ino)`, count once.
//! - An argument inside a walked folder is handled under its own spelling, when the walk reaches it, or after the
//!   walk if a skipped folder or a link lies on its way.
//! - Links found while walking are never followed.
//! - Files with more than one hard link are set aside and worked on once each, after the walk.

mod plan;
mod pool;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// The stack of each walk thread. oxc's parser recurses without a limit on nested text, and a stack overflow aborts
/// the whole process: bun's fixtures need up to 12 MiB, where a spawned thread has 2 MiB. Only the pages a thread
/// touches take memory, so the room costs nothing on ordinary files.
pub const STACK: usize = 64 << 20;

/// How to walk.
pub struct Options<'a> {
  /// Whether to work on a file found in a folder, by its folder and name. Files given as paths are always worked
  /// on.
  pub wanted: &'a (dyn Fn(&Path, &OsStr) -> bool + Sync),
  /// Whether to leave out a folder found while walking, by its parent and name.
  pub skip: &'a (dyn Fn(&Path, &OsStr) -> bool + Sync),
  /// Threads that list folders, read files and write them. At least 1.
  pub io: usize,
  /// Threads that run the work. With 0, the I/O threads run it, right after reading each file.
  pub workers: usize,
  /// The most bytes in flight: files read and not yet worked on, and writes not yet done. One file is always let
  /// through.
  pub bytes: usize,
}

impl Default for Options<'_> {
  /// Every file, without hidden folders and `node_modules`, with the thread counts that measured best: on macOS,
  /// 6 I/O threads and workers on every core, never more than 9% behind the best shape on corpora of tidy and
  /// mdtidy; elsewhere, I/O threads on every core that also run the work.
  fn default() -> Self {
    let cores = std::thread::available_parallelism().map_or(1, |count| count.get());
    let (io, workers) = match cfg!(target_os = "macos") {
      true => (cores.min(6), cores),
      false => (cores, 0),
    };
    Options { wanted: &everything, skip: &hidden_and_node_modules, io, workers, bytes: 256 << 20 }
  }
}

/// Wants every file.
pub fn everything(_: &Path, _: &OsStr) -> bool {
  true
}

/// Leaves out folders whose names start with `.`, and `node_modules`.
pub fn hidden_and_node_modules(_: &Path, name: &OsStr) -> bool {
  name.as_encoded_bytes().first() == Some(&b'.') || name == "node_modules"
}

/// A file to work on: its path, spelled from the path it was found under, and its bytes.
pub struct File {
  pub path: PathBuf,
  pub bytes: Vec<u8>,
}

/// What the work on a file returns.
pub struct Output<T> {
  pub value: T,
  /// New contents for the file, which an I/O thread writes in place.
  pub write: Option<Vec<u8>>,
}

/// An entry of a folder.
#[derive(Clone, Copy, Debug)]
pub struct Entry<'e> {
  pub name: &'e OsStr,
  pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  File,
  Folder,
  Link,
  Other,
}

#[derive(Debug)]
pub enum Error {
  Io(std::io::Error),
  /// The work, or one of the callbacks, panicked. The panic's message.
  Panic(String),
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::Io(error) => error.fmt(f),
      Error::Panic(message) => write!(f, "panicked: {message}"),
    }
  }
}

impl std::error::Error for Error {}

/// What a walk did.
pub struct Report<T> {
  /// What the work returned, sorted by path. A result whose write failed is an error instead.
  pub results: Vec<(PathBuf, T)>,
  /// Paths that could not be found, listed, read or written, and those whose work panicked, sorted by path.
  pub errors: Vec<(PathBuf, Error)>,
}

/// Walks `paths`, and runs `work` on each file. `listing` sees the entries of each folder before its files and
/// folders are queued. `state` makes the state of each thread that runs `work`, again after a panic.
pub fn run<S, T: Send>(
  paths: &[PathBuf],
  options: &Options,
  listing: impl Fn(&Path, &[Entry]) + Sync,
  state: impl Fn() -> S + Sync,
  work: impl Fn(&mut S, &File) -> Option<Output<T>> + Sync,
) -> Report<T> {
  let plan = plan::plan(paths);
  let mut report = Report {
    results: Vec::new(),
    errors: plan.errors.into_iter().map(|(path, error)| (path, Error::Io(error))).collect(),
  };
  let walk = pool::Walk {
    options,
    listing: &listing,
    state: &state,
    work: &work,
    arguments: &plan.arguments,
    nested: &plan.nested,
    claimed: plan.arguments.iter().map(|_| AtomicBool::new(false)).collect(),
  };
  let mut tasks: Vec<pool::Task> = plan.top.iter().map(|&index| walk.argument(index)).collect();
  let mut held = Vec::new();
  while !tasks.is_empty() {
    let done = pool::run(&walk, tasks, true);
    report.results.extend(done.results);
    report.errors.extend(done.errors);
    held.extend(done.held);
    tasks = plan
      .nested
      .values()
      .filter(|&&index| walk.claim(index))
      .map(|&index| walk.argument(index))
      .collect();
  }
  let grouped = group(held);
  if !grouped.is_empty() {
    let tasks =
      grouped.into_iter().map(|(path, argument)| pool::Task::File { path, argument }).collect();
    let done = pool::run(&walk, tasks, false);
    report.results.extend(done.results);
    report.errors.extend(done.errors);
  }
  report.results.sort_by(|(left, _), (right, _)| left.cmp(right));
  report.errors.sort_by(|(left, _), (right, _)| left.cmp(right));
  report
}

/// One path for each file with more than one hard link: its first argument's spelling, or its smallest path.
fn group(mut held: Vec<pool::Held>) -> Vec<(PathBuf, Option<usize>)> {
  held.sort_by(|left, right| {
    (left.identity, left.argument.is_none(), left.argument, &left.path).cmp(&(
      right.identity,
      right.argument.is_none(),
      right.argument,
      &right.path,
    ))
  });
  held.dedup_by_key(|it| it.identity);
  held.into_iter().map(|it| (it.path, it.argument)).collect()
}
