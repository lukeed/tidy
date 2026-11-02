# Parsers: JSON

4383 of 4569 files, 3.25 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy parse | tree, comments | 6.72 ms | 483 MB/s | 1.55x | 0 |
| tidy value | serde_json value | 13.39 ms | 243 MB/s | 3.09x | 1 |
| jsonc-parser ast | tree, comments | 11.66 ms | 279 MB/s | 2.69x | 8 |
| jsonc-parser value | own value | 12.16 ms | 267 MB/s | 2.81x | 8 |
| jsonc-parser serde | serde_json value | 15.34 ms | 212 MB/s | 3.54x | 9 |
| serde_json | value, strict | 9.49 ms | 342 MB/s | 2.19x | 185 |
| sonic-rs | value, strict | 4.33 ms | 750 MB/s | 1.00x | 185 |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, jsonc-parser 0.34.0, serde_json 1.0.154, sonic-rs 0.5.10
