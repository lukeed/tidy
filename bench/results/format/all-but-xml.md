# Formatters: all corpora but XML

12071 files, 25.8 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier. With embedded code formatted, only tidy, bun fmt, oxfmt and Prettier run, as the other tools can't format it.

| Tool            | Embedded off | Against tidy | Embedded on | Against tidy |
| --------------- | -----------: | -----------: | ----------: | -----------: |
| tidy            |     280.4 ms |        1.00x |    285.7 ms |        1.00x |
| bun             |     569.0 ms |        2.03x |    609.3 ms |        2.13x |
| tidy (1 thread) |     661.3 ms |        2.36x |    819.5 ms |        2.87x |
| oxfmt           |    1231.5 ms |        4.39x |   2385.1 ms |        8.35x |
| dprint          |    3474.3 ms |       12.39x |             |              |
| prettier        |   18548.4 ms |       66.15x |  30902.0 ms |      108.18x |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, oxfmt 0.75.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
