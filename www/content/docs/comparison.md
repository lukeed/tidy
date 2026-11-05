---
title: Comparison
summary: tidy next to Prettier, Biome, oxfmt and dprint, on output, languages and speed.
group: About
order: 3
---

Four other formatters cover the same ground. The difference that matters most is whether the output is Prettier's.

## At a glance

|                 | tidy     | Prettier   | Biome   | oxfmt             | dprint  |
| --------------- | -------- | ---------- | ------- | ----------------- | ------- |
| Output          | Prettier | Prettier   | its own | close to Prettier | its own |
| Written in      | Rust     | JavaScript | Rust    | Rust              | Rust    |
| Runtime needed  | none     | Node       | none    | none              | none    |
| Plugins         | none     | many       | none    | none              | many    |
| Also lints      | no       | no         | yes     | no                | no      |
| JS, TS, JSX     | yes      | yes        | yes     | yes               | yes     |
| CSS, SCSS, Less | yes      | yes        | yes     | yes               | yes     |
| HTML, Vue       | yes      | yes        | partial | partial           | yes     |
| Markdown        | yes      | yes        | yes     | yes               | yes     |
| YAML            | yes      | yes        | partial | yes               | yes     |
| JSON, JSONC     | yes      | yes        | yes     | yes               | yes     |
| TOML            | yes      | plugin     | no      | no                | plugin  |
| XML, SVG        | yes      | plugin     | no      | no                | plugin  |
| Dockerfile      | yes      | no         | no      | no                | plugin  |
| INI             | yes      | plugin     | no      | no                | no      |
| GraphQL         | yes      | yes        | yes     | yes               | no      |

## On output

Only tidy and Prettier print the same bytes. That is the whole point of tidy, and it is checked: the test suite runs Prettier over a 12,000-file corpus and compares byte for byte. The places where tidy prints something else are listed in [Deviations](/docs/deviations/), each with a reason.

"Close to Prettier" is a different claim. oxfmt aims at Prettier's style and differs in enough places that switching produces a diff. Biome and dprint do not aim at it at all.

If you have no existing formatting to preserve, this does not matter and any of these will serve you. If you do, it is the only thing that matters.

## On languages

tidy puts in one binary what Prettier needs four plugins for: XML, TOML, Dockerfiles and INI. Biome and oxfmt cover the JavaScript and style sheet core well, and little beyond it.

That also sets the ceiling. Prettier's plugin ecosystem reaches Astro, Svelte, Java, PHP, Solidity and more. tidy has no plugin system and never will — it is a fixed set of languages.

## On linting

Biome is a formatter and a linter in one tool. tidy is only a formatter, and expects a linter beside it. See [Linters](/docs/linters/) for turning off a linter's stylistic rules so the two do not fight.

## On speed

Checking every file in each corpus, all cores, on an M1 Max:

| Corpus          |  Files |       tidy | bun fmt |    oxfmt |   dprint |  Prettier |
| --------------- | -----: | ---------: | ------: | -------: | -------: | --------: |
| JavaScript      |  3,906 |  **78 ms** |  100 ms |   123 ms |          |  8,528 ms |
| TypeScript      |  3,309 |  **58 ms** |   85 ms |    96 ms |          |  7,523 ms |
| JSON            |  4,569 |  **86 ms** |  149 ms |   114 ms |   323 ms |  3,528 ms |
| Markdown        |  2,834 |  **37 ms** |   71 ms |    80 ms |   188 ms |  6,618 ms |
| **All but XML** | 12,071 | **280 ms** |  569 ms | 1,232 ms | 3,474 ms | 18,548 ms |

Every corpus, one-thread numbers, and the method are in [Benchmarks](/docs/benchmarks/).

Note that `bun fmt` and oxfmt are fast for JavaScript and much slower for markup: on HTML, oxfmt takes 237 ms where tidy takes 6.4 ms. The ranking depends on what you format.
