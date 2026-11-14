//! Times ways to walk a folder tree, alone or with reading each matching file. It chose the design of `tidy::walk`.
//! `--real md` builds against mdtidy, which must sit next to this repo until it merges in.
//!
//!   walk-bench <strategy> <dir>... [--threads <n>] [--read] [--ext md,markdown] [--work <n>] [--real tidy|md]
//!              [--readers <n>] [--cap <n>] [--write] [--gate <n>]
//!   walk-bench ops <list file> --op open|stat|read|rewrite|... [--threads <n>] [--procs <n>]
//!
//! Strategies: serial, bulk (serial, with getattrlistbulk), queue (a mutex queue of folders, each folder's files
//! handled by the thread that listed it), steal (work stealing), steal-bulk, steal-at, ignore (the ignore crate's
//! parallel walker), sem (tidy's queue of folder and file tasks, with at most --readers threads listing or reading),
//! pipe (stealing readers send to --threads workers), pipe-queue (the same with a mutex queue of folders), and
//! pipe-tasks (readers share folder and file tasks). Each skips hidden folders and node_modules, and doesn't follow
//! links. `--real` runs tidy or mdtidy on each file, `--work` times. `ops` runs one call per listed path.

use crossbeam_deque::{Injector, Stealer, Worker};
use crossbeam_utils::Backoff;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
  File,
  Dir,
  Other,
}

struct Settings {
  /// Passes of simulated formatting over each file's bytes.
  work: usize,
  /// For `pipe`: how many threads walk and read.
  readers: usize,
  read: bool,
  /// Open each file without reading it.
  open: bool,
  extensions: Vec<String>,
  /// Run a real formatter instead of the simulated work: tidy or mdtidy.
  real: Real,
  markdown: mdtidy::Options,
  /// For `pipe`: the channel's capacity in files.
  capacity: usize,
  /// For `pipe`: write each file's bytes back from the worker, through at most `gate` writers at once (0: no limit).
  write: bool,
  gate: usize,
}

/// A counting semaphore.
struct Permits {
  free: Mutex<usize>,
  ready: Condvar,
}

impl Permits {
  fn with<R>(&self, run: impl FnOnce() -> R) -> R {
    let mut free = self.free.lock().unwrap();
    while *free == 0 {
      free = self.ready.wait(free).unwrap();
    }
    *free -= 1;
    drop(free);
    let result = run();
    *self.free.lock().unwrap() += 1;
    self.ready.notify_one();
    result
  }
}

#[derive(Clone, Copy, PartialEq)]
enum Real {
  None,
  Tidy,
  Md,
}

#[derive(Default)]
struct State {
  formatter: tidy::Formatter,
}

/// The work on one file: simulated, or a real formatter that reports whether the file changed.
fn work(state: &mut State, path: &Path, bytes: &[u8], settings: &Settings) -> usize {
  match settings.real {
    Real::None => format(bytes, settings.work),
    Real::Tidy => {
      let Ok(text) = std::str::from_utf8(bytes) else { return 0 };
      let name = path.to_string_lossy();
      let Some(language) = tidy::Language::of_path(&name) else { return 0 };
      let options = tidy::Options::default();
      let mut changed = 0;
      for _ in 0..settings.work.max(1) {
        let result = state.formatter.format(text, language, &options, Some(&name), &mut tidy::embed::Verbatim);
        changed = result.map_or(0, |output| (output != text) as usize);
      }
      changed
    }
    Real::Md => {
      let Ok(text) = std::str::from_utf8(bytes) else { return 0 };
      let mut changed = 0;
      for _ in 0..settings.work.max(1) {
        changed = (mdtidy::format(text, &settings.markdown) != text) as usize;
      }
      changed
    }
  }
}

#[derive(Default)]
struct Tally {
  files: usize,
  bytes: usize,
  buffer: Vec<u8>,
  state: State,
}

