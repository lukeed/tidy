# Formatters: XML

6784 files, 15.8 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool            |      Time | Against tidy |
| --------------- | --------: | -----------: |
| tidy            |  129.7 ms |        1.00x |
| tidy (1 thread) |  344.2 ms |        2.65x |
| dprint          |  615.5 ms |        4.75x |
| prettier        | 6017.2 ms |       46.40x |

## Setup

- Date: 2026-11-03
- Commit: 86ff793
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
