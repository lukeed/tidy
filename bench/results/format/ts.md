# Formatters: TypeScript

3309 files, 9.1 MB, from vite, storybook, rolldown, BrowserBox: the files that Prettier parses. Each tool checks every file; a time is the mean of 3 runs in each order, or 2 for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output.

| Tool | Time | Against fastest | Same as Prettier |
| --- | ---: | ---: | --- |
| tidy | 65 ms | 1.00x | 3308 (100.0%) |
| bun | 94 ms | 1.45x | 3308 (100.0%) |
| oxfmt | 99 ms | 1.53x | 3245 (98.1%) |
| biome | 261 ms | 4.04x | 3150 (95.2%) |
| tidy (1 thread) | 323 ms | 5.00x | 3308 (100.0%) |
| oxfmt (1 thread) | 335 ms | 5.19x | 3245 (98.1%) |
| prettier | 7663 ms | 118.62x | reference |

## Setup

- Date: 2026-11-02
- Commit: a28d34f
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, Prettier 3.9.9, oxfmt 0.75.0, Biome 2.6.4, bun fmt 1.4.4
