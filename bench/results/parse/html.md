# Parsers: HTML

151 of 151 files, 0.15 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy | tree, strict | 0.49 ms | 299 MB/s | 1.73x | 0 |
| scraper | spec DOM | 1.78 ms | 82 MB/s | 6.29x | 0 |
| tl | tree, lenient | 0.28 ms | 516 MB/s | 1.00x | 0 |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, scraper 0.29.0, tl 0.7.8
