---
title: CLI
summary: Every flag of the tidy command, what it does, and the exit codes it returns.
group: Usage
order: 2
---

```sh
$ tidy up [paths] [options]       # format files in place
$ tidy check [paths] [options]    # list what isn't formatted
$ tidy schema                     # print the JSON Schema of tidy.json
$ tidy init                       # write a tidy.jsonc here
```

```sh
# format everything below here, in place
$ tidy up

# or name the paths, and the options
$ tidy up src tests --width 100

# list what isn't formatted; exits 1 if any
$ tidy check

# format stdin, as the file it would be, and print it
$ cat a.ts | tidy up --stdin=src/a.ts
```

## Commands

| Command       | What it does                                                                                    |
| ------------- | ----------------------------------------------------------------------------------------------- |
| `tidy up`     | Rewrite each file in place, and print what it rewrote. Files already formatted are not touched. |
| `tidy check`  | Print the paths that are not formatted. Changes nothing. Exits 1 if any.                        |
| `tidy schema` | Print the JSON Schema of `tidy.json`.                                                           |
| `tidy init`   | Write a config in this folder. See [init](#init).                                               |

`up` and `check` take as many paths as you like, and the current folder when you give none. Bare `tidy` prints the help, and `tidy <command> --help` prints a command's own.

```sh
$ tidy check          # the same as: tidy check .
$ tidy check src      # or only part of the tree
$ cat a.ts | tidy up --stdin=a.ts
```

A folder named `up` or `check` is read as the command, not as a path. Write `./up` to mean the folder.

## Only what git says changed

Instead of naming paths, let git name them. Both work with `up` and `check`.

| Flag                | What it does                                                                                                 |
| ------------------- | ------------------------------------------------------------------------------------------------------------ |
| `--staged`          | Only files staged in the index. `tidy up` stages what it rewrote, so the commit holds the formatted version. |
| `--changed[=<ref>]` | Only files that differ from a branch or commit. Without a value, the default branch.                         |

```sh
# a pre-commit hook, in one line
$ tidy up --staged

# everything this branch touched
$ tidy check --changed

# against a named branch or commit
$ tidy check --changed=main
$ tidy check --changed=HEAD~3
```

`--changed` compares the working tree against the merge base of the ref and `HEAD`, so it covers what you committed on this branch _and_ what you have not committed yet. New files that git does not track yet are included; files a `.gitignore` covers are not.

The default branch is `origin/HEAD` where that is set, else `main`, else `master`. If none of those exist, name the ref yourself.

A path narrows what git named, rather than adding to it:

```sh
# only the changed files under src/
$ tidy check --changed=main src
```

When git names no file tidy can format, the run succeeds and prints nothing — a commit that touches no formattable file is not a failure.

`--changed` takes its value with `=`, because `--changed main` cannot be told apart from `--changed` followed by the path `main`.

## Choosing files

| Flag               | What it does                                                                                                                |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| `--lang <name>`    | Force a language instead of guessing from the file name.                                                                    |
| `--stdin[=<path>]` | Read the text from stdin. The path is the one the text would have, which decides its language and which config files apply. |
| `--no-gitignore`   | Also format what `.gitignore` files leave out.                                                                              |
| `--skip-unknown`   | Leave text in a language tidy can't tell as it is, instead of failing.                                                      |
| `--threads <n>`    | How many files to format at once. Defaults to one per core; `1` uses a single thread.                                       |

`--lang` takes the name a code fence would: `css`, `scss`, `less`, `html`, `vue`, `mjml`, `hbs`, `tempura`, `js`, `jsx`, `ts`, `tsx`, `json`, `jsonc`, `json5`, `yaml`, `toml`, `md`, `graphql`, `dockerfile`, `ini` or `gitconfig`, and their longer forms, such as `typescript` or `markdown`. It also takes a file extension, such as `xml`, `svg` or `mts`. See [Languages](/docs/languages/#language-names).

When tidy walks a folder it skips `node_modules`, every folder whose name starts with `.`, and, inside a git repository, what git ignores: the `.gitignore` files, `.git/info/exclude` and your global excludes file. `--no-gitignore` formats those too. A path you name is always formatted. To leave other paths out, pass the paths you do want, or see [Ignoring code](/docs/ignore/).

## Config

| Flag              | What it does                                                |
| ----------------- | ----------------------------------------------------------- |
| `--config <path>` | Use this config for every file, instead of the nearest one. |
| `--no-config`     | Read no config files, and no `.editorconfig`.               |

Without either, each file takes the nearest `tidy.json`, `tidy.jsonc` or `tidy.toml`, on top of any `.editorconfig` that applies. Options given on the command line override both. See [Configuration file](/docs/configuration/).

## init

`tidy init` writes a `tidy.jsonc` in this folder, with a comment that links to these docs and a `$schema` for this version of tidy. `--json` writes a `tidy.json` instead, which can't hold the comment, and `--toml` writes a `tidy.toml`, with the schema in taplo's `#:schema` line. It does nothing if a `tidy.json`, `tidy.jsonc` or `tidy.toml` is already there.

If a Prettier or oxfmt config is in this folder, `tidy init` migrates it: each option under tidy's name, an override for one language as that language's section, and the import sorter that `plugins` names as `js.sortImports`. It prints each option that doesn't carry over, and why.

```sh
$ tidy init
tidy: wrote tidy.jsonc from .prettierrc.json
tidy: .prettierrc.json: the plugin `prettier-plugin-tailwindcss`: tidy runs no plugins
```

It reads `.prettierrc` in JSON or YAML, `.prettierrc.json`, `.json5`, `.yaml`, `.yml` and `.toml`, the `prettier` key of `package.json`, `.oxfmtrc.json` and `.oxfmtrc.jsonc`, and the JavaScript and TypeScript configs of both: `prettier.config.js`, `oxfmt.config.ts` and the rest. It reads a script without running it, so a value that only running it would tell, such as a spread or a function call, doesn't carry over, and tidy says on which line. If there is more than one config, `--from <file>` picks one; it also takes a config in another folder.

Without a config to migrate, it writes every option whose default is the same in every language. It leaves out `width` and `whitespace`, because a value at the top level would also change the languages whose default differs: Dockerfiles at 120 columns, and XML's `strict` whitespace.

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
| `0`  | Success. With `check`, every file is formatted.                                             |
| `1`  | With `check`, at least one file is not formatted.                                           |
| `2`  | Something failed: a file would not parse, a path could not be read, or an option was wrong. |

A `check` run that cannot parse a file exits `2`, not `1`, so a broken file is never reported as merely unformatted.

## stdin

```sh
$ cat index.html | tidy up --stdin=index.html
$ echo '{"a":1}' | tidy up --stdin --lang json
$ cat src/a.ts | tidy check --stdin=src/a.ts
```

`tidy up --stdin` prints the formatted text, because there is no file to rewrite. `tidy check --stdin` prints nothing, and exits `0` if the text is formatted or `1` if it is not.

`--stdin=<path>` is the better form, because the path also decides which config files apply. The file does not have to exist. Without a path, tidy reads no config files except one named with `--config`, and you must give `--lang`. If `--lang` is given, it wins over the path.

If tidy can't tell the language, it fails. With `--skip-unknown`, it prints the text as it is, and `check` exits `0`. The flag also works with paths: a named file in an unknown language is skipped. Editors use it so that they can send any buffer to tidy.
