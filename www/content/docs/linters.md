---
title: Linters
summary: Run tidy next to ESLint, oxlint or Biome without the two fighting over the same code.
group: Usage
order: 7
---

tidy formats. It has no rules and will not tell you about an unused variable. Keep a linter beside it, and make sure the linter has no opinions about layout — otherwise one tool rewrites what the other just wrote, and the loop never settles.

The rule is the same one Prettier documents: **turn off every stylistic rule in the linter.**

## ESLint

`eslint-config-prettier` exists to do exactly this, and works unchanged for tidy, because the style is the same.

```sh
$ npm install --save-dev eslint-config-prettier
```

```js [eslint.config.js]
import js from "@eslint/js";
import prettier from "eslint-config-prettier";

export default [js.configs.recommended, prettier];
```

Put it last, so it switches off the formatting rules the configs before it turned on.

Do not use `eslint-plugin-prettier`. It runs the formatter as a lint rule, which means every formatting difference becomes an error in your editor's problem list, and the formatting goes back through Node — the cost tidy exists to remove.

## oxlint

oxlint ships no stylistic rules, so there is nothing to disable:

```sh
$ oxlint
$ tidy check .
```

## Biome

Biome is a linter and a formatter in one. Use one or the other for formatting, not both:

```json [biome.json]
{
  "formatter": { "enabled": false },
  "linter": { "enabled": true }
}
```

With the formatter off, Biome lints and tidy formats.

## Stylelint

```sh
$ npm install --save-dev stylelint-config-prettier
```

```json [.stylelintrc.json]
{
  "extends": ["stylelint-config-standard", "stylelint-config-prettier"]
}
```

Recent Stylelint versions have already dropped their stylistic rules, so on Stylelint 15 and later this config is a no-op and you can leave it out.

## Order

Lint first, format second:

```sh
$ eslint . && tidy check .
```

A lint autofix can leave code unformatted — it is rewriting syntax, not layout. Formatting afterwards cleans up after it. The other order leaves you with a formatted file that the next autofix will unformat.
