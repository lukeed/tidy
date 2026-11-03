# Formatters: JavaScript

3906 files, 4.4 MB, from vite, storybook, rolldown, BrowserBox: the files that Prettier parses. Each tool checks every file; a time is the mean of 5 runs in each order, or 2 for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output. With one thread, tidy reads and formats the files in turn, while oxfmt still reads them on another thread, so its time doesn't include all of its reading.

| Tool | Time | Against fastest | Same as Prettier |
| --- | ---: | ---: | --- |
| tidy | 71 ms | 1.00x | 3906 (100.0%) |
| oxfmt | 119 ms | 1.67x | 3891 (99.6%) |
| bun | 146 ms | 2.05x | 3906 (100.0%) |
| oxfmt (1 thread) | 223 ms | 3.13x | 3891 (99.6%) |
| tidy (1 thread) | 226 ms | 3.16x | 3906 (100.0%) |
| biome | 228 ms | 3.20x | 3825 (97.9%) |
| prettier | 8538 ms | 119.71x | reference |

## Setup

- Date: 2026-11-03
- Commit: ec47772
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, Prettier 3.9.9, oxfmt 0.75.0, Biome 2.6.4, bun fmt 1.4.4
