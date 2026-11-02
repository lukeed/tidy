# Formatters: TOML

375 files, 411 KB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 8.4 ms | 1.00x |
| taplo | 19.1 ms | 2.28x |
| tidy (1 thread) | 19.9 ms | 2.37x |
| dprint | 21.8 ms | 2.60x |
| bun | 32.3 ms | 3.84x |
| oxfmt | 47.6 ms | 5.67x |
| tombi | 83.0 ms | 9.89x |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, taplo 0.10.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, bun fmt 1.4.4, oxfmt 0.75.0, tombi 1.9.8
