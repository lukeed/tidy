---
title: CLI
summary: Every flag of the tidy command, what it does, and the exit codes it returns.
group: Usage
order: 2
---

```sh
$ tidy [paths...] [options]
```

With no paths, tidy reads stdin and prints the result. With one file, it prints that file formatted, unless `--check` or `--write` is set. With a folder, it searches that folder for files it knows how to format.

```sh
# print one file, formatted
$ tidy index.html

# list the files that aren't formatted; exits 1 if any
$ tidy src --check

# rewrite everything in place
$ tidy . --write

# a different width for one run
$ tidy . --write --width 100
```

## Modes

Three modes, and they are mutually exclusive.

| Flag            | What it does                                                             |
| --------------- | ------------------------------------------------------------------------ |
| _(none)_        | Print the formatted result to stdout.                                    |
| `--check`       | Print the paths that are not formatted. Changes nothing. Exits 1 if any. |
| `-w`, `--write` | Rewrite each file in place. Files already formatted are not touched.     |

## Choosing files

| Flag                      | What it does                                                                                    |
| ------------------------- | ----------------------------------------------------------------------------------------------- |
| `--parser <name>`         | Force a language instead of guessing from the file name.                                        |
| `--stdin-filepath <path>` | The path the text on stdin would have, which decides its language and which config files apply. |
| `--threads <n>`           | How many files to format at once. Defaults to one per core; `1` uses a single thread.           |

`--parser` takes `html`, `vue`, `mjml`, `handlebars`, `tempura`, `css`, `scss`, `less`, `json`, `jsonc`, `json5`, `json-stringify`, `yaml`, `toml`, `dockerfile`, `ini`, `gitconfig`, `graphql`, `babel`, `typescript`, `babel-ts`, `xml` or `markdown`. The names are Prettier's.

When tidy walks a folder it skips `node_modules` and every folder whose name starts with `.`. It does not read `.gitignore`. To leave other paths out, pass the paths you do want, or see [Ignoring code](/docs/ignore/).

## Config

| Flag              | What it does                                                |
| ----------------- | ----------------------------------------------------------- |
| `--config <path>` | Use this config for every file, instead of the nearest one. |
| `--no-config`     | Read no config files, and no `.editorconfig`.               |

Without either, each file takes the nearest `tidy.json`, `tidy.jsonc` or `tidy.toml`, on top of any `.editorconfig` that applies. Options given on the command line override both. See [Configuration file](/docs/configuration/).

## Style

Every one of these has a config equivalent, documented in [Options](/docs/options/).

| Flag                          | Default           | What it does                                                                 |
| ----------------------------- | ----------------- | ---------------------------------------------------------------------------- |
| `--width <n>`                 | `80`              | The line width the printer fits to. `120` for Dockerfiles.                   |
| `--indent <n>`                | `2`               | The width of one indent.                                                     |
| `--tabs`                      | off               | Indent with tabs.                                                            |
| `--eol <eol>`                 | `lf`              | Line endings: `lf`, `crlf` or `cr`.                                          |
| `--whitespace <how>`          | `auto`            | HTML whitespace sensitivity: `strict`, `auto` or `ignore`. `strict` for XML. |
| `--quote <quote>`             | `double`          | Quotes in Handlebars, style sheets, JSON5, YAML and JS.                      |
| `--no-trailing-comma`         |                   | No trailing commas in SCSS, JSONC, JSON5, YAML, TOML and JS.                 |
| `--no-bracket-spacing`        |                   | No spaces inside the braces of JSON and JS objects, and YAML flow mappings.  |
| `--single-attribute-per-line` | off               | Put each attribute on its own line.                                          |
| `--bracket-same-line`         | off               | Keep the `>` of a multi-line tag on the last line.                           |
| `--ignore <name>`             | `prettier-ignore` | The name of the ignore directive.                                            |
| `--no-embedded`               |                   | Leave code of other languages inside a file as it is written.                |

## Other

| Flag              | What it does       |
| ----------------- | ------------------ |
| `-v`, `--version` | Print the version. |
| `-h`, `--help`    | Print help.        |

## Exit codes

| Code | Means                                                                                       |
| ---- | ------------------------------------------------------------------------------------------- |
| `0`  | Success. With `--check`, every file is formatted.                                           |
| `1`  | With `--check`, at least one file is not formatted.                                         |
| `2`  | Something failed: a file would not parse, a path could not be read, or an option was wrong. |

A `--check` run that cannot parse a file exits `2`, not `1`, so a broken file is never reported as merely unformatted.

## stdin

```sh
$ cat index.html | tidy --stdin-filepath index.html
$ echo '{"a":1}' | tidy --parser json
```

Without `--stdin-filepath` or `--parser`, tidy treats stdin as HTML. `--stdin-filepath` is the better of the two, because it also decides which config files apply.
