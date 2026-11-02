# Parsers: Handlebars

57 of 57 files, 0.12 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy | HTML and tags | 0.35 ms | 343 MB/s | 1.00x | 0 |
| handlebars | tags only | 12.32 ms | 10 MB/s | 34.92x | 0 |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, handlebars 6.4.4
