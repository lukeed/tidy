# Formatters: TypeScript

3309 files, 9.1 MB, from vite, storybook, rolldown, BrowserBox: the files that Prettier parses. Each tool checks every file; a time is the mean of 5 runs in each order, or 2 for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output. With one thread, tidy reads and formats the files in turn, while oxfmt still reads them on another thread, so its time doesn't include all of its reading. With embedded code formatted, Biome doesn't run, as it can't format it, and Prettier formats it too.

| Tool             | Embedded off | Against fastest | Same as Prettier | Embedded on | Against fastest | Same as Prettier |
| ---------------- | -----------: | --------------: | ---------------- | ----------: | --------------: | ---------------- |
| tidy             |        58 ms |           1.00x | 3308 (100.0%)    |       59 ms |           1.00x | 3308 (100.0%)    |
| bun              |        85 ms |           1.46x | 3308 (100.0%)    |      102 ms |           1.73x | 3308 (100.0%)    |
| oxfmt            |        96 ms |           1.65x | 3245 (98.1%)     |      261 ms |           4.41x | 3245 (98.1%)     |
| biome            |       249 ms |           4.29x | 3150 (95.2%)     |             |                 |                  |
| tidy (1 thread)  |       302 ms |           5.22x | 3308 (100.0%)    |      309 ms |           5.22x | 3308 (100.0%)    |
| oxfmt (1 thread) |       327 ms |           5.65x | 3245 (98.1%)     |      514 ms |           8.67x | 3245 (98.1%)     |
| prettier         |      7523 ms |         129.84x | reference        |     8065 ms |         136.12x | reference        |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, Prettier 3.9.9, oxfmt 0.75.0, Biome 2.6.4, bun fmt 1.4.4
