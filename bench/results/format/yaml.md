# Formatters: YAML

435 files, 1.2 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 23.4 ms | 1.00x |
| bun | 31.7 ms | 1.36x |
| tidy (1 thread) | 51.5 ms | 2.20x |
| dprint | 103.5 ms | 4.43x |
| yamlfmt | 110.7 ms | 4.74x |
| prettier | 1196.4 ms | 51.16x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2, yamlfmt 0.21.0, Prettier 3.9.9, @prettier/plugin-xml 3.4.2
