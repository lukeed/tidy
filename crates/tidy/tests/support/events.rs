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
        pending.push('\n');
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
  let text = words(&unbreak(pending));
  if !text.is_empty() {
    out.push(format!("Text({text:?})"));
  }
  pending.clear();
}

fn words(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` without the line breaks between two wide characters (Chinese, Japanese, or emoji, but
/// not Korean), with any spaces around them, which CSS renders as nothing.
fn unbreak(text: &str) -> String {
  let wide = |char: Option<char>| {
    char.is_some_and(|char| {
      let korean = matches!(char, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{A960}'..='\u{A97F}' | '\u{AC00}'..='\u{D7FF}');
      unicode_width::UnicodeWidthChar::width(char) == Some(2) && !korean
    })
  };
  let mut out = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(start) = rest.find(char::is_whitespace) {
    let run = rest[start..].len() - rest[start..].trim_start().len();
    let (before, space, after) = (&rest[..start], &rest[start..start + run], &rest[start + run..]);
    out.push_str(before);
    let joined =
      space.contains('\n') && wide(out.chars().next_back()) && wide(after.chars().next());
    if !joined {
      out.push_str(space);
    }
    rest = after;
  }
  out.push_str(rest);
  out
}
