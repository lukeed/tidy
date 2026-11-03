# Formatters: HTML pages

12 files, 4.6 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier. With embedded code formatted, only tidy, bun fmt, oxfmt and Prettier run, as the other tools can't format it.

| Tool | Embedded off | Against tidy | Embedded on | Against tidy |
| --- | ---: | ---: | ---: | ---: |
| tidy | 15.0 ms | 1.00x | 24.7 ms | 1.00x |
| tidy (1 thread) | 38.6 ms | 2.58x | 60.8 ms | 2.46x |
| bun | 42.6 ms | 2.85x | 48.8 ms | 1.97x |
| naive | 56.2 ms | 3.76x |  |  |
| oxfmt | 854.2 ms | 57.10x | 1605.6 ms | 64.89x |
| prettier | 1495.3 ms | 99.96x | 2961.4 ms | 119.68x |
| dprint | 2152.1 ms | 143.86x |  |  |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, a naive formatter on @lukeed/html 1.0.2, oxfmt 0.75.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2
