# Formatters: HTML pages

12 files, 4.6 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 23.0 ms | 1.00x |
| bun | 38.4 ms | 1.67x |
| naive | 53.1 ms | 2.31x |
| tidy (1 thread) | 56.6 ms | 2.45x |
| oxfmt | 842.5 ms | 36.55x |
| prettier | 1454.9 ms | 63.12x |
| dprint | 2132.3 ms | 92.51x |

## Setup

- Date: 2026-11-02
- Commit: b74b961
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, a naive formatter on @lukeed/html 1.0.2, oxfmt 0.75.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2