impl Tally {
  fn file(&mut self, path: &Path, settings: &Settings) {
    let matches = path.extension().is_some_and(|extension| {
      settings.extensions.is_empty() || settings.extensions.iter().any(|it| extension == it.as_str())
    });
    if !matches {
      return;
    }
    self.files += 1;
    if settings.open {
      drop(std::fs::File::open(path));
    }
    if settings.read {
      let Ok(mut file) = std::fs::File::open(path) else { return };
      self.buffer.clear();
      loop {
        if self.buffer.capacity() - self.buffer.len() < 16384 {
          self.buffer.reserve(65536);
        }
        let spare = self.buffer.spare_capacity_mut();
        let spare = unsafe { std::slice::from_raw_parts_mut(spare.as_mut_ptr().cast::<u8>(), spare.len()) };
        match file.read(spare) {
          Ok(0) | Err(_) => break,
          Ok(read) => unsafe { self.buffer.set_len(self.buffer.len() + read) },
        }
      }
      self.bytes += self.buffer.len();
      self.bytes += work(&mut self.state, path, &self.buffer, settings) & 1;
    }
  }
}

/// Simulated formatting: `passes` passes over the bytes.
fn format(bytes: &[u8], passes: usize) -> usize {
  let mut sum = 0usize;
  for pass in 0..passes {
    for &byte in bytes {
      sum = sum.wrapping_mul(31).wrapping_add(byte as usize ^ pass);
    }
  }
  std::hint::black_box(sum)
}

fn skipped(name: &OsStr) -> bool {
  name.as_bytes().first() == Some(&b'.') || name == "node_modules"
}

type List = fn(&Path, &mut Vec<(OsString, Kind)>);

/// `read_dir` and the type from `d_type`.
fn list_std(dir: &Path, out: &mut Vec<(OsString, Kind)>) {
  let Ok(entries) = std::fs::read_dir(dir) else { return };
  for entry in entries.flatten() {
    let Ok(kind) = entry.file_type() else { continue };
    let kind = if kind.is_dir() {
      Kind::Dir
    } else if kind.is_file() {
      Kind::File
    } else {
      Kind::Other
    };
    out.push((entry.file_name(), kind));
  }
}

#[cfg(not(target_os = "macos"))]
fn list_bulk(dir: &Path, out: &mut Vec<(OsString, Kind)>) {
  list_std(dir, out)
}

