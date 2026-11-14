//! The threads of a walk: I/O threads, which list folders, read files and write them, and workers, which run the
//! work. They share one state behind a mutex, with a condition variable for each kind of thread.

use super::ignored::{self, Here};
use super::plan::Argument;
use super::{Entry, Error, File, Kind, Options, Output};
use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::io::Read;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// How many files may wait for a worker.
const QUEUED: usize = 64;

pub(crate) enum Task {
  Folder {
    path: PathBuf,
    canonical: Option<PathBuf>,
    /// Set when the walk reads `.gitignore` files.
    ignore: Option<Ignore>,
  },
  /// `argument` is the index of the argument that the path spells, if any.
  File { path: PathBuf, argument: Option<usize> },
}

/// Where a folder is, from the root, and the git rules in effect above it: `None` until they are found, for a folder
/// the walk starts from.
pub(crate) struct Ignore {
  absolute: PathBuf,
  above: Option<Here>,
}

/// New contents to write, and what the work returned for them.
struct Write<T> {
  path: PathBuf,
  bytes: Vec<u8>,
  value: T,
}

/// A file with more than one hard link, set aside until the walk ends.
pub(crate) struct Held {
  pub(crate) identity: (u64, u64),
  pub(crate) path: PathBuf,
  pub(crate) argument: Option<usize>,
}

/// What a walk needs from its caller, and what it knows about its arguments.
pub(crate) struct Walk<'w, S, T> {
  pub(crate) options: &'w Options<'w>,
  pub(crate) listing: &'w (dyn Fn(&Path, &[Entry]) + Sync),
  pub(crate) state: &'w (dyn Fn() -> S + Sync),
  pub(crate) work: &'w (dyn Fn(&mut S, &File) -> Option<Output<T>> + Sync),
  pub(crate) arguments: &'w [Argument],
  /// The global excludes file, when the walk reads `.gitignore` files.
  pub(crate) global: Option<Arc<ignore::gitignore::Gitignore>>,
  pub(crate) nested: &'w HashMap<PathBuf, usize>,
  /// Whether each argument has been handled, by the walk reaching it or after the walk.
  pub(crate) claimed: Vec<AtomicBool>,
}

impl<S, T> Walk<'_, S, T> {
  /// Claims the argument at `index`. Returns false if it was claimed already.
  pub(crate) fn claim(&self, index: usize) -> bool {
    !self.claimed[index].swap(true, Ordering::AcqRel)
  }

  pub(crate) fn argument(&self, index: usize) -> Task {
    let argument = &self.arguments[index];
    match argument.folder {
      true => Task::Folder {
        path: argument.path.clone(),
        canonical: argument.canonical.clone(),
        ignore: self
          .global
          .as_ref()
          .map(|_| Ignore { absolute: ignored::absolute(&argument.path), above: None }),
      },
      false => Task::File { path: argument.path.clone(), argument: Some(index) },
    }
  }
}

/// What the threads of one run found.
pub(crate) struct Done<T> {
  pub(crate) results: Vec<(PathBuf, T)>,
  pub(crate) errors: Vec<(PathBuf, Error)>,
  pub(crate) held: Vec<Held>,
}

impl<T> Default for Done<T> {
  fn default() -> Self {
    Done { results: Vec::new(), errors: Vec::new(), held: Vec::new() }
  }
}

struct State<T> {
  tasks: Vec<Task>,
  /// Taken before tasks, so that the bytes they hold are freed.
  writes: Vec<Write<T>>,
  files: VecDeque<File>,
  /// I/O threads and workers that are busy with a task or a file.
  io: usize,
  workers: usize,
  /// Bytes in flight: files waiting for or being worked on, and writes not yet done.
  bytes: usize,
  /// I/O threads waiting for bytes to be freed.
  waiting: usize,
}

struct Shared<'s, 'w, S, T> {
  walk: &'s Walk<'w, S, T>,
  state: Mutex<State<T>>,
  io: Condvar,
  workers: Condvar,
  /// Set files with more than one hard link aside.
  hold: bool,
}

impl<S, T> Shared<'_, '_, S, T> {
  fn lock(&self) -> MutexGuard<'_, State<T>> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn wake(&self) {
    self.io.notify_all();
    self.workers.notify_all();
  }
}

