# Parsers: TOML

358 of 375 files, 0.42 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy parse | tree, trivia | 2.19 ms | 191 MB/s | 1.00x | 0 |
| tidy value | serde_json value | 4.80 ms | 87 MB/s | 2.19x | 17 |
| taplo | tree, trivia | 4.93 ms | 85 MB/s | 2.26x | 0 |
| toml_edit | document, trivia | 6.40 ms | 65 MB/s | 2.92x | 17 |
| toml | value | 4.85 ms | 86 MB/s | 2.22x | 17 |
| oxc-toml | tree, trivia | 3.64 ms | 115 MB/s | 1.66x | 0 |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, taplo 0.14.0, toml_edit 0.25.17+spec-1.1.0, toml 1.1.8+spec-1.1.0, oxc-toml 0.15.1
