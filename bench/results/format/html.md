# Formatters: HTML

151 files, 0.1 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 5.4 ms | 1.00x |
| tidy (1 thread) | 10.2 ms | 1.90x |
| dprint | 13.0 ms | 2.43x |
| bun | 30.3 ms | 5.66x |
| naive | 36.0 ms | 6.71x |
| prettier | 291.1 ms | 54.30x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2, bun fmt 1.4.4, a naive formatter on @lukeed/html 1.0.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2