/// `getattrlistbulk`: the names and types of a whole folder in a few calls.
#[cfg(target_os = "macos")]
fn list_bulk(dir: &Path, out: &mut Vec<(OsString, Kind)>) {
  let mut path = dir.as_os_str().as_bytes().to_vec();
  path.push(0);
  let fd = unsafe { libc::open(path.as_ptr().cast(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
  if fd < 0 {
    return;
  }
  let mut list: libc::attrlist = unsafe { std::mem::zeroed() };
  list.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
  list.commonattr = libc::ATTR_CMN_RETURNED_ATTRS | libc::ATTR_CMN_NAME | libc::ATTR_CMN_OBJTYPE;
  thread_local! {
    static BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0u8; 64 * 1024]);
  }
  BUFFER.with_borrow_mut(|buffer| loop {
    let count = unsafe {
      libc::getattrlistbulk(fd, (&raw mut list).cast(), buffer.as_mut_ptr().cast(), buffer.len(), 0)
    };
    if count <= 0 {
      break;
    }
    let mut at = 0usize;
    for _ in 0..count {
      let read_u32 = |offset: usize| u32::from_ne_bytes(buffer[offset..offset + 4].try_into().unwrap());
      let length = read_u32(at) as usize;
      let mut field = at + 4 + std::mem::size_of::<libc::attribute_set_t>();
      let offset = i32::from_ne_bytes(buffer[field..field + 4].try_into().unwrap());
      let name_length = read_u32(field + 4) as usize;
      let start = (field as isize + offset as isize) as usize;
      let name = OsStr::from_bytes(&buffer[start..start + name_length.saturating_sub(1)]).to_owned();
      field += 8;
      let kind = match read_u32(field) {
        1 => Kind::File,
        2 => Kind::Dir,
        _ => Kind::Other,
      };
      out.push((name, kind));
      at += length;
    }
  });
  unsafe { libc::close(fd) };
}

fn serial(roots: &[PathBuf], list: List, settings: &Settings) -> (usize, usize) {
  let mut tally = Tally::default();
  let mut stack: Vec<PathBuf> = roots.to_vec();
  let mut entries = Vec::new();
  while let Some(dir) = stack.pop() {
    entries.clear();
    list(&dir, &mut entries);
    for (name, kind) in entries.drain(..) {
      match kind {
        Kind::Dir if !skipped(&name) => stack.push(dir.join(name)),
        Kind::File => tally.file(&dir.join(name), settings),
        _ => {}
      }
    }
  }
  (tally.files, tally.bytes)
}

fn queue(roots: &[PathBuf], threads: usize, settings: &Settings) -> (usize, usize) {
  let state = Mutex::new((roots.to_vec(), 0usize));
  let ready = Condvar::new();
  let totals = Mutex::new((0, 0));
  std::thread::scope(|scope| {
    for _ in 0..threads {
      scope.spawn(|| {
        let mut tally = Tally::default();
        let mut entries = Vec::new();
        loop {
          let dir = {
            let mut guard = state.lock().unwrap();
            loop {
              if let Some(dir) = guard.0.pop() {
                guard.1 += 1;
                break Some(dir);
              }
              if guard.1 == 0 {
                break None;
              }
              guard = ready.wait(guard).unwrap();
            }
          };
          let Some(dir) = dir else { break };
          entries.clear();
          list_std(&dir, &mut entries);
          let mut found = Vec::new();
          for (name, kind) in entries.drain(..) {
            match kind {
              Kind::Dir if !skipped(&name) => found.push(dir.join(name)),
              Kind::File => tally.file(&dir.join(name), settings),
              _ => {}
            }
          }
          let mut guard = state.lock().unwrap();
          let wakes = !found.is_empty() || guard.1 == 1;
          guard.0.extend(found);
          guard.1 -= 1;
          drop(guard);
          if wakes {
            ready.notify_all();
          }
        }
        let mut totals = totals.lock().unwrap();
        totals.0 += tally.files;
        totals.1 += tally.bytes;
      });
    }
  });
  totals.into_inner().unwrap()
}

fn steal(roots: &[PathBuf], threads: usize, list: List, settings: &Settings) -> (usize, usize) {
  let injector = Injector::new();
  for root in roots {
    injector.push(root.clone());
  }
  let pending = AtomicUsize::new(roots.len());
  let workers: Vec<Worker<PathBuf>> = (0..threads).map(|_| Worker::new_lifo()).collect();
  let stealers: Vec<Stealer<PathBuf>> = workers.iter().map(Worker::stealer).collect();
  let totals = Mutex::new((0, 0));
  std::thread::scope(|scope| {
    for (index, local) in workers.into_iter().enumerate() {
      let (injector, pending, stealers, totals) = (&injector, &pending, &stealers, &totals);
      scope.spawn(move || {
        let mut tally = Tally::default();
        let mut entries = Vec::new();
        let backoff = Backoff::new();
        loop {
          let task = local.pop().or_else(|| {
            std::iter::repeat_with(|| {
              injector.steal_batch_and_pop(&local).or_else(|| {
                stealers
                  .iter()
                  .enumerate()
                  .filter(|(other, _)| *other != index)
                  .map(|(_, stealer)| stealer.steal())
                  .collect()
              })
            })
            .find(|steal| !steal.is_retry())
            .and_then(|steal| steal.success())
          });
          let Some(dir) = task else {
            if pending.load(Ordering::Acquire) == 0 {
              break;
            }
            backoff.snooze();
            continue;
          };
          backoff.reset();
          entries.clear();
          list(&dir, &mut entries);
          for (name, kind) in entries.drain(..) {
            match kind {
              Kind::Dir if !skipped(&name) => {
                pending.fetch_add(1, Ordering::AcqRel);
                local.push(dir.join(name));
              }
              Kind::File => tally.file(&dir.join(name), settings),
              _ => {}
            }
          }
          pending.fetch_sub(1, Ordering::AcqRel);
        }
        let mut totals = totals.lock().unwrap();
        totals.0 += tally.files;
        totals.1 += tally.bytes;
      });
    }
  });
  totals.into_inner().unwrap()
}

/// Work stealing with folder descriptors: each folder is opened from its parent's descriptor, and each file from its
/// folder's, so the kernel never resolves a whole path again.
fn steal_at(roots: &[PathBuf], threads: usize, settings: &Settings) -> (usize, usize) {
  use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
  use std::sync::Arc;
  struct Task {
    parent: Option<Arc<OwnedFd>>,
    path: PathBuf,
  }
  let injector = Injector::new();
  for root in roots {
    injector.push(Task { parent: None, path: root.clone() });
  }
  let pending = AtomicUsize::new(roots.len());
  let workers: Vec<Worker<Task>> = (0..threads).map(|_| Worker::new_lifo()).collect();
  let stealers: Vec<Stealer<Task>> = workers.iter().map(Worker::stealer).collect();
  let totals = Mutex::new((0, 0));
  let open = |parent: Option<&OwnedFd>, path: &Path, flags: i32| -> Option<OwnedFd> {
    let name = match parent {
      Some(_) => path.file_name()?.as_bytes(),
      None => path.as_os_str().as_bytes(),
    };
    let mut c = name.to_vec();
    c.push(0);
    let fd = unsafe {
      libc::openat(parent.map_or(libc::AT_FDCWD, |it| it.as_raw_fd()), c.as_ptr().cast(), flags | libc::O_CLOEXEC)
    };
    (fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(fd) })
  };
  std::thread::scope(|scope| {
    for (index, local) in workers.into_iter().enumerate() {
      let (injector, pending, stealers, totals) = (&injector, &pending, &stealers, &totals);
      scope.spawn(move || {
        let mut tally = Tally::default();
        let backoff = Backoff::new();
        loop {
          let task = local.pop().or_else(|| {
            std::iter::repeat_with(|| {
              injector.steal_batch_and_pop(&local).or_else(|| {
                stealers
                  .iter()
                  .enumerate()
                  .filter(|(other, _)| *other != index)
                  .map(|(_, stealer)| stealer.steal())
                  .collect()
              })
            })
            .find(|steal| !steal.is_retry())
            .and_then(|steal| steal.success())
          });
          let Some(task) = task else {
            if pending.load(Ordering::Acquire) == 0 {
              break;
            }
            backoff.snooze();
            continue;
          };
          backoff.reset();
          if let Some(dir) = open(task.parent.as_deref(), &task.path, libc::O_RDONLY | libc::O_DIRECTORY) {
            let dir = Arc::new(dir);
            let stream = unsafe { libc::fdopendir(libc::dup(dir.as_raw_fd())) };
            if !stream.is_null() {
              loop {
                let entry = unsafe { libc::readdir(stream) };
                if entry.is_null() {
                  break;
                }
                let entry = unsafe { &*entry };
                let name = unsafe { std::ffi::CStr::from_ptr(entry.d_name.as_ptr()) }.to_bytes();
                if name == b"." || name == b".." {
                  continue;
                }
                let name = OsStr::from_bytes(name);
                match entry.d_type {
                  libc::DT_DIR if !skipped(name) => {
                    pending.fetch_add(1, Ordering::AcqRel);
                    local.push(Task { parent: Some(dir.clone()), path: task.path.join(name) });
                  }
                  libc::DT_REG => {
                    let path = task.path.join(name);
                    let matches = path.extension().is_some_and(|extension| {
                      settings.extensions.is_empty() || settings.extensions.iter().any(|it| extension == it.as_str())
                    });
                    if !matches {
                      continue;
                    }
                    tally.files += 1;
                    if settings.read || settings.open {
                      let Some(file) = open(Some(&dir), &path, libc::O_RDONLY) else { continue };
                      if settings.read {
                        let mut file = std::fs::File::from(file);
                        tally.buffer.clear();
                        let _ = file.read_to_end(&mut tally.buffer);
                        tally.bytes += tally.buffer.len();
                        tally.bytes += work(&mut tally.state, &path, &tally.buffer, settings) & 1;
                      }
                    }
                  }
                  _ => {}
                }
              }
              unsafe { libc::closedir(stream) };
            }
          }
          pending.fetch_sub(1, Ordering::AcqRel);
        }
        let mut totals = totals.lock().unwrap();
        totals.0 += tally.files;
        totals.1 += tally.bytes;
      });
    }
  });
  totals.into_inner().unwrap()
}