/// Marks a thread as busy until it is dropped, even by a panic.
struct Busy<'b, 's, 'w, S, T> {
  shared: &'b Shared<'s, 'w, S, T>,
  worker: bool,
}

impl<S, T> Drop for Busy<'_, '_, '_, S, T> {
  /// Wakes the threads that can now end: workers once no more files can come, and everyone once all is done.
  fn drop(&mut self) {
    let mut state = self.shared.lock();
    match self.worker {
      true => state.workers -= 1,
      false => state.io -= 1,
    }
    let read = state.io == 0 && state.tasks.is_empty();
    let done = read && state.writes.is_empty() && state.workers == 0 && state.files.is_empty();
    drop(state);
    if done {
      self.shared.wake();
    } else if read {
      self.shared.workers.notify_all();
    }
  }
}

/// Runs `tasks`, and everything they lead to, to the end.
pub(crate) fn run<S, T: Send>(walk: &Walk<'_, S, T>, tasks: Vec<Task>, hold: bool) -> Done<T> {
  let state = State {
    tasks,
    writes: Vec::new(),
    files: VecDeque::new(),
    io: 0,
    workers: 0,
    bytes: 0,
    waiting: 0,
  };
  let shared =
    Shared { walk, state: Mutex::new(state), io: Condvar::new(), workers: Condvar::new(), hold };
  let shared = &shared;
  std::thread::scope(|scope| {
    let io = (0..walk.options.io.max(1)).map(|_| spawn(scope, move || io(shared)));
    let io: Vec<_> = io.collect();
    let workers: Vec<_> =
      (0..walk.options.workers).map(|_| spawn(scope, move || worker(shared))).collect();
    let mut done = Done::default();
    for thread in io.into_iter().chain(workers) {
      let Ok(found) = thread.join() else { continue };
      done.results.extend(found.results);
      done.errors.extend(found.errors);
      done.held.extend(found.held);
    }
    done
  })
}

/// Spawns a thread with a [`STACK`](super::STACK)-sized stack, as `scope.spawn` would.
fn spawn<'scope, R: Send + 'scope>(
  scope: &'scope std::thread::Scope<'scope, '_>,
  run: impl FnOnce() -> R + Send + 'scope,
) -> std::thread::ScopedJoinHandle<'scope, R> {
  let builder = std::thread::Builder::new().stack_size(super::STACK);
  builder.spawn_scoped(scope, run).expect("failed to spawn thread")
}

fn io<S, T: Send>(shared: &Shared<'_, '_, S, T>) -> Done<T> {
  let mut done = Done::default();
  let mut state = None;
  loop {
    let job = {
      let mut guard = shared.lock();
      loop {
        if let Some(write) = guard.writes.pop() {
          guard.io += 1;
          break Some(Err(write));
        }
        if let Some(task) = guard.tasks.pop() {
          guard.io += 1;
          break Some(Ok(task));
        }
        if guard.io == 0 && guard.workers == 0 && guard.files.is_empty() {
          break None;
        }
        guard = shared.io.wait(guard).unwrap_or_else(PoisonError::into_inner);
      }
    };
    let Some(job) = job else {
      shared.wake();
      return done;
    };
    let _busy = Busy { shared, worker: false };
    match job {
      Ok(Task::Folder { path, canonical, ignore }) => {
        folder(shared, path, canonical, ignore, &mut done)
      }
      Ok(Task::File { path, argument }) => file(shared, path, argument, &mut done, &mut state),
      Err(pending) => write(shared, pending, &mut done),
    }
  }
}

