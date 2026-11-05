---
title: EditorConfig
summary: The .editorconfig properties tidy reads, and how they sit under tidy.json.
group: Configuration
order: 3
---

tidy reads `.editorconfig` for the four properties that describe layout. They sit under a `tidy.json`, so a tidy config always wins, and over the defaults.

No package to install and nothing to turn on: if an `.editorconfig` applies to a file, tidy uses it.

## Properties

| Property          | Sets     | Notes                                                        |
| ----------------- | -------- | ------------------------------------------------------------ |
| `indent_style`    | `tabs`   | `tab` means indent with tabs; anything else means spaces.    |
| `indent_size`     | `indent` | `indent_size = tab` also sets `tabs`.                        |
| `tab_width`       | `indent` | Used when indenting with tabs; otherwise `indent_size` wins. |
| `max_line_length` | `width`  | `off` means no limit.                                        |
| `end_of_line`     | `eol`    | `lf`, `crlf` or `cr`.                                        |

Every other property — `charset`, `trim_trailing_whitespace`, `insert_final_newline` and the rest — is ignored. The last two describe what tidy does anyway.

```ini [.editorconfig]
root = true

[*]
indent_style = tab
end_of_line = lf

[*.{json,yml,yaml}]
indent_style = space
indent_size = 2

[*.md]
max_line_length = 80
```

With that file and no `tidy.json`, tidy indents with tabs, writes LF endings, indents JSON and YAML with two spaces, and fits Markdown to 80 columns.

## How the two stack

```ini [.editorconfig]
[*]
indent_size = 4
max_line_length = 100
```

```json [tidy.json]
{
  "indent": 2
}
```

The indent is 2, from `tidy.json`. The width is 100, from `.editorconfig`, because `tidy.json` says nothing about it.

Put differently: `.editorconfig` is a weaker layer, not a separate one. You can keep your editor settings where they are and use `tidy.json` only for what EditorConfig cannot say — quotes, semicolons, Markdown markers, and the per-language sections.

## Which files it applies to

Standard EditorConfig rules: the nearest `.editorconfig` applies, and tidy keeps walking up until one says `root = true`. Sections stack, with later and nearer ones winning, the same way your editor reads them.

## Turning it off

```sh
$ tidy up . --no-config
```

`--no-config` turns off both `.editorconfig` and the `tidy.json` search. There is no flag that turns off only one.

## The .editorconfig itself

`.editorconfig` files are INI, and tidy formats INI, so it formats them too:

```sh
$ tidy up .editorconfig
```
