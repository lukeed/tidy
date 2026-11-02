# Formatters: INI

75 files, 38 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 4.0 ms | 1.00x |
| tidy (1 thread) | 5.0 ms | 1.26x |
| prettier | 149.1 ms | 37.21x |

## Setup

- Date: 2026-11-01
- Commit: d19409c
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
