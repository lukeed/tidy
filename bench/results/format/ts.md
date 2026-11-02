# Formatters: TypeScript

3309 files, 9.1 MB, from vite, storybook, rolldown, BrowserBox: the files that Prettier parses. Each tool checks every file; a time is the mean of 3 runs in each order, or 2 for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output.

| Tool | Time | Against fastest | Same as Prettier |
| --- | ---: | ---: | --- |
| bun | 85 ms | 1.00x | 3308 (100.0%) |
| oxfmt | 98 ms | 1.15x | 3245 (98.1%) |
| biome | 249 ms | 2.92x | 3150 (95.2%) |
| oxfmt (1 thread) | 337 ms | 3.95x | 3245 (98.1%) |
| prettier | 7513 ms | 88.10x | reference |

## Setup

- Date: 2026-11-01
- Commit: 43f91d8
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: Prettier 3.9.9, oxfmt 0.75.0, Biome 2.6.4, bun fmt 1.4.4
