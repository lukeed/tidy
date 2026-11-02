# Parsers: TOML

358 of 375 files, 0.42 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy parse | tree, trivia | 2.15 ms | 195 MB/s | 1.00x | 0 |
| tidy value | serde_json value | 4.90 ms | 85 MB/s | 2.28x | 17 |
| taplo | tree, trivia | 5.12 ms | 82 MB/s | 2.38x | 0 |
| toml_edit | document, trivia | 6.50 ms | 64 MB/s | 3.03x | 17 |
| toml | value | 4.94 ms | 85 MB/s | 2.30x | 17 |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, taplo 0.14.0, toml_edit 0.25.17+spec-1.1.0, toml 1.1.8+spec-1.1.0
