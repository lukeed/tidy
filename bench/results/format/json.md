# Formatters: JSON

4569 files, 4.3 MB. Each tool checks every file. A time is the mean of 10 runs in each order, or 3 for Prettier.

| Tool | Time | Against tidy |
| --- | ---: | ---: |
| tidy | 86.9 ms | 1.00x |
| bun | 145.7 ms | 1.68x |
| tidy (1 thread) | 193.8 ms | 2.23x |
| biome | 229.2 ms | 2.64x |
| dprint | 326.5 ms | 3.76x |
| prettier | 2862.1 ms | 32.94x |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, bun fmt 1.4.4, Biome 2.6.4, dprint plugins: markup_fmt 0.25.0, malva 0.16.0, dprint-plugin-json 0.25.2, pretty_yaml 0.6.0, dprint-plugin-toml 0.9.0, dprint-plugin-markdown 0.28.2, Prettier 3.9.9, @prettier/plugin-xml 3.4.2
