# Parsers: XML

6752 of 6784 files, 16.56 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy | tree, comments | 50.17 ms | 330 MB/s | 1.78x | 0 |
| roxmltree | tree | 61.46 ms | 269 MB/s | 2.18x | 16 |
| quick-xml | events | 28.17 ms | 588 MB/s | 1.00x | 0 |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, roxmltree 0.21.2, quick-xml 0.42.0
