# Formatters: JavaScript

3906 files, 4.4 MB, from vite, storybook, rolldown, BrowserBox: the files that Prettier parses. Each tool checks every file; a time is the mean of 3 runs in each order, or 2 for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output.

| Tool | Time | Against fastest | Same as Prettier |
| --- | ---: | ---: | --- |
| tidy | 78 ms | 1.00x | 3906 (100.0%) |
| bun | 95 ms | 1.21x | 3906 (100.0%) |
| oxfmt | 122 ms | 1.56x | 3891 (99.6%) |
| oxfmt (1 thread) | 231 ms | 2.95x | 3891 (99.6%) |
| biome | 234 ms | 2.99x | 3825 (97.9%) |
| tidy (1 thread) | 247 ms | 3.15x | 3906 (100.0%) |
| prettier | 8510 ms | 108.78x | reference |

## Setup

- Date: 2026-11-02
- Commit: a28d34f
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, Prettier 3.9.9, oxfmt 0.75.0, Biome 2.6.4, bun fmt 1.4.4
