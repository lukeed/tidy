# Formatters: JavaScript

3906 files, 4.4 MB, from vite, storybook, rolldown, BrowserBox: the files that Prettier parses. Each tool checks every file; a time is the mean of 5 runs in each order, or 2 for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output. With one thread, tidy reads and formats the files in turn, while oxfmt still reads them on another thread, so its time doesn't include all of its reading. With embedded code formatted, Biome doesn't run, as it can't format it, and Prettier formats it too.

| Tool             | Embedded off | Against fastest | Same as Prettier | Embedded on | Against fastest | Same as Prettier |
| ---------------- | -----------: | --------------: | ---------------- | ----------: | --------------: | ---------------- |
| tidy             |        78 ms |           1.00x | 3906 (100.0%)    |       71 ms |           1.00x | 3906 (100.0%)    |
| bun              |       100 ms |           1.29x | 3906 (100.0%)    |       99 ms |           1.41x | 3906 (100.0%)    |
| oxfmt            |       123 ms |           1.58x | 3891 (99.6%)     |      208 ms |           2.94x | 3891 (99.6%)     |
| biome            |       212 ms |           2.73x | 3825 (97.9%)     |             |                 |                  |
| oxfmt (1 thread) |       223 ms |           2.87x | 3891 (99.6%)     |      333 ms |           4.71x | 3891 (99.6%)     |
| tidy (1 thread)  |       227 ms |           2.93x | 3906 (100.0%)    |      249 ms |           3.52x | 3906 (100.0%)    |
| prettier         |      8528 ms |         109.74x | reference        |     8722 ms |         123.47x | reference        |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, Prettier 3.9.9, oxfmt 0.75.0, Biome 2.6.4, bun fmt 1.4.4
