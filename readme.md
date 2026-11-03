# tidy

> Prettier's style, at native speed.

tidy is a code formatter written in Rust. It's a single binary that prints what Prettier prints for every language below. It does not delegate to Node, but there's a Node binding if you want one.

It's _fast_.<br>On a 12,000-file corpus of every language (except XML), tidy checks everything in 280ms. bun fmt takes 569ms. oxfmt 1.2s. Prettier 18.5s.

- **Matches Prettier**<br>100% of 3,906 JavaScript files and 3,308 of 3,309 TypeScript files in the bench corpus (with JSDoc formatting off)
- **Formats the code inside your code**<br>styles and scripts in HTML and Vue, Markdown code blocks and front matter, CSS, GraphQL, HTML and Markdown in JS templates
- **Configurable, if needed**<br>Reads `tidy.json`, `tidy.jsonc` or `tidy.toml`, plus `.editorconfig`
- **Stable JS and TS output**<br>where Prettier's output would change on its next run, tidy prints where it settles, so `--check` stays green in CI

## Languages

| Language                                     | Style                    |
| -------------------------------------------- | ------------------------ |
| JavaScript, TypeScript, JSX                  | Prettier                 |
| HTML, Vue, Handlebars, MJML                  | Prettier                 |
| CSS, SCSS, Less                              | Prettier                 |
| JSON, JSONC, JSON5                           | Prettier                 |
| YAML, GraphQL, Markdown                      | Prettier                 |
| XML, SVG                                     | `@prettier/plugin-xml`   |
| TOML                                         | taplo                    |
| Dockerfiles                                  | dprint-plugin-dockerfile |
| INI, `.editorconfig`, `.npmrc`, `.gitconfig` | `prettier-plugin-ini`    |

## Install

Nothing is published yet. For now, build it from a clone of this repo:

```sh
$ cargo install --path crates/tidy_cli
```

## Usage

```sh
# print one file, formatted
$ tidy index.html

# list the files that aren't formatted; exits 1 if any
$ tidy src --check

# rewrite everything in place
$ tidy . --write
```

Without paths, tidy reads stdin. Run `tidy --help` for every flag.

### Config

Each file uses the nearest `tidy.json`, `tidy.jsonc` or `tidy.toml`, on top of `.editorconfig`. Shared keys apply everywhere, and a language section overrides them:

```json
{
  "width": 100,
  "js": { "semi": false, "quote": "single" },
  "markdown": { "wrap": "always" }
}
```

### Node

```js
import { format, check } from "tidyrs";

let output = format("<p   >hi</p>", { filepath: "index.html" });
//=> "<p>hi</p>\n"

check(output, { filepath: "index.html" });
//=> true
```

### Rust

```rust
use tidy::{Language, Options, embed::Verbatim};

let language = Language::of_path("App.vue").unwrap();
let output = tidy::format(source, language, &Options::default(), Some("App.vue"), &mut Verbatim)?;
```

## Benchmarks

Every tool checks every file of each corpus, using all cores, on an M1 Max. Times are the mean of both run orders. **Embedded code is excluded** so that every tool does the same amount of work.

| Corpus          |  Files |       tidy | bun fmt |    oxfmt |   dprint |  Prettier |
| --------------- | -----: | ---------: | ------: | -------: | -------: | --------: |
| JavaScript      |  3,906 |  **78 ms** |  100 ms |   123 ms |          |  8,528 ms |
| TypeScript      |  3,309 |  **58 ms** |   85 ms |    96 ms |          |  7,523 ms |
| JSON            |  4,569 |  **86 ms** |  149 ms |   114 ms |   323 ms |  3,528 ms |
| CSS             |    300 | **7.2 ms** |   35 ms |    47 ms |    25 ms |    654 ms |
| SCSS            |    101 | **4.6 ms** |   32 ms |    46 ms |    11 ms |    340 ms |
| Less            |    396 | **9.7 ms** |   37 ms |    48 ms |    58 ms |  1,491 ms |
| HTML            |    151 | **6.4 ms** |   34 ms |   237 ms |    14 ms |    327 ms |
| Vue             |    230 | **7.3 ms** |   34 ms |   435 ms |   205 ms |    684 ms |
| Handlebars      |     57 | **3.7 ms** |   30 ms |   201 ms |          |    279 ms |
| Markdown        |  2,834 |  **37 ms** |   71 ms |    80 ms |   188 ms |  6,618 ms |
| YAML            |    435 |  **24 ms** |   35 ms |    46 ms |   104 ms |  1,271 ms |
| TOML            |    375 | **8.6 ms** |   36 ms |    49 ms |    22 ms |           |
| GraphQL         |  2,511 |  **37 ms** |   63 ms |    72 ms |   271 ms |  1,988 ms |
| XML             |  6,784 | **130 ms** |         |          |   616 ms |  6,017 ms |
| Dockerfiles     |     25 | **3.6 ms** |         |          |   3.7 ms |    458 ms |
| INI             |     75 | **3.7 ms** |         |          |          |    147 ms |
| **All but XML** | 12,071 | **280 ms** |  569 ms | 1,232 ms | 3,474 ms | 18,548 ms |

With embedded code formatted too, the gap grows: on all but XML, tidy takes 286 ms, bun fmt 609 ms, oxfmt 2.4 s, and Prettier 30.9 s.

The full tables, with one-thread times, Biome, taplo, yamlfmt, lightningcss and oxc's CSS formatter, are in [`bench/results/format`](bench/results/format). The scripts are [`bench/run.ts`](bench/run.ts) and [`bench/js.ts`](bench/js.ts).

## Differences from Prettier

tidy prints something else on purpose in a few places: where Prettier's output changes what your code means, where it would change again on the next run, and a few choices, like formatting JSDoc comments by default, as `prettier-plugin-jsdoc` does. Turn that off with `"js": { "jsdoc": false }`. Each case is listed, with an example, in [`docs/deviations.md`](docs/deviations.md).

## Credits

tidy ports a lot of great work: Prettier, Biome, oxc, Bun's Rust ports, taplo, dprint, and more. See [`docs/credits.md`](docs/credits.md) for all of them.

## License

MIT © [Luke Edwards](https://lukeed.com)
