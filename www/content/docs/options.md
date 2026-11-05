---
title: Options
summary: Every option tidy accepts, the Prettier option it matches, and the languages it reaches.
group: Configuration
order: 2
---

Options go in a [config file](/docs/configuration/), at the top level or inside a language section. Most also have a command-line flag; see [CLI](/docs/cli/).

Names are shorter than Prettier's, because the prefix is implied: `width`, not `printWidth`. The behaviour is Prettier's.

## Completion in your editor

tidy ships a JSON Schema for its own config. Point `$schema` at it and an editor completes every key, shows its default, and marks a typo before tidy does:

```json [tidy.json]
{
  "$schema": "https://tidy.lukeed.com/tidy.schema.json",
  "width": 100
}
```

The same file is in the `tidyrs` package, for a project that would rather not fetch it:

```json [tidy.json]
{
  "$schema": "./node_modules/tidyrs/tidy.schema.json"
}
```

Or print it from the binary:

```sh
$ tidy schema > tidy.schema.json
```

The schema is generated from the same table the parser is checked against, so it cannot describe an option tidy does not take, or miss one it does. The [playground](/playground/) completes the config pane from it.

## Shared options

These are valid at the top level and inside any language section.

| Option                   | Type                             | Default             | Prettier                     |
| ------------------------ | -------------------------------- | ------------------- | ---------------------------- |
| `width`                  | number                           | `80`                | `printWidth`                 |
| `indent`                 | number                           | `2`                 | `tabWidth`                   |
| `tabs`                   | boolean                          | `false`             | `useTabs`                    |
| `eol`                    | `"lf"`, `"crlf"`, `"cr"`         | `"lf"`              | `endOfLine`                  |
| `whitespace`             | `"strict"`, `"auto"`, `"ignore"` | `"auto"`            | `htmlWhitespaceSensitivity`  |
| `quote`                  | `"double"`, `"single"`           | `"double"`          | `singleQuote`                |
| `trailingComma`          | boolean                          | `true`              | `trailingComma`              |
| `bracketSpacing`         | boolean                          | `true`              | `bracketSpacing`             |
| `bracketSameLine`        | boolean                          | `false`             | `bracketSameLine`            |
| `singleAttributePerLine` | boolean                          | `false`             | `singleAttributePerLine`     |
| `ignore`                 | string                           | `"prettier-ignore"` | —                            |
| `embedded`               | boolean                          | `true`              | `embeddedLanguageFormatting` |

`width`, `indent`, `tabs`, `eol`, `ignore` and `embedded` reach every language. `width` defaults to 120 for Dockerfiles, and `whitespace` to `"strict"` for XML. The rest reach only where the idea applies:

- `whitespace` — HTML, Vue, Handlebars and XML.
- `quote` — Handlebars, style sheets, JSON5, YAML and JS.
- `trailingComma` — SCSS, JSONC, JSON5, YAML flow collections, TOML arrays and JS.
- `bracketSpacing` — JSON and JS objects, and YAML flow mappings.
- `bracketSameLine`, `singleAttributePerLine` — HTML, Vue, XML and JSX.

`ignore` names the [ignore directive](/docs/ignore/); `embedded` turns [embedded code](/docs/embedded/) on and off.

```json [tidy.json]
{
  "width": 100,
  "tabs": true,
  "quote": "single",
  "css": { "quote": "double" }
}
```

## Language sections

Each section takes every shared option, plus its own.

### `js`

Covers JavaScript, TypeScript, JSX and TSX, and JavaScript wherever tidy formats it inside another language.

| Option                         | Type                                        | Default       | Prettier                       |
| ------------------------------ | ------------------------------------------- | ------------- | ------------------------------ |
| `semi`                         | boolean                                     | `true`        | `semi`                         |
| `jsxSingleQuote`               | boolean                                     | `false`       | `jsxSingleQuote`               |
| `quoteProps`                   | `"as-needed"`, `"consistent"`, `"preserve"` | `"as-needed"` | `quoteProps`                   |
| `trailingComma`                | `"all"`, `"es5"`, `"none"`, or boolean      | `"all"`       | `trailingComma`                |
| `arrowParens`                  | `"always"`, `"avoid"`                       | `"always"`    | `arrowParens`                  |
| `objectWrap`                   | `"preserve"`, `"collapse"`                  | `"preserve"`  | `objectWrap`                   |
| `experimentalTernaries`        | boolean                                     | `false`       | `experimentalTernaries`        |
| `experimentalOperatorPosition` | `"start"`, `"end"`                          | `"end"`       | `experimentalOperatorPosition` |
| `jsdoc`                        | boolean                                     | `true`        | —                              |

`jsdoc` formats JSDoc comments the way `prettier-plugin-jsdoc` does. It is on by default, which is one of tidy's two departures from Prettier's defaults — see [Option philosophy](/docs/option-philosophy/). Turn it off with `"js": { "jsdoc": false }`.

#### Sorting imports

Off by default. Two ways to turn it on:

```json [tidy.json]
{
  "js": { "sortImports": true }
}
```

That is oxfmt's sorter. Or name a Prettier plugin you already configure, and give it its own options:

```json [tidy.json]
{
  "js": {
    "plugins": ["@ianvs/prettier-plugin-sort-imports"],
    "importOrder": ["^node:", "<THIRD_PARTY_MODULES>", "^[./]"]
  }
}
```

The plugins understood are `@trivago/prettier-plugin-sort-imports`, `@ianvs/prettier-plugin-sort-imports` and `prettier-plugin-organize-imports`. tidy does not load the package — it ports the sorter and reads that plugin's option names, so the behaviour follows the config you already have. No other Prettier plugin is supported.