/// `settings.readers` threads walk and read, and `threads` threads format what they read.
fn pipe(roots: &[PathBuf], threads: usize, settings: &Settings, readers: Readers) -> (usize, usize) {
  let (send, receive) = crossbeam_channel::bounded::<(PathBuf, Vec<u8>)>(settings.capacity);
  let totals = Mutex::new((0, 0));
  let gate = Permits { free: Mutex::new(settings.gate), ready: Condvar::new() };
  std::thread::scope(|scope| {
    for _ in 0..threads {
      let (receive, totals, gate) = (receive.clone(), &totals, &gate);
      scope.spawn(move || {
        let (mut files, mut bytes) = (0, 0);
        let mut state = State::default();
        for (path, buffer) in receive {
          files += 1;
          bytes += buffer.len() + (work(&mut state, &path, &buffer, settings) & 1);
          if settings.write {
            let written = match settings.gate {
              0 => std::fs::write(&path, &buffer),
              _ => gate.with(|| std::fs::write(&path, &buffer)),
            };
            written.unwrap();
          }
        }
        let mut totals = totals.lock().unwrap();
        totals.0 += files;
        totals.1 += bytes;
      });
    }
    drop(receive);
    match readers {
      Readers::Steal => drop(steal_paths(roots, settings.readers, settings, &Mutex::new(Vec::new()), &send)),
      Readers::Queue => queue_paths(roots, settings.readers, settings, &send),
      Readers::Tasks => task_paths(roots, settings.readers, settings, &send),
    }
    drop(send);
  });
  totals.into_inner().unwrap()
}

