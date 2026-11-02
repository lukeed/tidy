//! The nesting depth limit: as deep as the stack allows. tidy's [`Stack`], with the names of bun's
//! `src/format/stack.rs` (MIT).

pub(crate) use crate::stack::Stack;

impl Stack {
  /// For code that builds a tree, and for the first recursive walk of a tree that a loop built.
  pub(crate) fn for_tree() -> Stack {
    Stack::new()
  }
}

/// bun's `StackCheck`: true while recursing deeper is safe.
pub(crate) struct StackCheck(Stack);

impl StackCheck {
  pub(crate) fn init() -> StackCheck {
    StackCheck(Stack::new())
  }

  pub(crate) fn is_safe_to_recurse(&self) -> bool {
    self.0.has_room()
  }
}
