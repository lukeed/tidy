---
title: Option philosophy
summary: Why tidy has the options it has, and why it will not grow more.
group: About
order: 4
---

tidy's options are Prettier's options. That is the whole policy, and it decides every question about adding one.

## The rule

An option exists in tidy if and only if Prettier has it, or a plugin tidy absorbed has it.

That gives three groups:

1. **Prettier's own options**, under their own names: `width` for `printWidth`, `quote` for `singleQuote`, and so on. See [Options](/docs/options/) for the mapping.
2. **The options of plugins tidy built in.** `toml.*` is taplo's set, `dockerfile.*` is dprint-plugin-dockerfile's, `xml.*` is `@prettier/plugin-xml`'s, `ini.spaceAroundEquals` is `prettier-plugin-ini`'s. They keep their own names, under their language.
3. **`markdown.*`.** Prettier has one Markdown option; tidy has nine. This is the one real departure, and it exists because Prettier's Markdown printer normalises markers with no way to opt out. `markdown.preset: "preserve"` keeps them as written. The default, `"prettier"`, is Prettier's behaviour exactly.

## Why not more

Prettier's [option philosophy](https://prettier.io/docs/option-philosophy) argues that a formatter with many options is a formatter you argue about. tidy inherits that argument, plus one of its own: every option is a branch in the printer, and every branch is a place where tidy and Prettier can drift apart. The test suite compares tidy against Prettier over a corpus; an option Prettier does not have is an output no test can check.

So tidy will not grow an option for a style Prettier does not support. If you want that style, the place to ask is Prettier.

## Defaults

Every default is Prettier's default, with two exceptions, both visible and both reversible:

- **JSDoc comments are formatted**, as `prettier-plugin-jsdoc` does. Turn it off with `"js": { "jsdoc": false }`.
- **Imports are not sorted.** Turn it on with `js.sortImports`, or by listing a sorting plugin in `js.plugins`. See [Options](/docs/options/#js).

Anything else that differs from Prettier's output is a bug or a listed [deviation](/docs/deviations/), not a default.

## On `--width`

The width is 80, and 120 for Dockerfiles, because those are Prettier's and dprint's. It is not a hard limit: it is the width the printer tries to fit lines into. Long strings, long identifiers and some comments will exceed it, exactly as they do in Prettier.
