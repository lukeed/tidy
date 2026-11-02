# Parsers: SCSS

101 of 101 files, 0.23 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy | tree, values as text | 0.65 ms | 356 MB/s | 1.00x | 0 |
| raffia | tree, comments | 1.69 ms | 136 MB/s | 2.62x | 0 |
| oxc-css-parser | tree, comments | 1.30 ms | 177 MB/s | 2.01x | 0 |

## Setup

- Date: 2026-11-02
- Commit: e043a18
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, raffia 0.13.0, oxc-css-parser 0.0.19
