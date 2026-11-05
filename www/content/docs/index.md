---
title: What is tidy?
summary: tidy is a code formatter written in Rust. One binary prints what Prettier prints, for every language it supports.
group: About
order: 1
---

tidy is a code formatter. You give it a file and it prints that file formatted. It is a single native binary: it parses, formats and prints in Rust, with no Node process in the loop.

Its output is Prettier's. Not a style inspired by Prettier — the same bytes, checked against Prettier on a 12,000-file corpus on every change. Where tidy prints something else, it is listed in [Deviations](/docs/deviations/), with the reason.

```sh
$ curl -fsSL https://tidy.lukeed.com | sh
$ tidy . --write
```

## Why it exists

Prettier settled the argument about code style, and the cost of that is a Node process that reads, parses and prints every file. On a large repository that is tens of seconds in CI and a visible pause on save.

Several formatters are faster. Most ask you to accept a different style, which means a one-time diff across the whole repository and a new argument about the parts that changed. tidy takes the other path: keep the style, replace the engine.

That constraint shapes everything. A faster formatter that prints _almost_ Prettier is a formatter you cannot adopt without a migration.

## What you get

- **The same output.** 100% of 3,906 JavaScript files and 3,308 of 3,309 TypeScript files in the bench corpus match Prettier byte for byte.
- **One binary.** No `node_modules`, no plugin registration. XML, TOML, Dockerfiles and INI are built in, which in Prettier are four separate plugins.
- **Every language in one pass.** Styles and scripts inside HTML and Vue, code blocks and front matter inside Markdown, and CSS, GraphQL and HTML inside JavaScript template literals.
- **Stable output.** Prettier's output is not always a fixed point: run it twice and a few files change again. Where that happens, tidy prints where Prettier settles, so `--check` stays green.
- **Prettier's options.** `tidy.json` takes the options you already know, in the same shapes. [`.editorconfig`](/docs/editorconfig/) is read too.

## How fast

Checking every file of a 12,071-file corpus of every language except XML, using all cores on an M1 Max:

| Tool     |      Time |
| -------- | --------: |
| tidy     |    280 ms |
| bun fmt  |    569 ms |
| oxfmt    |  1,232 ms |
| dprint   |  3,474 ms |
| Prettier | 18,548 ms |

[Benchmarks](/docs/benchmarks/) has every corpus, the method, and the numbers with embedded code formatted too.

## What it does not do

tidy is a formatter, not a linter. It has no rules, no autofixes, and no opinions about your code beyond its layout. Use it next to a linter, not instead of one — see [Linters](/docs/linters/).

It also has no plugin system. The languages it formats are the ones in the binary. If you need a formatter for a language it does not know, tidy is not the tool.

## Where to go next

- [Install](/docs/install/) — a binary, npm, Homebrew or cargo.
- [CLI](/docs/cli/) — every flag, with examples.
- [Options](/docs/options/) — every option, and the Prettier option it matches.
- [Comparison](/docs/comparison/) — tidy next to Prettier, Biome, oxfmt and dprint.