fn worker<S, T: Send>(shared: &Shared<'_, '_, S, T>) -> Done<T> {
  let mut done = Done::default();
  let mut state = None;
  loop {
    let file = {
      let mut guard = shared.lock();
      loop {
        if let Some(file) = guard.files.pop_front() {
          guard.workers += 1;
          break Some(file);
        }
        if guard.io == 0 && guard.tasks.is_empty() {
          break None;
        }
        guard = shared.workers.wait(guard).unwrap_or_else(PoisonError::into_inner);
      }
    };
    let Some(file) = file else {
      shared.wake();
      return done;
    };
    let _busy = Busy { shared, worker: true };
    let read = file.bytes.len();
    let output = work(shared.walk, &mut state, &file, &mut done);
    let mut guard = shared.lock();
    let wrote = match output {
      Some(Output { value, write: Some(bytes) }) => {
        guard.bytes = guard.bytes - read + bytes.len();
        guard.writes.push(Write { path: file.path, bytes, value });
        true
      }
      Some(Output { value, write: None }) => {
        guard.bytes -= read;
        done.results.push((file.path, value));
        false
      }
      None => {
        guard.bytes -= read;
        false
      }
    };
    let waiting = guard.waiting > 0;
    drop(guard);
    if waiting {
      shared.io.notify_all();
    } else if wrote {
      shared.io.notify_one();
    }
  }
}

/// Runs the work on `file`. A panic is an error for its path, and the state is made again for the next file.
fn work<S, T>(
  walk: &Walk<'_, S, T>,
  state: &mut Option<S>,
  file: &File,
  done: &mut Done<T>,
) -> Option<Output<T>> {
  let output = catch_unwind(AssertUnwindSafe(|| {
    let state = state.get_or_insert_with(|| (walk.state)());
    (walk.work)(state, file)
  }));
  output.unwrap_or_else(|panic| {
    *state = None;
    done.errors.push((file.path.clone(), Error::Panic(message(&*panic))));
    None
  })
}

fn folder<S, T: Send>(
  shared: &Shared<'_, '_, S, T>,
  path: PathBuf,
  canonical: Option<PathBuf>,
  ignore: Option<Ignore>,
  done: &mut Done<T>,
) {
  let walk = shared.walk;
  let entries = match std::fs::read_dir(&path) {
    Ok(entries) => entries,
    Err(error) => return done.errors.push((path, Error::Io(error))),
  };
  let mut list: Vec<(OsString, Kind)> = Vec::new();
  for entry in entries {
    let entry = match entry {
      Ok(entry) => entry,
      Err(error) => {
        done.errors.push((path.clone(), Error::Io(error)));
        continue;
      }
    };
    let name = entry.file_name();
    match entry.file_type() {
      Ok(kind) if kind.is_symlink() => list.push((name, Kind::Link)),
      Ok(kind) if kind.is_dir() => list.push((name, Kind::Folder)),
      Ok(kind) if kind.is_file() => list.push((name, Kind::File)),
      Ok(_) => list.push((name, Kind::Other)),
      Err(error) => done.errors.push((path.join(&name), Error::Io(error))),
    }
  }
  let here = ignore.as_ref().zip(walk.global.as_ref()).map(|(ignore, global)| {
    let above = match &ignore.above {
      Some(above) => Ok(above.clone()),
      None => ignored::above(&ignore.absolute, global),
    };
    let git = list.iter().any(|(name, _)| name == ".git");
    let gitignore = list.iter().any(|(name, kind)| name == ".gitignore" && *kind == Kind::File);
    above.and_then(|above| ignored::enter(&above, &ignore.absolute, git, gitignore, global))
  });
  let here = match here {
    Some(Ok(here)) => here,
    Some(Err(error)) => return done.errors.push((path, Error::Io(error))),
    None => None,
  };
  let ignored = |name: &std::ffi::OsStr, folder: bool| match (&here, &ignore) {
    (Some(rules), Some(ignore)) => rules.ignores(&ignore.absolute.join(name), folder),
    _ => false,
  };
  let found = catch_unwind(AssertUnwindSafe(|| {
    let entries: Vec<Entry> = list.iter().map(|(name, kind)| Entry { name, kind: *kind }).collect();
    (walk.listing)(&path, &entries);
    let mut found = Vec::new();
    for (name, kind) in &list {
      if let Some(canonical) = canonical.as_ref().filter(|_| !walk.nested.is_empty())
        && let Some(&index) = walk.nested.get(&canonical.join(name))
      {
        if walk.claim(index) {
          found.push(walk.argument(index));
        }
        continue;
      }
      match kind {
        Kind::Folder if !(walk.options.skip)(&path, name) && !ignored(name, true) => {
          found.push(Task::Folder {
            path: path.join(name),
            canonical: canonical.as_ref().map(|it| it.join(name)),
            ignore: ignore.as_ref().map(|ignore| Ignore {
              absolute: ignore.absolute.join(name),
              above: Some(here.clone()),
            }),
          })
        }
        Kind::File if (walk.options.wanted)(&path, name) && !ignored(name, false) => {
          found.push(Task::File { path: path.join(name), argument: None })
        }
        _ => {}
      }
    }
    found
  }));
  match found {
    Ok(found) if found.is_empty() => {}
    Ok(found) => {
      let many = found.len() > 1;
      shared.lock().tasks.extend(found);
      match many {
        true => shared.io.notify_all(),
        false => shared.io.notify_one(),
      }
    }
    Err(panic) => done.errors.push((path, Error::Panic(message(&*panic)))),
  }
}

