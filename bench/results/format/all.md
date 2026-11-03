# Formatters: all corpora

18855 files, 41.6 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier. With embedded code formatted, only tidy, bun fmt, oxfmt and Prettier run, as the other tools can't format it.

| Tool            | Embedded off | Against tidy | Embedded on | Against tidy |
| --------------- | -----------: | -----------: | ----------: | -----------: |
| tidy            |     656.8 ms |        1.00x |    632.5 ms |        1.00x |
| tidy (1 thread) |    1049.0 ms |        1.60x |   1246.7 ms |        1.97x |
| oxfmt           |    1319.3 ms |        2.01x |   2465.6 ms |        3.90x |
| dprint          |    4122.5 ms |        6.28x |             |              |
| prettier        |   24202.1 ms |       36.85x |  37924.4 ms |       59.96x |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxfmt 0.75.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
