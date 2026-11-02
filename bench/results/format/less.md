# Formatters: Less

396 files, 0.9 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 9.0 ms | 1.00x |
| oxc | 11.8 ms | 1.32x |
| tidy (1 thread) | 32.1 ms | 3.58x |
| bun | 33.0 ms | 3.68x |
| dprint | 58.7 ms | 6.55x |
| prettier | 1424.0 ms | 158.88x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxc_formatter_css 0.75.0, bun fmt 1.4.4, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2
