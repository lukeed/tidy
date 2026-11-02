# Formatters: JavaScript

3906 files, 4.4 MB, from vite, storybook, rolldown, BrowserBox: the files that Prettier parses. Each tool checks every file; a time is the mean of 3 runs in each order, or 2 for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output.

| Tool | Time | Against fastest | Same as Prettier |
| --- | ---: | ---: | --- |
| bun | 128 ms | 1.00x | 3906 (100.0%) |
| oxfmt | 142 ms | 1.11x | 3891 (99.6%) |
| tidy | 211 ms | 1.65x | 3906 (100.0%) |
| biome | 245 ms | 1.91x | 3825 (97.9%) |
| oxfmt (1 thread) | 251 ms | 1.96x | 3891 (99.6%) |
| tidy (1 thread) | 357 ms | 2.79x | 3906 (100.0%) |
| prettier | 9266 ms | 72.32x | reference |

## Setup

- Date: 2026-11-02
- Commit: 6b62e4a, with changes not committed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, Prettier 3.9.9, oxfmt 0.75.0, Biome 2.6.4, bun fmt 1.4.4
