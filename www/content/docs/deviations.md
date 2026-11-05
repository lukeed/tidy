---
title: Deviations from Prettier
summary: Where tidy prints something other than Prettier, why, and how each case is tracked.
group: About
order: 5
---

tidy prints what Prettier 3.9.9 prints. Where it does not, the difference is deliberate, listed, and covered by a test.

The authoritative list is [`docs/deviations.md`](https://github.com/lukeed/tidy/blob/main/docs/deviations.md) in the repository, generated alongside the tests that enforce it. This page explains the rules it follows.

## The rule for keeping one

> Keep a deviation only where Prettier's output changes what the code means — it loses text, changes a value a program reads, or no longer parses. Where the difference is only layout, print what Prettier prints.

So a deviation is never a matter of taste. It is a case where running Prettier would change your program.

Most of them come from Bun's ports, which found these cases first. When Prettier's HTML printer was ported, the corrections that were only about layout — an SVG `<text>`, `<script>` and `<dialog>` on their own lines, splitting classes at ASCII whitespace only — were taken back out, so HTML prints what Prettier prints there.

## What the categories look like

- **Handlebars.** The largest group. Prettier's Glimmer printer drops markup in several positions, loses escapes before a mustache, and does not round-trip some path syntax. tidy keeps the input.
- **Style sheets.** Escaped spaces in selectors and values, non-ASCII lowercasing, custom property values. Prettier rewrites some of these in ways that change which element a rule matches.
- **HTML.** A `<pre>` with no end tag, an unquoted attribute value, whitespace-only `<pre>` and `<textarea>` content.
- **JSDoc.** On by default, which Prettier leaves alone. Turn it off with `"js": { "jsdoc": false }`.

## Stability

Separately from deviations, Prettier's output is not always a fixed point: for some inputs, running it twice gives a third result. Where that happens tidy prints the result Prettier settles on, so `tidy --check` does not fail on a file Prettier itself would reformat.

This is not listed as a deviation, because tidy and Prettier agree once Prettier has stopped changing its mind.

## How a new one gets added

1. A change that makes tidy print something other than Prettier is a deviation, and is never shipped silently.
2. It is added to `docs/deviations.md` with status `proposed`, and its inputs are added to `crates/tidy/tests/deviations.json`.
3. It is raised for sign-off. An approved entry gets a date; a rejected one is removed from the code, the file and the test.
4. `cargo test --test deviations` fails if tidy stops printing what an entry says.

## Markdown

Markdown follows Prettier 3.9.10, and its differences are generated from its own cases into [`docs/markdown-divergences.md`](https://github.com/lukeed/tidy/blob/main/docs/markdown-divergences.md).

## Other formatters tidy follows

Where Prettier has no printer, tidy follows the tool its printer was ported from:

| Language    | Follows                                    |
| ----------- | ------------------------------------------ |
| XML, SVG    | `@prettier/plugin-xml` 3.4.2               |
| TOML        | taplo 0.14.0, default settings             |
| Dockerfiles | dprint-plugin-dockerfile 0.7.0, first pass |
| INI         | `prettier-plugin-ini` 1.3.0                |
