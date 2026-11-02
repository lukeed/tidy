# Parsers: XML

6752 of 6784 files, 16.56 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy | tree, comments | 34.12 ms | 485 MB/s | 1.24x | 0 |
| roxmltree | tree | 61.45 ms | 269 MB/s | 2.24x | 16 |
| quick-xml | events | 27.46 ms | 603 MB/s | 1.00x | 0 |

## Setup

- Date: 2026-11-02
- Commit: e043a18
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, roxmltree 0.21.2, quick-xml 0.42.0
