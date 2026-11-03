# Parsers: YAML

392 of 435 files, 1.22 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool            | Builds         |     Time |    Speed | Against fastest | Rejects |
| --------------- | -------------- | -------: | -------: | --------------: | ------: |
| tidy parse      | tree, comments | 24.85 ms |  49 MB/s |           3.54x |       0 |
| yaml_parser     | tree, trivia   | 30.66 ms |  40 MB/s |           4.37x |       0 |
| oxc-yaml-parser | tree, comments |  7.02 ms | 174 MB/s |           1.00x |       0 |
| saphyr          | value          | 22.08 ms |  55 MB/s |           3.14x |      10 |
| yaml-rust2      | value          | 21.94 ms |  56 MB/s |           3.12x |      13 |
| serde_yaml      | value, libyaml | 30.33 ms |  40 MB/s |           4.32x |      42 |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, yaml_parser 0.3.0, oxc-yaml-parser 0.0.8, saphyr 0.3.3, yaml-rust2 0.13.0, serde_yaml 0.9.34+deprecated
