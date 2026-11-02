# Formatters: SCSS

101 files, 225 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 4.2 ms | 1.00x |
| oxc | 4.9 ms | 1.17x |
| tidy (1 thread) | 8.0 ms | 1.90x |
| dprint | 10.6 ms | 2.51x |
| bun | 27.7 ms | 6.59x |
| oxfmt | 43.5 ms | 10.36x |
| prettier | 328.2 ms | 78.08x |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxc_formatter_css 0.75.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, bun fmt 1.4.4, oxfmt 0.75.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
