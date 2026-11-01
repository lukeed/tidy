<h1 align="center">mdtidy</h1>

<div align="center">A fast Markdown formatter that streams, with Prettier's style by default</div>

<br />

Most formatters parse your Markdown into a tree, turn that tree into a layout document, and then print the document. mdtidy skips the middle. It walks [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark)'s events once and writes the output as it goes.

It fixes how blocks are laid out &mdash; list markers, indentation, blank lines, headings, fences, tables &mdash; and copies everything inside a line exactly as you wrote it. So your escapes, entities, links, and HTML never move. That's also why it's fast: there's very little left to decide.

* 1.6x faster than `bun fmt`, which is already _very_ fast
* Matches Prettier's output on 94-99% of real-world files, out of the box
* Keeps what a document means: the tests re-parse every output and compare it to the input
* Presets plus per-option overrides, so you can keep your own markers
* Supports `prettier-ignore` style directives, under any name you like
* Hands code blocks & front matter to you, so your other formatters can take a pass
* Rust library, CLI, and Node binding


## Install

```sh
$ cargo add mdtidy         # library
$ cargo install mdtidy_cli # CLI
$ bun add mdtidy           # Node binding
```


## Usage

### CLI

```sh
$ mdtidy readme.md                  # print the result
$ mdtidy docs --check               # list files that aren't formatted
$ mdtidy . --write                  # rewrite in place
$ cat notes.md | mdtidy --wrap never
```

Run `mdtidy --help` for every flag.

### Rust

```rust
use mdtidy::{Options, Preset, Wrap};

let output = mdtidy::format("* one\n* two\n", &Options::default());
assert_eq!(output, "- one\n- two\n");

// Start from a preset, then change what you want
let options = Options { wrap: Wrap::Always, ..Options::from(Preset::Preserve) };

// Format code blocks with something else
let output = mdtidy::format_with(source, &options, |code| match code.lang {
  "json" => Some(pretty_json(code.text)),
  _ => None, // keep as written
});
```

### Node

```js
import { check, format } from "mdtidy";

let output = format("* one\n* two\n"); //=> "- one\n- two\n"
let pretty = format(input, { preset: "preserve", wrap: "always" }, (lang, text) => {
  if (lang === "json") return JSON.stringify(JSON.parse(text), null, 2);
});

if (!check(input)) process.exitCode = 1;
```


## Options

Every option starts from a preset. `prettier` is the default; `preserve` keeps every marker as written and only fixes the layout.

| Option      | Values                                     | `prettier`        | `preserve`      |
| ----------- | ------------------------------------------ | ----------------- | --------------- |
| `width`     | any number                                 | `80`              | `80`            |
| `wrap`      | `preserve`, `always`, `never`              | `preserve`        | `preserve`      |
| `bullet`    | `keep`, `dash`, `asterisk`, `plus`         | `dash`            | `keep`          |
| `emphasis`  | `keep`, `underscore`, `asterisk`           | `underscore`      | `keep`          |
| `strong`    | `keep`, `asterisk`, `underscore`           | `asterisk`        | `keep`          |
| `numbering` | `keep`, `auto`                             | `auto`            | `keep`          |
| `rule`      | `keep`, `dash`, `asterisk`, `underscore`   | `dash`            | `keep`          |
| `fence`     | `keep`, `backtick`, `tilde`                | `backtick`        | `keep`          |
| `heading`   | `keep`, `atx`                              | `keep`            | `keep`          |
| `ignore`    | any name                                   | `prettier-ignore` | `mdtidy-ignore` |

A few of these bend on purpose, the same way Prettier does:

* Adjacent sibling lists alternate their marker (`-` then `*`), or else they'd merge into one list
* `_` emphasis falls back to `*` inside a word, where `_` doesn't work
* `auto` numbering counts up from the first number &mdash; unless your second item repeats it (`1.`, `1.`, `1.`), in which case it's kept


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

And `<!-- prettier-ignore-file -->` at the top of a file (after any front matter) skips the whole file.


## Benchmarks

Check mode with all cores, code blocks left alone. The mean of both run orders, since whichever runs first is slower. Run your own with `mise run bench -- <dir>`.

```
                  vite, storybook, shadcn-svelte    starship & bun repos
                  (865 files, 4.2 MB)               (2,147 files, 5.7 MB)
mdtidy            15 ms                             31 ms
bun fmt           25 ms                             47-57 ms
```

> Measured on an M1 Max with Bun 1.4.4, whose `bun fmt` matches Prettier byte for byte.


## License

MIT © [Luke Edwards](https://lukeed.com)
