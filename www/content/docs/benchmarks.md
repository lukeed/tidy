---
title: Benchmarks
summary: Every corpus, every tool, and the method behind the numbers on this site.
group: Misc
order: 1
---

Every tool checks every file of each corpus, using all cores, on an M1 Max. Times are the mean of both run orders. **Embedded code is excluded**, so that every tool does the same amount of work.

| Corpus          |  Files |       tidy | bun fmt |    oxfmt |   dprint |  Prettier |
| --------------- | -----: | ---------: | ------: | -------: | -------: | --------: |
| JavaScript      |  3,906 |  **78 ms** |  100 ms |   123 ms |          |  8,528 ms |
| TypeScript      |  3,309 |  **58 ms** |   85 ms |    96 ms |          |  7,523 ms |
| JSON            |  4,569 |  **86 ms** |  149 ms |   114 ms |   323 ms |  3,528 ms |
| CSS             |    300 | **7.2 ms** |   35 ms |    47 ms |    25 ms |    654 ms |
| SCSS            |    101 | **4.6 ms** |   32 ms |    46 ms |    11 ms |    340 ms |
| Less            |    396 | **9.7 ms** |   37 ms |    48 ms |    58 ms |  1,491 ms |
| HTML            |    151 | **6.4 ms** |   34 ms |   237 ms |    14 ms |    327 ms |
| Vue             |    230 | **7.3 ms** |   34 ms |   435 ms |   205 ms |    684 ms |
| Handlebars      |     57 | **3.7 ms** |   30 ms |   201 ms |          |    279 ms |
| Markdown        |  2,834 |  **37 ms** |   71 ms |    80 ms |   188 ms |  6,618 ms |
| YAML            |    435 |  **24 ms** |   35 ms |    46 ms |   104 ms |  1,271 ms |
| TOML            |    375 | **8.6 ms** |   36 ms |    49 ms |    22 ms |           |
| GraphQL         |  2,511 |  **37 ms** |   63 ms |    72 ms |   271 ms |  1,988 ms |
| XML             |  6,784 | **130 ms** |         |          |   616 ms |  6,017 ms |
| Dockerfiles     |     25 | **3.6 ms** |         |          |   3.7 ms |    458 ms |
| INI             |     75 | **3.7 ms** |         |          |          |    147 ms |
| **All but XML** | 12,071 | **280 ms** |  569 ms | 1,232 ms | 3,474 ms | 18,548 ms |

A blank cell means the tool does not format that corpus.

## With embedded code

Formatting the code inside each file as well — styles and scripts in HTML, code blocks in Markdown — widens the gap, because that is more parsing for everyone and because not every tool does it:

| Tool     | All but XML |
| -------- | ----------: |
| tidy     |  **286 ms** |
| bun fmt  |      609 ms |
| oxfmt    |       2.4 s |
| Prettier |      30.9 s |

tidy adds 6 ms to do it. Prettier adds 12 seconds.

## Reading these numbers

**XML is listed apart** because its corpus is 6,784 files, over half the total, which would dominate a combined figure and flatter whichever tool is best at XML.

**The ranking depends on what you format.** `bun fmt` and oxfmt are fast on JavaScript and slow on markup: oxfmt takes 237 ms on 151 HTML files, where tidy takes 6.4 ms. A JavaScript-only repository sees a much smaller gap than these totals suggest.

**Throughput, not latency.** Every number is a whole corpus with every core busy. Formatting one file on save is a different measurement, dominated by process start — which is where a native binary is furthest ahead of a Node process.

## Method

- All cores, unless the full results say otherwise. The repository also has one-thread numbers.
- Each pair of tools is run in both orders and the two means averaged, so that a warm page cache does not favour whichever ran second.
- Every tool runs in check mode where it has one, so none of them pays for writing files.
- The corpora are real files from open-source projects, not generated input.

## The full results

Tables with one-thread times, and with Biome, taplo, yamlfmt, lightningcss and oxc's CSS formatter, are in [`bench/results/format`](https://github.com/lukeed/tidy/tree/main/bench/results/format).

The scripts are [`bench/run.ts`](https://github.com/lukeed/tidy/blob/main/bench/run.ts) and [`bench/js.ts`](https://github.com/lukeed/tidy/blob/main/bench/js.ts). Run them yourself:

```sh
$ bun bench/run.ts
```

## Versions

| Tool     | Version                                              |
| -------- | ---------------------------------------------------- |
| Prettier | 3.9.9                                                |
| bun fmt  | 1.4                                                  |
| oxfmt    | 0.75.0                                               |
| dprint   | with its TOML, Dockerfile, Markdown and JSON plugins |

Numbers move as these tools change. If a result here is stale, the scripts above are the way to check.
