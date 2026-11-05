---
title: Why tidy
summary: What tidy is for, who should use it, and when another formatter is the better answer.
group: About
order: 2
---

Prettier is the style most JavaScript projects already use. The problem is not the style. The problem is what it costs to apply it.

## The cost of Prettier

Prettier runs on Node. Every file is read into a JavaScript string, parsed into a JavaScript tree, printed through a JavaScript document printer, and written back. On a 12,000-file repository that is about 18 seconds with every core busy. In CI it is a job. On save it is long enough to notice.

That cost is not a flaw in Prettier's design. It is what a formatter written in JavaScript costs.

## The usual trade

The fast formatters ask you to change style. Biome, dprint and oxfmt each print something close to Prettier, and each differs in places. Close is the hard part: a formatter that prints _almost_ Prettier gives you a diff across every file the first time you run it, and an argument about each hunk.

For a new project that is a fine trade. For a repository with history, open branches and a team, it is a migration.

## What tidy trades instead

tidy keeps the style and replaces the engine. The same input gives the same bytes, so adopting it is a no-op diff: run `tidy . --write` on a formatted repository and nothing changes.

The trade is scope. tidy has no plugin system, so the languages it formats are the ones compiled into the binary. If you need a formatter for a language it does not know, tidy cannot be extended to cover it.

## Use tidy when

- You already use Prettier and want it to be fast.
- You want one binary in CI instead of a `node_modules` install.
- You format more than JavaScript: HTML, Vue, Markdown, YAML, TOML, XML and Dockerfiles are in the same binary.
- Your repository is large enough that formatting time is a line item.

## Use something else when

- **You are starting fresh and like another style.** Biome's or dprint's defaults are good, and matching Prettier buys you nothing.
- **You want one tool for formatting and linting.** tidy does not lint. Biome does both.
- **You need a formatter for a language tidy does not know.** Prettier's plugin ecosystem is the reason to stay.
- **You rely on a Prettier plugin for style.** Plugins that change printing, such as Tailwind class sorting, have no equivalent here. Import sorting is the exception: see [Options](/docs/options/#js).

## On being a port

tidy is not a reimplementation from the spec. It ports Prettier's document printer and each of its language printers, mostly through Bun's Rust ports, and checks the result against Prettier on every change. That is why the output matches, and it is why tidy tracks a specific Prettier version rather than "Prettier style" in general. See [Technical details](/docs/technical-details/) and [Credits](/docs/credits/).
