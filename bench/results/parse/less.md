# Parsers: Less

396 of 396 files, 0.98 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool           | Builds               |     Time |    Speed | Against fastest | Rejects |
| -------------- | -------------------- | -------: | -------: | --------------: | ------: |
| tidy           | tree, values as text |  7.52 ms | 130 MB/s |           1.00x |       0 |
| raffia         | tree, comments       | 15.22 ms |  64 MB/s |           2.02x |       0 |
| oxc-css-parser | tree, comments       | 11.96 ms |  82 MB/s |           1.59x |       0 |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, raffia 0.13.0, oxc-css-parser 0.0.19
