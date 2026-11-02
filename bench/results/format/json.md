# Formatters: JSON

4569 files, 4.3 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 90.5 ms | 1.00x |
| oxfmt | 129.9 ms | 1.44x |
| bun | 144.9 ms | 1.60x |
| tidy (1 thread) | 196.6 ms | 2.17x |
| biome | 225.0 ms | 2.49x |
| dprint | 324.0 ms | 3.58x |
| prettier | 3439.2 ms | 38.00x |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxfmt 0.75.0, bun fmt 1.4.4, Biome 2.6.4, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-dockerfile 0.7.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2, prettier-plugin-sh 0.20.2, prettier-plugin-ini 1.3.0
