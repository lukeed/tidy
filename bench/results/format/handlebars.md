# Formatters: Handlebars

57 files, 118 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier. With embedded code formatted, only tidy, bun fmt, oxfmt and Prettier run, as the other tools can't format it.

| Tool | Embedded off | Against tidy | Embedded on | Against tidy |
| --- | ---: | ---: | ---: | ---: |
| tidy | 3.7 ms | 1.00x | 3.8 ms | 1.00x |
| tidy (1 thread) | 4.9 ms | 1.32x | 5.5 ms | 1.43x |
| bun | 30.4 ms | 8.21x | 30.8 ms | 8.01x |
| oxfmt | 201.3 ms | 54.28x | 230.3 ms | 59.84x |
| prettier | 278.7 ms | 75.15x | 320.7 ms | 83.31x |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, oxfmt 0.75.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
