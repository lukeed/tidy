---
title: Linters
summary: Run tidy next to ESLint or Biome without the two fighting over the same code.
group: Usage
order: 7
---

tidy formats. It has no rules and will not tell you about an unused variable. Keep a linter beside it, and make sure that linter has no opinions about layout — otherwise one tool rewrites what the other just wrote, and the loop never settles.

The rule is Prettier's: **turn off every stylistic rule in the linter.**

## ESLint

`eslint-config-prettier` turns them off, and works unchanged for tidy, because the style is the same. Nothing about your existing setup has to move.

```sh
$ npm install --save-dev eslint-config-prettier
```

```js [eslint.config.js]
import js from "@eslint/js";
import prettier from "eslint-config-prettier";

export default [js.configs.recommended, prettier];
```

Put it last, so it switches off the formatting rules that the configs before it turned on.

<!-- prettier-ignore -->
> **Do not use `eslint-plugin-prettier`.** It runs the formatter as a lint rule,
> so every formatting difference becomes an error in your editor's problem list
> — and the formatting goes back through Node, which is the cost tidy exists to
> remove. Format with `tidy up`, lint with ESLint, and keep them apart.

## Biome

Biome lints and formats. Use one of the two for formatting, not both:

```json [biome.json]
{
  "formatter": { "enabled": false },
  "linter": { "enabled": true }
}
```

With its formatter off, Biome lints and tidy formats.

## Others

oxlint ships no stylistic rules, so there is nothing to turn off. Stylelint dropped its own in version 15, so recent versions need nothing either.

## Order

Lint first, format second:

```sh
$ eslint . && tidy check .
```

A lint autofix rewrites syntax, not layout, so it can leave code unformatted. Formatting afterwards cleans up after it. The other order leaves you with a formatted file that the next autofix will unformat.