/// One pool of threads that list, read and format, sharing tidy's queue of folder and file tasks, with at most
/// `settings.readers` threads inside a list or a read at once.
fn sem(roots: &[PathBuf], threads: usize, settings: &Settings) -> (usize, usize) {
  enum Task {
    Folder(PathBuf),
    File(PathBuf),
  }
  let permits = Permits { free: Mutex::new(settings.readers), ready: Condvar::new() };
  let state = Mutex::new((roots.iter().cloned().map(Task::Folder).collect::<Vec<_>>(), 0usize));
  let ready = Condvar::new();
  let totals = Mutex::new((0, 0));
  std::thread::scope(|scope| {
    for _ in 0..threads {
      scope.spawn(|| {
        let (mut files, mut bytes) = (0, 0);
        let mut worker = State::default();
        let mut entries = Vec::new();
        loop {
          let task = {
            let mut guard = state.lock().unwrap();
            loop {
              if let Some(task) = guard.0.pop() {
                guard.1 += 1;
                break Some(task);
              }
              if guard.1 == 0 {
                break None;
              }
              guard = ready.wait(guard).unwrap();
            }
          };
          let Some(task) = task else { break };
          let mut found = Vec::new();
          match task {
            Task::Folder(dir) => {
              entries.clear();
              permits.with(|| list_std(&dir, &mut entries));
              for (name, kind) in entries.drain(..) {
                match kind {
                  Kind::Dir if !skipped(&name) => found.push(Task::Folder(dir.join(name))),
                  Kind::File => {
                    let path = dir.join(name);
                    let matches = path.extension().is_some_and(|extension| {
                      settings.extensions.is_empty() || settings.extensions.iter().any(|it| extension == it.as_str())
                    });
                    if matches {
                      found.push(Task::File(path));
                    }
                  }
                  _ => {}
                }
              }
            }
            Task::File(path) => {
              if let Ok(buffer) = permits.with(|| std::fs::read(&path)) {
                files += 1;
                bytes += buffer.len() + (work(&mut worker, &path, &buffer, settings) & 1);
              }
            }
          }
          let mut guard = state.lock().unwrap();
          let wakes = !found.is_empty() || guard.1 == 1;
          guard.0.extend(found);
          guard.1 -= 1;
          drop(guard);
          if wakes {
            ready.notify_all();
          }
        }
        let mut totals = totals.lock().unwrap();
        totals.0 += files;
        totals.1 += bytes;
      });
    }
  });
  totals.into_inner().unwrap()
}

#[derive(Clone, Copy)]
enum Readers {
  Steal,
  Queue,
  Tasks,
}

