# Parsers: GraphQL

2156 of 2511 files, 4.92 MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of 20 passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.

| Tool                 | Builds          |      Time |    Speed | Against fastest | Rejects |
| -------------------- | --------------- | --------: | -------: | --------------: | ------: |
| tidy                 | tree, comments  |   9.53 ms | 517 MB/s |           1.00x |       0 |
| oxc-graphql-parser   | tree, comments  |   9.99 ms | 493 MB/s |           1.05x |       0 |
| apollo-parser        | tree, trivia    |  67.52 ms |  73 MB/s |           7.09x |       0 |
| cynic-parser         | query or schema |  36.97 ms | 133 MB/s |           3.88x |       0 |
| graphql-parser       | query or schema |  45.24 ms | 109 MB/s |           4.75x |       0 |
| async-graphql-parser | query or schema | 164.43 ms |  30 MB/s |          17.26x |     355 |

## Setup

- Date: 2026-11-02
- Commit: b843db5
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxc-graphql-parser 0.0.9, apollo-parser 0.8.7, cynic-parser 0.11.2, graphql-parser 0.4.1, async-graphql-parser 7.2.1
