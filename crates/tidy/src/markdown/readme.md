<h1 align="center">tidy: Markdown</h1>

<div align="center">A fast Markdown formatter that streams, with Prettier's style by default</div>

<br />

Most formatters parse your Markdown into a tree, turn that tree into a layout document, and then print the document. tidy skips the middle. It walks [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark)'s events once and writes the output as it goes.

It fixes how blocks are laid out &mdash; list markers, indentation, blank lines, headings, fences, tables &mdash; and copies everything inside a line exactly as you wrote it. So your escapes, entities, links, and HTML never move. That's also why it's fast: there's very little left to decide.

- 1.75x faster than `bun fmt`, which is already _very_ fast
- Matches Prettier's output on 99%+ of real-world files, out of the box
- Keeps what a document means: the tests re-parse every output and compare it to the input
- Presets plus per-option overrides, so you can keep your own markers
- Supports `prettier-ignore` style directives, under any name you like
- Hands code blocks & front matter to you, so your other formatters can take a pass
- Rust library, CLI, and Node binding

## Install

```sh
$ cargo add tidy         # library
$ cargo install tidy # CLI
$ bun add tidyrs         # Node binding
```

## Usage

### CLI

```sh
$ tidy readme.md                     # print the result
$ tidy check docs                    # list files that aren't formatted
$ tidy up .                          # rewrite in place
$ cat notes.md | tidy --parser markdown
```

Run `tidy --help` for every flag. Markdown's own options go in the `markdown` section of a `tidy.json`.

### Rust

```rust
use tidy::markdown::{Options, Preset, Wrap};

let output = tidy::markdown::format("* one\n* two\n", &Options::default());
assert_eq!(output, "- one\n- two\n");

// Start from a preset, then change what you want
let options = Options { wrap: Wrap::Always, ..Options::from(Preset::Preserve) };

// Format code blocks with something else
let output = tidy::markdown::format_with(source, &options, |code| match code.lang {
  "json" => Some(pretty_json(code.text)),
  _ => None, // keep as written
});
```

### Node

```js
import { check, format } from "tidyrs";

let output = format("* one\n* two\n", { parser: "markdown" }); //=> "- one\n- two\n"
let options = {
  parser: "markdown",
  markdown: { preset: "preserve", wrap: "always" },
};
let pretty = format(input, options, (code) => {
  if (code.language === "json")
    return JSON.stringify(JSON.parse(code.code), null, 2);
});

if (!check(input, { parser: "markdown" })) process.exitCode = 1;
```

## Options

Every option starts from a preset. `prettier` is the default; `preserve` keeps every marker as written and only fixes the layout.

| Option      | Values                                   | `prettier`        | `preserve`    |
| ----------- | ---------------------------------------- | ----------------- | ------------- |
| `width`     | any number                               | `80`              | `80`          |
| `wrap`      | `preserve`, `always`, `never`            | `preserve`        | `preserve`    |
| `bullet`    | `keep`, `dash`, `asterisk`, `plus`       | `dash`            | `keep`        |
| `emphasis`  | `keep`, `underscore`, `asterisk`         | `underscore`      | `keep`        |
| `strong`    | `keep`, `asterisk`, `underscore`         | `asterisk`        | `keep`        |
| `numbering` | `keep`, `auto`                           | `auto`            | `keep`        |
| `rule`      | `keep`, `dash`, `asterisk`, `underscore` | `dash`            | `keep`        |
| `fence`     | `keep`, `backtick`, `tilde`              | `backtick`        | `keep`        |
| `heading`   | `keep`, `atx`                            | `keep`            | `keep`        |
| `ignore`    | any name                                 | `prettier-ignore` | `tidy-ignore` |

A few of these bend on purpose, the same way Prettier does:

- Adjacent sibling lists alternate their marker (`-` then `*`), or else they'd merge into one list
- `_` emphasis falls back to `*` inside a word, where `_` doesn't work
- `auto` numbering counts up from the first number &mdash; unless your second item repeats it (`1.`, `1.`, `1.`), in which case it's kept

## Ignore directives

Put an HTML comment on its own line. With the default name:

```md
<!-- prettier-ignore -->
| keep   | this table | as is |
|--|--|--|

<!-- prettier-ignore-start -->
Everything in here is left alone.
<!-- prettier-ignore-end -->
```

A range only counts at the top level, not inside a quote or list, as in Prettier. And `<!-- prettier-ignore-file -->` at the top of a file (after any front matter) skips the whole file.

## Benchmarks

Check mode with all cores, code blocks left alone. The mean of both run orders, since whichever runs first is slower. Run your own with `mise run bench -- --only markdown`.

```
                  vite, storybook, shadcn-svelte & bun repos
                  (2,834 files, 7.6 MB)
tidy              39 ms
bun fmt           68 ms
dprint            200 ms
prettier          6,527 ms
```

> Measured on an M1 Max with Bun 1.4.4, whose `bun fmt` matches Prettier byte for byte.

## License

MIT © [Luke Edwards](https://lukeed.com)
