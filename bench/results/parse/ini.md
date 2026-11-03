# Parsers: INI

74 of 75 files, 0.04 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool         | Builds               |    Time |    Speed | Against fastest | Rejects |
| ------------ | -------------------- | ------: | -------: | --------------: | ------: |
| tidy         | lines, comments      | 0.14 ms | 277 MB/s |           1.00x |       0 |
| rust-ini     | map                  | 0.46 ms |  83 MB/s |           3.32x |       1 |
| configparser | map, lowercase names | 0.45 ms |  85 MB/s |           3.28x |       0 |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, rust-ini 0.21.3, configparser 3.3.0
