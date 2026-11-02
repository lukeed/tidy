# Formatters: Handlebars

57 files, 118 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 3.9 ms | 1.00x |
| tidy (1 thread) | 5.3 ms | 1.38x |
| bun | 27.1 ms | 7.00x |
| oxfmt | 207.8 ms | 53.62x |
| prettier | 274.2 ms | 70.76x |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, oxfmt 0.75.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
