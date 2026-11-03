# Formatters: Less

396 files, 955 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool            |      Time | Against tidy |
| --------------- | --------: | -----------: |
| tidy            |    9.7 ms |        1.00x |
| oxc             |   12.3 ms |        1.28x |
| tidy (1 thread) |   32.6 ms |        3.37x |
| bun             |   36.8 ms |        3.81x |
| oxfmt           |   48.0 ms |        4.97x |
| dprint          |   58.3 ms |        6.04x |
| prettier        | 1490.7 ms |      154.35x |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxc_formatter_css 0.75.0, bun fmt 1.4.4, oxfmt 0.75.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
