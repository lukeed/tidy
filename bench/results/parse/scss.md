# Parsers: SCSS

101 of 101 files, 0.23 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy | tree, values as text | 0.61 ms | 380 MB/s | 1.00x | 0 |
| raffia | tree, comments | 1.68 ms | 137 MB/s | 2.78x | 0 |
| oxc-css-parser | tree, comments | 1.19 ms | 193 MB/s | 1.97x | 0 |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, raffia 0.13.0, oxc-css-parser 0.0.19
