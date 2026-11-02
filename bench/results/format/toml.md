# Formatters: TOML

375 files, 0.4 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 8.4 ms | 1.00x |
| taplo | 18.4 ms | 2.20x |
| tidy (1 thread) | 20.5 ms | 2.45x |
| dprint | 23.3 ms | 2.79x |
| bun | 42.2 ms | 5.05x |
| tombi | 81.4 ms | 9.73x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, taplo 0.10.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2, bun fmt 1.4.4, tombi 1.9.8
