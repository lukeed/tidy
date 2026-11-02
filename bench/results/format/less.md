# Formatters: Less

396 files, 955 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 9.0 ms | 1.00x |
| oxc | 11.8 ms | 1.30x |
| tidy (1 thread) | 31.8 ms | 3.53x |
| bun | 32.8 ms | 3.64x |
| oxfmt | 47.1 ms | 5.23x |
| dprint | 57.8 ms | 6.41x |
| prettier | 1474.7 ms | 163.65x |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxc_formatter_css 0.75.0, bun fmt 1.4.4, oxfmt 0.75.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
