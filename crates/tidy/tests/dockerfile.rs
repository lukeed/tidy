//! Dockerfiles, with outputs that dprint-plugin-dockerfile 0.7.0 prints, where tidy agrees with it.
//! `tests/oracle/dockerfile` compares the rest with dprint.

use tidy::doc::Eol;
use tidy::dockerfile::{Options, format, is_dockerfile_path};

fn tidy(text: &str) -> String {
  tidy_with(text, &Options::default())
}

fn tidy_with(text: &str, options: &Options) -> String {
  let formatted = format(text, options).unwrap();
  assert_eq!(format(&formatted, options).unwrap(), formatted, "formatted again: {text:?}");
  formatted
}

#[test]
fn knows_dockerfile_paths() {
  for path in
    ["Dockerfile", "x/Containerfile", "Dockerfile.dev", "a/app.dockerfile", "web.Containerfile"]
  {
    assert!(is_dockerfile_path(path), "{path}");
  }
  for path in [
    "Dockerfile.json",
    "Dockerfile.md",
    "Dockerfile.dev.bak",
    "dockerfile-notes.txt",
    "x.dockerfile/a",
  ] {
    assert!(!is_dockerfile_path(path), "{path}");
  }
}

#[test]
fn formats_instructions() {
  let text = "from   node:22   as   build\nrun   echo   \"a    b\"   &&   echo c\nENV A 1\nLABEL a=\"b\"     c=\"d\"\nCMD [\"a\",   \"b\"]\n";
  let expected = "FROM node:22 AS build\nRUN echo \"a    b\" && echo c\nENV A=1\nLABEL a=\"b\" c=\"d\"\nCMD [\"a\", \"b\"]\n";
  assert_eq!(tidy(text), expected);
}

#[test]
fn breaks_values_past_the_width() {
  let options = Options { width: 30, ..Options::default() };
  let text = "ENV NAME=value OTHER=value THIRD=value\n";
  let expected = "ENV NAME=value \\\n    OTHER=value \\\n    THIRD=value\n";
  assert_eq!(tidy_with(text, &options), expected);
}

#[test]
fn keeps_heredocs_as_they_are() {
  let text = "RUN <<EOF\necho   \"keep   this\"\n  indented\nEOF\n";
  assert_eq!(tidy(text), text);
}

#[test]
fn keeps_what_docker_reads() {
  let cases = [
    // A quoted value keeps its escapes and its quotes.
    (
      "ENV A='a $HOME b' B=\"say \\\"hi\\\" now\"\n",
      "ENV A='a $HOME b' B=\"say \\\"hi\\\" now\"\n",
    ),
    // A value with a tab is quoted, so the tab doesn't split it.
    ("ENV A a\tb\n", "ENV A=\"a\tb\"\n"),
    // A bare line continuation continues.
    ("RUN a \\\n\\\n  b\n", "RUN a \\\n\\\n  b\n"),
    // Docker skips the empty lines after a continuation.
    ("RUN a \\\n\n  b\n", "RUN a \\\n  b\n"),
    // A keyword glued to its text isn't an instruction, and stays as it is.
    ("RUN-script is not one\n", "RUN-script is not one\n"),
  ];
  for (text, expected) in cases {
    assert_eq!(tidy(text), expected, "{text:?}");
  }
}

#[test]
fn indents_stages() {
  let options = Options { indent_stages: true, ..Options::default() };
  let text = "FROM a\nRUN b \\\n  c\n\n# next stage\nFROM d\nRUN e\n";
  let expected = "FROM a\n  RUN b \\\n    c\n\n# next stage\nFROM d\n  RUN e\n";
  assert_eq!(tidy_with(text, &options), expected);
}

#[test]
fn breaks_a_healthcheck_command_onto_its_own_line() {
  let options = Options { healthcheck_cmd_new_line: true, ..Options::default() };
  let text = "HEALTHCHECK --interval=5s CMD curl -f http://localhost/\n";
  let expected = "HEALTHCHECK --interval=5s \\\n  CMD curl -f http://localhost/\n";
  assert_eq!(tidy_with(text, &options), expected);
}

#[test]
fn keeps_the_byte_order_mark_and_writes_the_line_ending() {
  let options = Options { eol: Eol::Crlf, ..Options::default() };
  assert_eq!(tidy_with("\u{FEFF}from a\nrun b\n", &options), "\u{FEFF}FROM a\r\nRUN b\r\n");
}

#[test]
fn formats_dockerfiles_in_markdown() {
  let text = "# Build\n\n```dockerfile\nfrom   node\nrun   npm   ci\n```\n";
  let expected = "# Build\n\n```dockerfile\nFROM node\nRUN npm ci\n```\n";
  let options = tidy::Options::default();
  let printed =
    tidy::format(text, tidy::Language::Markdown, &options, None, &mut tidy::embed::Verbatim)
      .unwrap();
  assert_eq!(printed, expected);
}
