//! Code in markup and Markdown, by the names of Prettier's parsers. See `js.rs`.

mod js;

pub(crate) use js::{Host, format, format_for, formats};
