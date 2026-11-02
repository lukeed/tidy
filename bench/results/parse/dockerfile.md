# Parsers: Dockerfiles

22 of 25 files, 0.03 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy | tree, comments | 0.14 ms | 224 MB/s | 3.42x | 0 |
| dprint-plugin-dockerfile | tree | 0.14 ms | 218 MB/s | 3.52x | 0 |
| dockerfile-parser | tree | 1.01 ms | 31 MB/s | 24.80x | 3 |
| parse-dockerfile | tree | 0.04 ms | 768 MB/s | 1.00x | 0 |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, dprint-plugin-dockerfile 0.7.0, dockerfile-parser 0.9.0, parse-dockerfile 0.1.9
