# Parsers: HTML

151 of 151 files, 0.15 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool               | Builds        |    Time |    Speed | Against fastest | Rejects |
| ------------------ | ------------- | ------: | -------: | --------------: | ------: |
| tidy               | tree, strict  | 0.57 ms | 255 MB/s |           2.34x |       0 |
| scraper            | spec DOM      | 1.75 ms |  83 MB/s |           7.16x |       0 |
| tl                 | tree, lenient | 0.32 ms | 456 MB/s |           1.31x |       0 |
| oxc-htmlish-parser | tree, lenient | 0.24 ms | 597 MB/s |           1.00x |       0 |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, scraper 0.29.0, tl 0.7.8, oxc-htmlish-parser 0.0.2
