# Formatters: Handlebars

57 files, 118 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 3.7 ms | 1.00x |
| tidy (1 thread) | 5.5 ms | 1.48x |
| bun | 26.9 ms | 7.25x |
| oxfmt | 202.7 ms | 54.63x |
| prettier | 271.4 ms | 73.16x |

## Setup

- Date: 2026-11-02
- Commit: b74b961
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, oxfmt 0.75.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
