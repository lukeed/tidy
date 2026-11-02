# Formatters: JavaScript

3906 files, 4.4 MB, from vite, storybook, rolldown, BrowserBox: the files that Prettier parses. Each tool checks every file; a time is the mean of 3 runs in each order, or 2 for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output.

| Tool | Time | Against fastest | Same as Prettier |
| --- | ---: | ---: | --- |
| bun | 98 ms | 1.00x | 3906 (100.0%) |
| oxfmt | 117 ms | 1.20x | 3891 (99.6%) |
| biome | 201 ms | 2.06x | 3825 (97.9%) |
| oxfmt (1 thread) | 222 ms | 2.28x | 3891 (99.6%) |
| prettier | 8540 ms | 87.49x | reference |

## Setup

- Date: 2026-11-01
- Commit: 43f91d8
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: Prettier 3.9.9, oxfmt 0.75.0, Biome 2.6.4, bun fmt 1.4.4