/// Readers that share one queue of folder and file tasks, so that the files of one folder are read by all readers.
fn task_paths(roots: &[PathBuf], threads: usize, settings: &Settings, send: &crossbeam_channel::Sender<(PathBuf, Vec<u8>)>) {
  enum Task {
    Folder(PathBuf),
    File(PathBuf),
  }
  let state = Mutex::new((roots.iter().cloned().map(Task::Folder).collect::<Vec<_>>(), 0usize));
  let ready = Condvar::new();
  std::thread::scope(|scope| {
    for _ in 0..threads {
      let send = send.clone();
      let (state, ready) = (&state, &ready);
      scope.spawn(move || {
        let mut entries = Vec::new();
        loop {
          let task = {
            let mut guard = state.lock().unwrap();
            loop {
              if let Some(task) = guard.0.pop() {
                guard.1 += 1;
                break Some(task);
              }
              if guard.1 == 0 {
                break None;
              }
              guard = ready.wait(guard).unwrap();
            }
          };
          let Some(task) = task else { break };
          let mut found = Vec::new();
          match task {
            Task::Folder(dir) => {
              entries.clear();
              list_std(&dir, &mut entries);
              for (name, kind) in entries.drain(..) {
                match kind {
                  Kind::Dir if !skipped(&name) => found.push(Task::Folder(dir.join(name))),
                  Kind::File => {
                    let path = dir.join(name);
                    let matches = path.extension().is_some_and(|extension| {
                      settings.extensions.is_empty() || settings.extensions.iter().any(|it| extension == it.as_str())
                    });
                    if matches {
                      found.push(Task::File(path));
                    }
                  }
                  _ => {}
                }
              }
            }
            Task::File(path) => {
              if let Ok(bytes) = std::fs::read(&path) {
                let _ = send.send((path, bytes));
              }
            }
          }
          let mut guard = state.lock().unwrap();
          let wakes = !found.is_empty() || guard.1 == 1;
          guard.0.extend(found);
          guard.1 -= 1;
          drop(guard);
          if wakes {
            ready.notify_all();
          }
        }
      });
    }
  });
}

/// tidy's mutex queue of folders, as the design proposes for the readers, with each matching file read and sent.
fn queue_paths(roots: &[PathBuf], threads: usize, settings: &Settings, send: &crossbeam_channel::Sender<(PathBuf, Vec<u8>)>) {
  let state = Mutex::new((roots.to_vec(), 0usize));
  let ready = Condvar::new();
  std::thread::scope(|scope| {
    for _ in 0..threads {
      let send = send.clone();
      let (state, ready) = (&state, &ready);
      scope.spawn(move || {
        let mut entries = Vec::new();
        loop {
          let dir = {
            let mut guard = state.lock().unwrap();
            loop {
              if let Some(dir) = guard.0.pop() {
                guard.1 += 1;
                break Some(dir);
              }
              if guard.1 == 0 {
                break None;
              }
              guard = ready.wait(guard).unwrap();
            }
          };
          let Some(dir) = dir else { break };
          entries.clear();
          list_std(&dir, &mut entries);
          let mut found = Vec::new();
          let mut files = Vec::new();
          for (name, kind) in entries.drain(..) {
            match kind {
              Kind::Dir if !skipped(&name) => found.push(dir.join(name)),
              Kind::File => {
                let path = dir.join(name);
                let matches = path.extension().is_some_and(|extension| {
                  settings.extensions.is_empty() || settings.extensions.iter().any(|it| extension == it.as_str())
                });
                if matches {
                  files.push(path);
                }
              }
              _ => {}
            }
          }
          let mut guard = state.lock().unwrap();
          let wakes = !found.is_empty() || guard.1 == 1;
          guard.0.extend(found);
          guard.1 -= 1;
          drop(guard);
          if wakes {
            ready.notify_all();
          }
          for path in files {
            if let Ok(bytes) = std::fs::read(&path) {
              let _ = send.send((path, bytes));
            }
          }
        }
      });
    }
  });
}

/// `steal`, with each matching file read into a new buffer and sent.
fn steal_paths(
  roots: &[PathBuf],
  threads: usize,
  settings: &Settings,
  _paths: &Mutex<Vec<PathBuf>>,
  send: &crossbeam_channel::Sender<(PathBuf, Vec<u8>)>,
) -> usize {
  let injector = Injector::new();
  for root in roots {
    injector.push(root.clone());
  }
  let pending = AtomicUsize::new(roots.len());
  let workers: Vec<Worker<PathBuf>> = (0..threads).map(|_| Worker::new_lifo()).collect();
  let stealers: Vec<Stealer<PathBuf>> = workers.iter().map(Worker::stealer).collect();
  std::thread::scope(|scope| {
    for (index, local) in workers.into_iter().enumerate() {
      let (injector, pending, stealers, send) = (&injector, &pending, &stealers, send.clone());
      scope.spawn(move || {
        let mut entries = Vec::new();
        let backoff = Backoff::new();
        loop {
          let task = local.pop().or_else(|| {
            std::iter::repeat_with(|| {
              injector.steal_batch_and_pop(&local).or_else(|| {
                stealers
                  .iter()
                  .enumerate()
                  .filter(|(other, _)| *other != index)
                  .map(|(_, stealer)| stealer.steal())
                  .collect()
              })
            })
            .find(|steal| !steal.is_retry())
            .and_then(|steal| steal.success())
          });
          let Some(dir) = task else {
            if pending.load(Ordering::Acquire) == 0 {
              break;
            }
            backoff.snooze();
            continue;
          };
          backoff.reset();
          entries.clear();
          list_std(&dir, &mut entries);
          for (name, kind) in entries.drain(..) {
            match kind {
              Kind::Dir if !skipped(&name) => {
                pending.fetch_add(1, Ordering::AcqRel);
                local.push(dir.join(name));
              }
              Kind::File => {
                let path = dir.join(name);
                let matches = path.extension().is_some_and(|extension| {
                  settings.extensions.is_empty() || settings.extensions.iter().any(|it| extension == it.as_str())
                });
                if matches && let Ok(bytes) = std::fs::read(&path) {
                  let _ = send.send((path, bytes));
                }
              }
              _ => {}
            }
          }
          pending.fetch_sub(1, Ordering::AcqRel);
        }
      });
    }
  });
  0
}

