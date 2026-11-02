# Formatters: SCSS

101 files, 0.2 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 4.4 ms | 1.00x |
| oxc | 5.1 ms | 1.16x |
| tidy (1 thread) | 8.5 ms | 1.94x |
| dprint | 11.4 ms | 2.61x |
| bun | 30.7 ms | 6.99x |
| prettier | 309.4 ms | 70.53x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxc_formatter_css 0.75.0, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2, bun fmt 1.4.4, Prettier 3.9.9, @prettier/plugin-xml 3.4.2
