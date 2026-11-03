# Formatters: SCSS

101 files, 225 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool            |     Time | Against tidy |
| --------------- | -------: | -----------: |
| tidy            |   4.6 ms |        1.00x |
| oxc             |   5.0 ms |        1.09x |
| tidy (1 thread) |   8.3 ms |        1.80x |
| dprint          |  10.9 ms |        2.36x |
| bun             |  31.9 ms |        6.94x |
| oxfmt           |  45.7 ms |        9.92x |
| prettier        | 340.3 ms |       73.91x |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxc_formatter_css 0.75.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, bun fmt 1.4.4, oxfmt 0.75.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