fn ignore(roots: &[PathBuf], threads: usize, settings: &Settings) -> (usize, usize) {
  let mut builder = ignore::WalkBuilder::new(&roots[0]);
  for root in &roots[1..] {
    builder.add(root);
  }
  builder
    .standard_filters(false)
    .threads(threads)
    .filter_entry(|entry| !(entry.file_type().is_some_and(|it| it.is_dir()) && entry.depth() > 0 && skipped(entry.file_name())));
  let totals = Mutex::new((0, 0));
  builder.build_parallel().run(|| {
    let totals = &totals;
    let mut tally = Tally::default();
    Box::new(move |entry| {
      if let Ok(entry) = entry
        && entry.file_type().is_some_and(|it| it.is_file())
      {
        tally.file(entry.path(), settings);
        let mut totals = totals.lock().unwrap();
        totals.0 += std::mem::take(&mut tally.files);
        totals.1 += std::mem::take(&mut tally.bytes);
      }
      ignore::WalkState::Continue
    })
  });
  totals.into_inner().unwrap()
}

/// Runs one file system call on each path of a list file, on `threads` threads, or in `procs` processes of one
/// thread each, to find what limits scaling.
fn ops(list: &Path, op: &str, threads: usize, procs: usize, slice: (usize, usize)) -> (usize, usize) {
  let text = std::fs::read_to_string(list).unwrap();
  let paths: Vec<std::ffi::CString> = text
    .lines()
    .enumerate()
    .filter(|(index, _)| index % slice.1 == slice.0)
    .map(|(_, line)| std::ffi::CString::new(line).unwrap())
    .collect();
  if procs > 1 {
    let exe = std::env::current_exe().unwrap();
    let children: Vec<_> = (0..procs)
      .map(|index| {
        std::process::Command::new(&exe)
          .args(["ops", list.to_str().unwrap(), "--op", op, "--threads", "1", "--slice"])
          .arg(format!("{index}/{procs}"))
          .stdout(std::process::Stdio::null())
          .spawn()
          .unwrap()
      })
      .collect();
    for mut child in children {
      child.wait().unwrap();
    }
    return (paths.len() * procs, 0);
  }
  let next = AtomicUsize::new(0);
  let kept = Mutex::new(Vec::new());
  std::thread::scope(|scope| {
    for _ in 0..threads {
      scope.spawn(|| {
        let mut mine = Vec::new();
        let mut buffer = vec![0u8; 1 << 20];
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        loop {
          let index = next.fetch_add(1, Ordering::Relaxed);
          let Some(path) = paths.get(index) else { break };
          let path = path.as_ptr();
          unsafe {
            match op {
              "stat" => drop(libc::stat(path, &mut stat)),
              "lstat" => drop(libc::lstat(path, &mut stat)),
              "access" => drop(libc::access(path, libc::R_OK)),
              "open" => {
                let fd = libc::open(path, libc::O_RDONLY | libc::O_CLOEXEC);
                libc::close(fd);
              }
              "openonly" => mine.push(libc::open(path, libc::O_RDONLY | libc::O_CLOEXEC)),
              "read" => {
                let fd = libc::open(path, libc::O_RDONLY | libc::O_CLOEXEC);
                while libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) > 0 {}
                libc::close(fd);
              }
              "rewrite" => {
                let fd = libc::open(path, libc::O_RDONLY | libc::O_CLOEXEC);
                let read = libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len());
                libc::close(fd);
                let fd = libc::open(path, libc::O_WRONLY | libc::O_TRUNC | libc::O_CLOEXEC);
                libc::write(fd, buffer.as_ptr().cast(), read.max(0) as usize);
                libc::close(fd);
              }
              "readfstat" => {
                let fd = libc::open(path, libc::O_RDONLY | libc::O_CLOEXEC);
                libc::fstat(fd, &mut stat);
                libc::read(fd, buffer.as_mut_ptr().cast(), (stat.st_size as usize + 1).min(buffer.len()));
                libc::close(fd);
              }
              other => panic!("unknown op {other}"),
            }
          }
        }
        kept.lock().unwrap().extend(mine);
      });
    }
  });
  for fd in kept.into_inner().unwrap() {
    unsafe { libc::close(fd) };
  }
  (paths.len(), 0)
}

