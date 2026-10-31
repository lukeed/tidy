//! The nesting depth limit for recursive parsers and printers: as deep as the stack allows. Ported from bun (MIT).

use std::cell::Cell;

/// How much stack recursion may use. A spawned thread has 2 MiB, which leaves room for the walks that come later.
const ROOM: usize = 1 << 20;

/// Nested input is parsed, walked and printed by recursive functions. Each calls [`Stack::has_room`] before it
/// recurses. If that returns false, it can continue with any placeholder, because the result is discarded. The
/// top-level caller checks [`Stack::ran_out`] at the end and gives up.
pub(crate) struct Stack {
  /// The address of a local of the function that made this, which the stack grows down from.
  base: usize,
  ran_out: Cell<bool>,
}

impl Stack {
  #[inline(never)]
  pub(crate) fn new() -> Stack {
    Stack { base: here(), ran_out: Cell::new(false) }
  }

  /// Stays false once the stack has run out, because the remaining work is discarded anyway.
  #[inline]
  pub(crate) fn has_room(&self) -> bool {
    let has_room = !self.ran_out.get() && self.base.abs_diff(here()) < ROOM;
    if !has_room {
      self.ran_out.set(true);
    }
    has_room
  }

  pub(crate) fn ran_out(&self) -> bool {
    self.ran_out.get()
  }
}

/// The address of a local, which tells how deep the stack is.
#[inline(always)]
fn here() -> usize {
  let local = 0u8;
  std::hint::black_box(&local) as *const u8 as usize
}
