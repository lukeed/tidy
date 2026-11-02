# Formatters: all corpora

16244 files, 36.7 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 508.6 ms | 1.00x |
| tidy (1 thread) | 964.8 ms | 1.90x |
| dprint | 3848.6 ms | 7.57x |
| prettier | 19407.9 ms | 38.16x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2
