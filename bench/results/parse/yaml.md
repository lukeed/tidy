# Parsers: YAML

392 of 435 files, 1.22 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy parse | tree, comments | 24.94 ms | 49 MB/s | 1.12x | 0 |
| yaml_parser | tree, trivia | 31.58 ms | 39 MB/s | 1.41x | 0 |
| saphyr | value | 22.45 ms | 54 MB/s | 1.01x | 10 |
| yaml-rust2 | value | 22.32 ms | 55 MB/s | 1.00x | 13 |
| serde_yaml | value, libyaml | 30.81 ms | 40 MB/s | 1.38x | 42 |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, yaml_parser 0.3.0, saphyr 0.3.3, yaml-rust2 0.13.0, serde_yaml 0.9.34+deprecated
