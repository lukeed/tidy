# Formatters: HTML pages

12 files, 4.6 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 23.1 ms | 1.00x |
| bun | 40.2 ms | 1.74x |
| naive | 55.9 ms | 2.43x |
| tidy (1 thread) | 57.4 ms | 2.49x |
| prettier | 1503.1 ms | 65.20x |
| dprint | 2153.9 ms | 93.43x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, a naive formatter on @lukeed/html 1.0.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2