### `markdown`

The one section with more than Prettier offers. The default is Prettier's behaviour exactly.

| Option      | Type                                             | Default        | What it sets                                     |
| ----------- | ------------------------------------------------ | -------------- | ------------------------------------------------ |
| `preset`    | `"prettier"`, `"preserve"`                       | `"prettier"`   | A starting set of the options below.             |
| `wrap`      | `"preserve"`, `"always"`, `"never"`              | `"preserve"`   | Prettier's `proseWrap`.                          |
| `bullet`    | `"keep"`, `"dash"`, `"asterisk"`, `"plus"`       | `"dash"`       | The bullet marker.                               |
| `emphasis`  | `"keep"`, `"underscore"`, `"asterisk"`           | `"underscore"` | The emphasis delimiter.                          |
| `strong`    | `"keep"`, `"asterisk"`, `"underscore"`           | `"asterisk"`   | The strong delimiter.                            |
| `numbering` | `"keep"`, `"auto"`                               | `"auto"`       | The numbers of ordered items.                    |
| `rule`      | `"keep"`, `"dash"`, `"asterisk"`, `"underscore"` | `"dash"`       | Thematic breaks.                                 |
| `fence`     | `"keep"`, `"backtick"`, `"tilde"`                | `"backtick"`   | Code fences.                                     |
| `heading`   | `"keep"`, `"atx"`                                | `"keep"`       | `"atx"` turns setext headings into `#` prefixes. |

`"preset": "preserve"` sets every marker option to `"keep"`, so tidy normalises the layout of blocks and leaves your markers alone. It also renames the ignore directive to `tidy-ignore`; set `markdown.ignore` to change it back.

```json [tidy.json]
{
  "markdown": { "preset": "preserve", "wrap": "always", "width": 80 }
}
```

Adjacent sibling lists alternate markers, so that two lists in a row stay two lists.

### `json`

Covers JSON, JSONC, JSON5 and `package.json`.

| Option       | Type                                        | Default       | Prettier     |
| ------------ | ------------------------------------------- | ------------- | ------------ |
| `quoteProps` | `"as-needed"`, `"consistent"`, `"preserve"` | `"as-needed"` | `quoteProps` |
| `objectWrap` | `"preserve"`, `"collapse"`                  | `"preserve"`  | `objectWrap` |

### `yaml`

| Option | Type                                | Default      | Prettier    |
| ------ | ----------------------------------- | ------------ | ----------- |
| `wrap` | `"preserve"`, `"always"`, `"never"` | `"preserve"` | `proseWrap` |

### `vue`

| Option         | Type    | Default | Prettier                  |
| -------------- | ------- | ------- | ------------------------- |
| `indentBlocks` | boolean | `false` | `vueIndentScriptAndStyle` |

### `handlebars`

| Option    | Type                                  | Default  | What it sets                     |
| --------- | ------------------------------------- | -------- | -------------------------------- |
| `dialect` | `"auto"`, `"handlebars"`, `"tempura"` | `"auto"` | Which mustache grammar to parse. |

### `xml`

Covers XML, SVG and the other file types `@prettier/plugin-xml` handles. Its options keep that plugin's names.

| Option             | Type                                 | Default      | Plugin                   |
| ------------------ | ------------------------------------ | ------------ | ------------------------ |
| `selfClosingSpace` | boolean                              | `true`       | `xmlSelfClosingSpace`    |
| `sortAttributes`   | boolean                              | `false`      | `xmlSortAttributesByKey` |
| `attributeQuotes`  | `"preserve"`, `"single"`, `"double"` | `"preserve"` | `xmlQuoteAttributes`     |

### `toml`

taplo's settings, in camel case. Defaults are taplo's.

`alignEntries`, `alignComments`, `alignSingleComments`, `arrayAutoExpand`, `arrayAutoCollapse`, `inlineTableExpand`, `compactArrays`, `compactInlineTables`, `compactEntries`, `indentTables`, `indentEntries`, `trailingNewline`, `reorderKeys`, `reorderArrays`, `reorderInlineTables` are booleans. `allowedBlankLines` is a number.

```json [tidy.json]
{
  "toml": { "alignEntries": true, "reorderKeys": false }
}
```

### `dockerfile`

dprint-plugin-dockerfile's settings.

| Option                  | Type    | Default |
| ----------------------- | ------- | ------- |
| `indentStages`          | boolean | `false` |
| `healthcheckCmdNewLine` | boolean | `false` |

### `ini`

Covers `.ini`, `.cfg`, `.editorconfig`, `.npmrc`, `.gitconfig` and the rest.

| Option              | Type    | Default | Plugin                 |
| ------------------- | ------- | ------- | ---------------------- |
| `spaceAroundEquals` | boolean | `true`  | `iniSpaceAroundEquals` |

### `html`, `css`, `graphql`

Shared options only. `css` covers SCSS and Less.

## Prettier options with no equivalent

| Prettier                        | Why not                                                       |
| ------------------------------- | ------------------------------------------------------------- |
| `parser`                        | A command-line flag, `--parser`, not a config key.            |
| `filepath`                      | A flag, `--stdin-filepath`.                                   |
| `plugins`                       | Only under `js`, and only for the import sorters named above. |
| `rangeStart`, `rangeEnd`        | Not supported. Use an [ignore directive](/docs/ignore/).      |
| `requirePragma`, `insertPragma` | Not supported.                                                |
| `endOfLine: "auto"`             | Not supported. Name the ending you want.                      |
| `overrides`                     | Not supported. Use a `tidy.json` in the folder instead.       |
| `jsxBracketSameLine`            | Removed in Prettier 3. Use `bracketSameLine`.                 |
