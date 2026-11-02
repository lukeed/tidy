# Parsers: TOML

358 of 375 files, 0.42 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy parse | tree, trivia | 2.24 ms | 187 MB/s | 1.00x | 0 |
| tidy value | serde_json value | 4.90 ms | 85 MB/s | 2.19x | 17 |
| taplo | tree, trivia | 5.00 ms | 84 MB/s | 2.23x | 0 |
| toml_edit | document, trivia | 6.45 ms | 65 MB/s | 2.88x | 17 |
| toml | value | 4.90 ms | 85 MB/s | 2.18x | 17 |
| oxc-toml | tree, trivia | 3.70 ms | 113 MB/s | 1.65x | 0 |

## Setup

- Date: 2026-11-02
- Commit: e043a18
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, taplo 0.14.0, toml_edit 0.25.17+spec-1.1.0, toml 1.1.8+spec-1.1.0, oxc-toml 0.15.1
