# Formatters: YAML

435 files, 1.2 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool            |      Time | Against tidy |
| --------------- | --------: | -----------: |
| tidy            |   24.3 ms |        1.00x |
| bun             |   34.9 ms |        1.44x |
| oxfmt           |   45.8 ms |        1.88x |
| tidy (1 thread) |   51.0 ms |        2.10x |
| dprint          |  103.5 ms |        4.26x |
| yamlfmt         |  113.3 ms |        4.66x |
| prettier        | 1271.2 ms |       52.33x |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, oxfmt 0.75.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, yamlfmt 0.21.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
