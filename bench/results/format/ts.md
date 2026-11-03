# Formatters: TypeScript

3309 files, 9.1 MB, from vite, storybook, rolldown, BrowserBox: the files that Prettier parses. Each tool checks every file; a time is the mean of 5 runs in each order, or 2 for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output. With one thread, tidy reads and formats the files in turn, while oxfmt still reads them on another thread, so its time doesn't include all of its reading.

| Tool | Time | Against fastest | Same as Prettier |
| --- | ---: | ---: | --- |
| tidy | 57 ms | 1.00x | 3308 (100.0%) |
| bun | 86 ms | 1.52x | 3308 (100.0%) |
| oxfmt | 100 ms | 1.77x | 3245 (98.1%) |
| biome | 241 ms | 4.25x | 3150 (95.2%) |
| tidy (1 thread) | 303 ms | 5.33x | 3308 (100.0%) |
| oxfmt (1 thread) | 338 ms | 5.96x | 3245 (98.1%) |
| prettier | 7549 ms | 133.12x | reference |

## Setup

- Date: 2026-11-03
- Commit: ec47772
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, Prettier 3.9.9, oxfmt 0.75.0, Biome 2.6.4, bun fmt 1.4.4
