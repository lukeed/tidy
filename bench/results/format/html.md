# Formatters: HTML

151 files, 143 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier. With embedded code formatted, only tidy, bun fmt, oxfmt and Prettier run, as the other tools can't format it.

| Tool            | Embedded off | Against tidy | Embedded on | Against tidy |
| --------------- | -----------: | -----------: | ----------: | -----------: |
| tidy            |       6.4 ms |        1.00x |      6.2 ms |        1.00x |
| tidy (1 thread) |      10.9 ms |        1.72x |     12.0 ms |        1.95x |
| dprint          |      14.3 ms |        2.25x |             |              |
| bun             |      34.0 ms |        5.35x |     34.4 ms |        5.58x |
| naive           |      38.6 ms |        6.08x |             |              |
| oxfmt           |     236.7 ms |       37.25x |    367.3 ms |       59.62x |
| prettier        |     326.9 ms |       51.44x |    511.1 ms |       82.96x |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, bun fmt 1.4.4, a naive formatter on @lukeed/html 1.0.2, oxfmt 0.75.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
