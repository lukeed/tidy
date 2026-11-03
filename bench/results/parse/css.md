# Parsers: CSS

299 of 300 files, 0.23 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool           | Builds               |    Time |    Speed | Against fastest | Rejects |
| -------------- | -------------------- | ------: | -------: | --------------: | ------: |
| tidy           | tree, values as text | 1.12 ms | 201 MB/s |           1.00x |       0 |
| raffia         | tree, comments       | 3.85 ms |  59 MB/s |           3.44x |       0 |
| oxc-css-parser | tree, comments       | 3.24 ms |  70 MB/s |           2.90x |       0 |
| lightningcss   | typed rules          | 2.99 ms |  76 MB/s |           2.67x |       1 |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, raffia 0.13.0, oxc-css-parser 0.0.19, lightningcss 1.0.0-alpha.72
