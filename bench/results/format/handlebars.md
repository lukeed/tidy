# Formatters: Handlebars

57 files, 0.1 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 3.8 ms | 1.00x |
| tidy (1 thread) | 6.0 ms | 1.57x |
| bun | 27.1 ms | 7.05x |
| prettier | 266.3 ms | 69.34x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, Prettier 3.9.9, @prettier/plugin-xml 3.4.2
