# Parsers: JSON

4383 of 4569 files, 3.25 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool | Builds | Time | Speed | Against fastest | Rejects |
| --- | --- | ---: | ---: | ---: | ---: |
| tidy parse | tree, comments | 6.70 ms | 485 MB/s | 1.56x | 0 |
| tidy value | serde_json value | 13.16 ms | 247 MB/s | 3.06x | 1 |
| jsonc-parser ast | tree, comments | 11.44 ms | 284 MB/s | 2.66x | 8 |
| oxc_parser | JS tree, comments | 13.65 ms | 238 MB/s | 3.17x | 12 |
| jsonc-parser value | own value | 11.92 ms | 272 MB/s | 2.77x | 8 |
| jsonc-parser serde | serde_json value | 14.88 ms | 218 MB/s | 3.46x | 9 |
| serde_json | value, strict | 9.10 ms | 357 MB/s | 2.12x | 185 |
| sonic-rs | value, strict | 4.30 ms | 755 MB/s | 1.00x | 185 |

## Setup

- Date: 2026-11-02
- Commit: e043a18
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, jsonc-parser 0.34.0, oxc_parser 0.156.0, serde_json 1.0.154, sonic-rs 0.5.10
