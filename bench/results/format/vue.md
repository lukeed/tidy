# Formatters: Vue

230 files, 1.2 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 6.5 ms | 1.00x |
| tidy (1 thread) | 22.5 ms | 3.44x |
| bun | 30.7 ms | 4.69x |
| naive | 35.5 ms | 5.43x |
| dprint | 206.6 ms | 31.59x |
| prettier | 614.9 ms | 94.04x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, a naive formatter on @lukeed/html 1.0.2, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2
