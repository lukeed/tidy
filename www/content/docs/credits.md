---
title: Credits
summary: The projects tidy ports code from, and the licenses they carry.
group: Misc
order: 3
---

tidy is a port. Almost none of the hard thinking in it is original, and all of it is someone else's work under a license that allowed the port.

The authoritative list is [`docs/credits.md`](https://github.com/lukeed/tidy/blob/main/docs/credits.md) in the repository, which names versions and copyright holders.

## MIT

- **[Prettier](https://github.com/prettier/prettier)** 3.9.9 — the document printer; the HTML, Glimmer, PostCSS, JSON, YAML and GraphQL printers; comment attachment; and the tables of languages and event attributes. © James Long and contributors.
- **[Bun](https://github.com/oven-sh/bun)** — the Rust ports that tidy's HTML, Handlebars, CSS and JavaScript modules start from, and the string width tables. © Oven-sh.
- **[oxc](https://github.com/oxc-project/oxc)** — `oxc_formatter`: the JavaScript and TypeScript printer, its JSDoc formatting, and oxfmt's import sorting, through Bun's port. © VoidZero Inc. and contributors.
- **[Biome](https://github.com/biomejs/biome)** — the formatter `oxc_formatter` ports, and its intermediate representation and printer. © Biome Developers and contributors, and Rome Tools, Inc.
- **[angular-html-parser](https://github.com/prettier/angular-html-parser)** 10.12.0 — the HTML lexer, tree builder, tag definitions and entity names. © Ika and Google LLC.
- **[yaml-unist-parser](https://github.com/prettier/yaml-unist-parser)** 3.2.0 — the tree Prettier prints YAML from, and its comment attachment. © Ika.
- **[PostCSS](https://github.com/postcss/postcss)** and its scss, less, value, selector and media-query parsers — the style sheet parsers Prettier uses. © Andrey Sitnik and contributors.
- **[Handlebars](https://github.com/handlebars-lang/handlebars-parser)**, **[simple-html-tokenizer](https://github.com/tildeio/simple-html-tokenizer)** and **[@glimmer/syntax](https://github.com/glimmerjs/glimmer-vm)** — the Handlebars lexer and grammar, the HTML tokenizer, and Glimmer's tree builder. © Yehuda Katz, Tilde, Inc., and contributors.
- **[taplo](https://github.com/tamasfe/taplo)** 0.14.0 — the TOML lexer, parser and formatter. © Ferenc Tamás.
- **[@prettier/plugin-xml](https://github.com/prettier/plugin-xml)** 3.4.2 — the XML printer. © Kevin Newton.
- **[prettier-plugin-ini](https://github.com/kddnewton/prettier-plugin-ini)** 1.3.0 — the INI parser and printer. © Kevin Newton.
- **[dprint-plugin-dockerfile](https://github.com/dprint/dprint-plugin-dockerfile)** 0.7.0 — the Dockerfile parser and generator, with the parts of dprint-core's printer they use. © David Sherret.
- **[graphql-js](https://github.com/graphql/graphql-js)** 17 — the GraphQL lexer and parser, through Bun's port. © GraphQL Contributors.
- **[prettier-plugin-jsdoc](https://github.com/hosseinmd/prettier-plugin-jsdoc)** — how JSDoc comments are formatted, through Bun's port. © Hossein Mohammadi.

## ISC

- **[yaml](https://github.com/eemeli/yaml)** 2.9.0 — the lexer, the concrete syntax tree parser, and the composer. © Eemeli Aro.

## Apache 2.0

- **[@trivago/prettier-plugin-sort-imports](https://github.com/trivago/prettier-plugin-sort-imports)**, **[@ianvs/prettier-plugin-sort-imports](https://github.com/ianvs/prettier-plugin-sort-imports)** and TypeScript's `organizeImports.ts` — the import sorters, through Bun's port. © Microsoft Corporation and the respective authors. They run when [`js.sortImports` or `js.plugins`](/docs/options/#js) asks for it.
- **[@xml-tools/parser](https://github.com/SAP/xml-tools)** 1.0.12 — the XML lexer and grammar. © SAP SE or an SAP affiliate company. The port in `crates/tidy/src/xml` is a hand-written lexer and parser rather than chevrotain's, rejects the same input instead of recovering, and reads a hexadecimal character reference whole.

## tidy itself

MIT © [Luke Edwards](https://lukeed.com).
