# Parsers: YAML

392 of 435 files, 1.22 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy parse | tree, comments | 24.90 ms | 49 MB/s | 3.46x | 0 |
| yaml_parser | tree, trivia | 30.99 ms | 39 MB/s | 4.30x | 0 |
| oxc-yaml-parser | tree, comments | 7.21 ms | 169 MB/s | 1.00x | 0 |
| saphyr | value | 22.31 ms | 55 MB/s | 3.10x | 10 |
| yaml-rust2 | value | 22.18 ms | 55 MB/s | 3.08x | 13 |
| serde_yaml | value, libyaml | 30.66 ms | 40 MB/s | 4.25x | 42 |

## Setup

- Date: 2026-11-02
- Commit: e043a18
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, yaml_parser 0.3.0, oxc-yaml-parser 0.0.8, saphyr 0.3.3, yaml-rust2 0.13.0, serde_yaml 0.9.34+deprecated
