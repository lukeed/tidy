# Parsers: Less

396 of 396 files, 0.98 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy | tree, values as text | 7.68 ms | 127 MB/s | 1.00x | 0 |
| raffia | tree, comments | 15.46 ms | 63 MB/s | 2.01x | 0 |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, raffia 0.13.0
