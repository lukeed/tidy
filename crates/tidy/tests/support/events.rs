//! The events the semantics oracle compares, shared by its test and the `events` example.

use pulldown_cmark::{CodeBlockKind, Event, Options as Flags, Parser, Tag};

fn flags() -> Flags {
  Flags::ENABLE_TABLES
    | Flags::ENABLE_FOOTNOTES
    | Flags::ENABLE_STRIKETHROUGH
    | Flags::ENABLE_TASKLISTS
}

/// The events of `text`, with where lines break inside text not counting. Link labels match
/// with their whitespace collapsed, so they compare that way.
pub fn events(text: &str) -> Vec<String> {
  let mut out = Vec::new();
  let mut pending = String::new();
  for event in Parser::new_ext(text, flags()) {
    match &event {
      Event::Text(text) => {
        pending.push_str(text);
        continue;
      }
      Event::SoftBreak => {
        pending.push(' ');
        continue;
      }
      _ => {}
    }
    close(&mut out, &mut pending);
    out.push(match event {
      Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
        format!("Fenced({:?})", words(&info))
      }
      Event::Html(html) => format!("Html({:?})", html.trim_end()),
      Event::InlineHtml(html) => format!("InlineHtml({:?})", html.as_ref()),
      Event::Code(code) => format!("Code({:?})", code.as_ref()),
      Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
        format!(
          "Link({link_type:?}, {:?}, {:?}, {:?})",
          dest_url.as_ref(),
          title.as_ref(),
          words(&id)
        )
      }
      Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
        format!(
          "Image({link_type:?}, {:?}, {:?}, {:?})",
          dest_url.as_ref(),
          title.as_ref(),
          words(&id)
        )
      }
      event => format!("{event:?}"),
    });
  }
  close(&mut out, &mut pending);
  out
}

/// Close a run of text, whose whitespace only matters as a separator. Whitespace alone renders
/// as nothing.
fn close(out: &mut Vec<String>, pending: &mut String) {
  let text = words(pending);
  if !text.is_empty() {
    out.push(format!("Text({text:?})"));
  }
  pending.clear();
}

fn words(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}
