# Formatters: HTML pages

12 files, 4.6 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 23.9 ms | 1.00x |
| bun | 39.0 ms | 1.63x |
| naive | 55.6 ms | 2.33x |
| tidy (1 thread) | 56.4 ms | 2.36x |
| oxfmt | 849.7 ms | 35.57x |
| prettier | 1482.2 ms | 62.05x |
| dprint | 2135.7 ms | 89.41x |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, a naive formatter on @lukeed/html 1.0.2, oxfmt 0.75.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2
