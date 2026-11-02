//! Comments that apply to the whole file: bun's `is_file_level_comment` (`src/lint/fix.rs`), ESLint's
//! `directivesPattern` (`src/lint/utils/directives.rs`), and the part of `src/format/pragma.rs` that finds `@format`
//! and `@noformat` (MIT).

use crate::js::strings;

/// True if a comment applies to the whole file, judging by its text. That covers the `#!` line, pragmas that only work
/// above all code (`// @ts-nocheck`, `/// <reference path="a" />`, `/** @jsx h */`), linter directives that apply from
/// their position on (`/* eslint-disable */`), licenses, and file docs (`/** @module */`). `text` is the whole comment
/// and `content` is the part between its delimiters.
pub(super) fn is_file_level_comment(text: &[u8], content: &[u8]) -> bool {
  let content = strings::trim_js_whitespace(content);
  let starts_with_word = |word: &[u8]| {
    (content.strip_prefix(word))
      .is_some_and(|rest| rest.first().is_none_or(u8::is_ascii_whitespace))
  };
  let is_triple_slash_directive =
    (text.strip_prefix(b"///")).is_some_and(|rest| rest.trim_ascii_start().starts_with(b"<"));
  text.starts_with(b"#!")
    || is_triple_slash_directive
    // The bundler preserves `//!` and `/*!` comments.
    || text.get(2) == Some(&b'!')
    || match_directives_pattern(content)
      .is_some_and(|it| it.starts_with("eslint") && !it.ends_with("line"))
    || [&b"oxlint-disable"[..], b"oxlint-enable"].into_iter().any(starts_with_word)
    || FILE_IGNORE_DIRECTIVES.into_iter().any(starts_with_word)
    || starts_license(content)
    || has_file_level_tag(content)
}

/// Directives that ignore the whole file.
const FILE_IGNORE_DIRECTIVES: [&[u8]; 4] =
  [b"biome-ignore-all", b"deno-lint-ignore-file", b"deno-fmt-ignore-file", b"dprint-ignore-file"];

/// Longer names come before their prefixes.
const DIRECTIVES: [&str; 9] = [
  "eslint-disable-next-line",
  "eslint-disable-line",
  "eslint-disable",
  "eslint-enable",
  "eslint-env",
  "eslint",
  "exported",
  "globals",
  "global",
];

/// ESLint's `directivesPattern`: the directive that `text`, a comment's trimmed value, starts with.
fn match_directives_pattern(text: &[u8]) -> Option<&'static str> {
  if !matches!(text.first(), Some(b'e' | b'g')) {
    return None;
  }
  DIRECTIVES.into_iter().find(|directive| {
    text.starts_with(directive.as_bytes())
      && (text[directive.len()..].is_empty()
        || strings::js_whitespace_len(&text[directive.len()..]) > 0)
  })
}

/// True if `content` has a file-level `@tag` such as `@ts-nocheck` or `@license`. The tag must be a whole word, so
/// `@bun/types` doesn't match.
fn has_file_level_tag(content: &[u8]) -> bool {
  const TAGS: [&[u8]; 17] = [
    b"ts-nocheck",
    b"ts-check",
    b"bun",
    b"license",
    b"preserve",
    b"copyright",
    b"file",
    b"fileoverview",
    b"overview",
    b"module",
    b"packageDocumentation",
    b"flow",
    b"noflow",
    b"format",
    b"prettier",
    b"noformat",
    b"noprettier",
  ];
  let ends_a_word = |byte: &u8| byte.is_ascii_whitespace() || matches!(byte, b'*');
  let mut from = 0;
  while let Some(at) = strings::index_of_char_usize(content.get(from..).unwrap_or_default(), b'@') {
    let (at, start) = (from + at, from + at + 1);
    let rest = content.get(start..).unwrap_or_default();
    let is_part = |byte: &&u8| byte.is_ascii_alphanumeric() || matches!(byte, b'-');
    let len = rest.iter().take_while(is_part).count();
    let name = rest.get(..len).unwrap_or_default();
    from = start + len;
    let before = at.checked_sub(1).and_then(|it| content.get(it));
    // `@jsxImportSource`, `@jest-environment`, `@vitest-environment`
    let is_pragma = [&b"jsx"[..], b"jest-", b"vitest-"].iter().any(|it| name.starts_with(it));
    if (TAGS.contains(&name) || is_pragma)
      && before.is_none_or(|byte| ends_a_word(byte) || matches!(byte, b'/'))
      && content.get(from).is_none_or(ends_a_word)
    {
      return true;
    }
  }
  false
}

/// True if `content`, a comment's text, begins a license header.
fn starts_license(content: &[u8]) -> bool {
  let is_star = |byte: &&u8| matches!(byte, b'*') || byte.is_ascii_whitespace();
  let stars = content.iter().take_while(is_star).count();
  let text = content.get(stars..).unwrap_or_default();
  text.get(..9).is_some_and(|it| it.eq_ignore_ascii_case(b"copyright"))
    || strings::contains(content, b"SPDX-License-Identifier")
}

/// True if the block comment `text` has a line that starts with `@format`, `@prettier`, `@noformat` or `@noprettier`:
/// Prettier's `hasPragma` and `hasIgnorePragma` on a file that starts with it.
pub(super) fn has_any_pragma(text: &[u8]) -> bool {
  const NAMES: [&[u8]; 4] = [b"format", b"prettier", b"noformat", b"noprettier"];
  let Some(content) = text.strip_prefix(b"/*").and_then(|it| it.strip_suffix(b"*/")) else {
    return false;
  };
  if strings::contains(content, b"\xE2\x80\xA8") || strings::contains(content, b"\xE2\x80\xA9") {
    return false;
  }
  let content = content.strip_prefix(b"*").unwrap_or(content);
  content.split(|byte| matches!(byte, b'\n' | b'\r')).any(|line| {
    let spaces = |text: &[u8]| text.len() - text.iter().take_while(|it| **it == b' ').count();
    let line = &line[line.len() - spaces(line)..];
    let line = line.strip_prefix(b"*").map_or(line, |it| it.strip_prefix(b" ").unwrap_or(it));
    let Some(name) = line[line.len() - spaces(line)..].strip_prefix(b"@") else {
      return false;
    };
    let len = (0..name.len()).find(|at| strings::js_whitespace_len(&name[*at..]) > 0);
    NAMES.contains(&&name[..len.unwrap_or(name.len())])
  })
}
