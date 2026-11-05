---
title: Configuration file
summary: Where tidy looks for tidy.json, how the nearest one wins, and how it stacks with .editorconfig and the command line.
group: Configuration
order: 1
---

tidy reads one config file per file it formats: the nearest one above it. There is no `extends`, no cascade of several configs, and no plugin resolution.

## File names

In each folder, tidy looks for these, in this order, and uses the first it finds:

1. `tidy.json`
2. `tidy.jsonc`
3. `tidy.toml`

`tidy.json` is parsed as JSONC, so comments and trailing commas are fine in it too. The `.jsonc` name exists only to say so.

```json [tidy.json]
{
  "width": 100,
  "js": { "semi": false, "quote": "single" },
  "markdown": { "wrap": "always" }
}
```

```toml [tidy.toml]
width = 100

[js]
semi = false
quote = "single"

[markdown]
wrap = "always"
```

The two are the same config. Keys are identical; TOML tables are the language sections.

## Which config a file gets

For each file, tidy walks up from the file's own folder and uses the first config it finds. It stops at the root of the filesystem.

```
repo/
  tidy.json            ← used by everything below, except
  src/
    app.ts
  vendor/
    tidy.json          ← used by everything in vendor/
    legacy.js
```

Configs do not merge. `vendor/legacy.js` takes `vendor/tidy.json` and nothing from `repo/tidy.json`. If you want shared values, repeat them.

## Shared keys and language sections

A key at the top level applies to every language that has it. A key inside a language section applies to that language only, and wins:

```json [tidy.json]
{
  "width": 100,
  "quote": "single",
  "markdown": { "width": 80 }
}
```

Everything is formatted at 100 columns with single quotes; Markdown is wrapped against 80.

The sections are `html`, `vue`, `handlebars`, `css`, `json`, `yaml`, `toml`, `xml`, `markdown`, `dockerfile`, `ini`, `graphql` and `js`. `css` covers SCSS and Less; `json` covers JSONC and JSON5; `js` covers TypeScript and JSX. See [Options](/docs/options/) for what each accepts.

## What wins

From weakest to strongest:

1. tidy's defaults, which are Prettier's.
2. `.editorconfig`, for the properties it maps. See [EditorConfig](/docs/editorconfig/).
3. The nearest `tidy.json`, `tidy.jsonc` or `tidy.toml` — first its shared keys, then its language section.
4. Options given on the command line.

```sh
# the config says 100; this run uses 120
$ tidy up . --width 120
```

## Overriding the search

| Flag              | What it does                                                                        |
| ----------------- | ----------------------------------------------------------------------------------- |
| `--config <path>` | Use this file for every file, wherever they are. `.editorconfig` still applies.     |
| `--no-config`     | Read no config files, and no `.editorconfig`. Defaults and command-line flags only. |

`--no-config` is the way to see what tidy does with nothing set, which is what the comparisons on this site use.

## Errors

A config that does not parse, or that sets a key tidy does not know, stops the run with exit code `2` and a message that names the key:

```
tidy: tidy.json: unknown option `js.printWidth`
```

That is deliberate: a typo in a config is a silent change in output otherwise. The option you want there is `width`, under `js` or at the top level — see [Options](/docs/options/) for the mapping from Prettier's names.
