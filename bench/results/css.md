# Formatters: CSS, SCSS and Less

## frameworks

13 files, 2061 KB. A time is the mean of 10 runs in each order. The last column counts the files that a tool prints exactly as Prettier does.

| Tool                    |      Wall |       CPU |      User | Same as Prettier |
| ----------------------- | --------: | --------: | --------: | ---------------- |
| tidy                    |   17.9 ms |   42.1 ms |   35.0 ms | 10/13 (77%)      |
| tidy (1 thread)         |   36.0 ms |   34.5 ms |   31.9 ms | —                |
| oxc                     |   27.2 ms |   69.7 ms |   51.6 ms | 10/13 (77%)      |
| oxc (1 thread)          |   59.9 ms |   57.4 ms |   47.5 ms | —                |
| malva                   |  374.9 ms |  652.3 ms |  623.7 ms | 2/13 (15%)       |
| malva (1 thread)        |  629.5 ms |  625.9 ms |  601.6 ms | —                |
| lightningcss            |   17.3 ms |   42.6 ms |   34.5 ms | 0/13 (0%)        |
| lightningcss (1 thread) |   36.4 ms |   35.3 ms |   31.2 ms | —                |
| bun                     |   38.1 ms |   62.5 ms |   34.1 ms | 10/13 (77%)      |
| bun (1 thread)          |   55.0 ms |   53.7 ms |   31.1 ms | —                |
| prettier                | 2406.7 ms | 3609.4 ms | 3461.3 ms | —                |

## css

300 files, 241 KB. A time is the mean of 10 runs in each order. The last column counts the files that a tool prints exactly as Prettier does.

| Tool                    |     Wall |      CPU |     User | Same as Prettier |
| ----------------------- | -------: | -------: | -------: | ---------------- |
| tidy                    |   8.5 ms |  38.7 ms |   9.3 ms | 299/300 (100%)   |
| tidy (1 thread)         |  15.1 ms |  14.3 ms |   6.1 ms | —                |
| oxc                     |  11.1 ms |  33.3 ms |  10.0 ms | 298/300 (99%)    |
| oxc (1 thread)          |  18.6 ms |  17.8 ms |   8.2 ms | —                |
| malva                   |  13.0 ms |  41.7 ms |  18.8 ms | 262/300 (87%)    |
| malva (1 thread)        |  25.0 ms |  24.0 ms |  14.2 ms | —                |
| lightningcss            |  10.5 ms |  30.0 ms |   7.6 ms | 112/300 (37%)    |
| lightningcss (1 thread) |  15.8 ms |  14.8 ms |   5.8 ms | —                |
| bun                     |  31.6 ms |  64.5 ms |  12.4 ms | 299/300 (100%)   |
| bun (1 thread)          |  38.1 ms |  37.1 ms |   8.7 ms | —                |
| prettier                | 580.0 ms | 986.7 ms | 920.8 ms | —                |

## scss

102 files, 225 KB. A time is the mean of 10 runs in each order. The last column counts the files that a tool prints exactly as Prettier does.

| Tool             |     Wall |      CPU |     User | Same as Prettier |
| ---------------- | -------: | -------: | -------: | ---------------- |
| tidy             |   4.4 ms |  14.4 ms |   6.3 ms | 102/102 (100%)   |
| tidy (1 thread)  |   8.3 ms |   7.8 ms |   4.8 ms | —                |
| oxc              |   4.5 ms |  11.7 ms |   4.7 ms | 94/102 (92%)     |
| oxc (1 thread)   |   7.2 ms |   6.9 ms |   3.8 ms | —                |
| malva            |   5.8 ms |  16.5 ms |   9.3 ms | 59/102 (58%)     |
| malva (1 thread) |  11.2 ms |  10.6 ms |   7.1 ms | —                |
| bun              |  28.0 ms |  38.4 ms |   9.3 ms | 102/102 (100%)   |
| bun (1 thread)   |  30.4 ms |  29.7 ms |   7.0 ms | —                |
| prettier         | 287.9 ms | 562.6 ms | 526.7 ms | —                |

## less

396 files, 955 KB. A time is the mean of 10 runs in each order. The last column counts the files that a tool prints exactly as Prettier does.

| Tool             |      Wall |       CPU |      User | Same as Prettier |
| ---------------- | --------: | --------: | --------: | ---------------- |
| tidy             |    9.4 ms |   54.4 ms |   28.9 ms | 393/396 (99%)    |
| tidy (1 thread)  |   32.5 ms |   31.2 ms |   21.7 ms | —                |
| oxc              |   11.3 ms |   47.4 ms |   27.5 ms | 381/396 (96%)    |
| oxc (1 thread)   |   34.6 ms |   33.7 ms |   23.0 ms | —                |
| malva            |   15.4 ms |   81.2 ms |   62.8 ms | 301/396 (76%)    |
| malva (1 thread) |   58.0 ms |   56.7 ms |   45.0 ms | —                |
| bun              |   33.0 ms |   79.8 ms |   29.3 ms | 393/396 (99%)    |
| bun (1 thread)   |   53.9 ms |   52.7 ms |   23.0 ms | —                |
| prettier         | 1374.9 ms | 2211.2 ms | 2116.2 ms | —                |

## Setup

- Date: 2026-11-01
- Commit: 65190ed
- Machine: Apple M1 Max, 10 cores, macOS 15.7.9
- Tools: tidy at the commit above, oxc_formatter_css 0.75.0, malva 0.16.0, lightningcss 1.0.0-alpha.72, bun fmt 1.4.4, Prettier 3.9.9