fn file<S, T: Send>(
  shared: &Shared<'_, '_, S, T>,
  path: PathBuf,
  argument: Option<usize>,
  done: &mut Done<T>,
  state: &mut Option<S>,
) {
  let (bytes, links, identity) = match read(&path) {
    Ok(read) => read,
    Err(error) => return done.errors.push((path, Error::Io(error))),
  };
  if shared.hold && links > 1 {
    return done.held.push(Held { identity, path, argument });
  }
  let file = File { path, bytes };
  let options = shared.walk.options;
  if options.workers == 0 {
    return match work(shared.walk, state, &file, done) {
      Some(Output { value, write: Some(bytes) }) => match std::fs::write(&file.path, bytes) {
        Ok(()) => done.results.push((file.path, value)),
        Err(error) => done.errors.push((file.path, Error::Io(error))),
      },
      Some(Output { value, write: None }) => done.results.push((file.path, value)),
      None => {}
    };
  }
  let size = file.bytes.len();
  let mut guard = shared.lock();
  loop {
    let room =
      guard.bytes == 0 || (guard.bytes + size <= options.bytes && guard.files.len() < QUEUED);
    if room {
      break;
    }
    if let Some(pending) = guard.writes.pop() {
      drop(guard);
      write(shared, pending, done);
      guard = shared.lock();
      continue;
    }
    guard.waiting += 1;
    guard = shared.io.wait(guard).unwrap_or_else(PoisonError::into_inner);
    guard.waiting -= 1;
  }
  guard.bytes += size;
  guard.files.push_back(file);
  drop(guard);
  shared.workers.notify_one();
}

fn write<S, T>(
  shared: &Shared<'_, '_, S, T>,
  Write { path, bytes, value }: Write<T>,
  done: &mut Done<T>,
) {
  match std::fs::write(&path, &bytes) {
    Ok(()) => done.results.push((path, value)),
    Err(error) => done.errors.push((path, Error::Io(error))),
  }
  let mut guard = shared.lock();
  guard.bytes -= bytes.len();
  let waiting = guard.waiting > 0;
  drop(guard);
  if waiting {
    shared.io.notify_all();
  }
}

/// The bytes of the file at `path`, how many hard links it has, and its `(dev, ino)`. The size that `fstat` gives is
/// only a first capacity: the file is read to its end.
fn read(path: &Path) -> std::io::Result<(Vec<u8>, u64, (u64, u64))> {
  let file = std::fs::File::open(path)?;
  let metadata = file.metadata()?;
  let mut bytes = Vec::with_capacity(metadata.len() as usize);
  (&file).take(u64::MAX).read_to_end(&mut bytes)?;
  let (links, identity) = links(&metadata);
  Ok((bytes, links, identity))
}

#[cfg(unix)]
fn links(metadata: &std::fs::Metadata) -> (u64, (u64, u64)) {
  use std::os::unix::fs::MetadataExt;
  (metadata.nlink(), (metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn links(_: &std::fs::Metadata) -> (u64, (u64, u64)) {
  (1, (0, 0))
}

fn message(panic: &(dyn std::any::Any + Send)) -> String {
  match (panic.downcast_ref::<&str>(), panic.downcast_ref::<String>()) {
    (Some(message), _) => (*message).to_owned(),
    (_, Some(message)) => message.clone(),
    _ => String::new(),
  }
}