fn main() {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let strategy = args[0].clone();
  if strategy == "ops" {
    let limit = libc::rlimit { rlim_cur: 65536, rlim_max: libc::RLIM_INFINITY };
    unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) };
    let (mut op, mut threads, mut procs, mut slice) = (String::from("open"), 1, 1, (0, 1));
    let mut rest = args[2..].iter();
    while let Some(arg) = rest.next() {
      match arg.as_str() {
        "--op" => op = rest.next().unwrap().clone(),
        "--threads" => threads = rest.next().unwrap().parse().unwrap(),
        "--procs" => procs = rest.next().unwrap().parse().unwrap(),
        "--slice" => {
          let (index, count) = rest.next().unwrap().split_once('/').unwrap();
          slice = (index.parse().unwrap(), count.parse().unwrap());
        }
        other => panic!("unknown option {other}"),
      }
    }
    let (files, _) = ops(Path::new(&args[1]), &op, threads, procs, slice);
    println!("{files} files");
    return;
  }
  let mut roots = Vec::new();
  let mut threads = std::thread::available_parallelism().map_or(1, |it| it.get());
  let mut settings = Settings {
    work: 0,
    readers: 4,
    read: false,
    open: false,
    extensions: Vec::new(),
    real: Real::None,
    markdown: mdtidy::Options::default(),
    capacity: 256,
    write: false,
    gate: 0,
  };
  let mut rest = args[1..].iter();
  while let Some(arg) = rest.next() {
    match arg.as_str() {
      "--threads" => threads = rest.next().unwrap().parse().unwrap(),
      "--read" => settings.read = true,
      "--work" => settings.work = rest.next().unwrap().parse().unwrap(),
      "--readers" => settings.readers = rest.next().unwrap().parse().unwrap(),
      "--open" => settings.open = true,
      "--real" => {
        settings.real = match rest.next().unwrap().as_str() {
          "tidy" => Real::Tidy,
          "md" => Real::Md,
          other => panic!("unknown formatter {other}"),
        };
        settings.read = true;
      }
      "--write" => settings.write = true,
      "--gate" => settings.gate = rest.next().unwrap().parse().unwrap(),
      "--cap" => settings.capacity = rest.next().unwrap().parse().unwrap(),
      "--ext" => settings.extensions = rest.next().unwrap().split(',').map(str::to_owned).collect(),
      path => roots.push(PathBuf::from(path)),
    }
  }
  let (files, bytes) = match strategy.as_str() {
    "serial" => serial(&roots, list_std, &settings),
    "bulk" => serial(&roots, list_bulk, &settings),
    "queue" => queue(&roots, threads, &settings),
    "steal" => steal(&roots, threads, list_std, &settings),
    "steal-bulk" => steal(&roots, threads, list_bulk, &settings),
    "ignore" => ignore(&roots, threads, &settings),
    "steal-at" => steal_at(&roots, threads, &settings),
    "pipe" => pipe(&roots, threads, &settings, Readers::Steal),
    "pipe-queue" => pipe(&roots, threads, &settings, Readers::Queue),
    "sem" => sem(&roots, threads, &settings),
    "pipe-tasks" => pipe(&roots, threads, &settings, Readers::Tasks),
    other => panic!("unknown strategy {other}"),
  };
  println!("{files} files, {bytes} bytes");
}
