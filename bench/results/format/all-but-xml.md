# Formatters: all corpora but XML

9460 files, 20.9 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 288.1 ms | 1.00x |
| bun | 433.0 ms | 1.50x |
| tidy (1 thread) | 553.9 ms | 1.92x |
| dprint | 3162.7 ms | 10.98x |
| prettier | 14378.6 ms | 49.91x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2
