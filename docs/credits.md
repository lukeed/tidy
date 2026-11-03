# Credits

tidy ports code from these projects, all under the MIT license:

- [Prettier](https://github.com/prettier/prettier) 3.9.9: the document printer, the HTML, Glimmer, PostCSS, JSON, YAML and GraphQL printers, its comment attachment, and the tables of languages and event attributes. Copyright © James Long and contributors.
- [yaml-unist-parser](https://github.com/prettier/yaml-unist-parser) 3.2.0: the tree that Prettier prints YAML from, and the attachment of its comments. Copyright (c) Ika.
- [taplo](https://github.com/tamasfe/taplo) 0.14.0: the TOML lexer, parser and formatter. Copyright (c) 2020 Ferenc Tamás.
- [angular-html-parser](https://github.com/prettier/angular-html-parser) 10.12.0: the HTML lexer, tree builder, tag definitions, and entity names. Copyright (c) Ika and Copyright (c) Google LLC.
- [PostCSS](https://github.com/postcss/postcss), [postcss-scss](https://github.com/postcss/postcss-scss), [postcss-less](https://github.com/shellscape/postcss-less), [postcss-value-parser](https://github.com/TrySound/postcss-value-parser), [postcss-selector-parser](https://github.com/postcss/postcss-selector-parser), and [postcss-media-query-parser](https://github.com/dryoma/postcss-media-query-parser): the style sheet parsers that Prettier uses. Copyright (c) Andrey Sitnik and contributors.
- [Handlebars](https://github.com/handlebars-lang/handlebars-parser), [simple-html-tokenizer](https://github.com/tildeio/simple-html-tokenizer), and [@glimmer/syntax](https://github.com/glimmerjs/glimmer-vm): the Handlebars lexer and grammar, the HTML tokenizer, and the tree builder that Glimmer uses. Copyright (c) Yehuda Katz, Tilde, Inc., and contributors.
- [@prettier/plugin-xml](https://github.com/prettier/plugin-xml) 3.4.2: the XML printer. Copyright (c) 2019-present Kevin Newton.
- [dprint-plugin-dockerfile](https://github.com/dprint/dprint-plugin-dockerfile) 0.7.0: the Dockerfile parser and generator, with the parts of [dprint-core](https://github.com/dprint/dprint)'s printer that they use. Copyright (c) 2021 David Sherret.
- [graphql-js](https://github.com/graphql/graphql-js) 17: the GraphQL lexer and parser, through bun's port. Copyright (c) GraphQL Contributors.
- [prettier-plugin-ini](https://github.com/kddnewton/prettier-plugin-ini) 1.3.0: the INI parser and printer. Copyright (c) 2020-present Kevin Newton.
- [oxc](https://github.com/oxc-project/oxc)'s `oxc_formatter`: the JavaScript and TypeScript printer, its JSDoc formatting and oxfmt's import sorting, through bun's port. Copyright (c) 2024-present VoidZero Inc. & Contributors.
- [Biome](https://github.com/biomejs/biome): the formatter that `oxc_formatter` ports, and its intermediate representation and printer. Copyright (c) 2023-present Biome Developers and Contributors, and Copyright (c) 2020-2023 Rome Tools, Inc. and its affiliates.
- [prettier-plugin-jsdoc](https://github.com/hosseinmd/prettier-plugin-jsdoc): how JSDoc comments are formatted, through bun's port. Copyright (c) 2020 Hossein Mohammadi.
- [Bun](https://github.com/oven-sh/bun): the Rust ports of the above that the HTML, Handlebars, CSS and JavaScript modules start from, and the tables of string widths. Copyright (c) Oven-sh.

It also ports the lexer, the concrete syntax tree parser and the composer of [yaml](https://github.com/eemeli/yaml) 2.9.0, under the [ISC License](https://opensource.org/license/isc-license-txt). Copyright Eemeli Aro.

It also ports [@trivago/prettier-plugin-sort-imports](https://github.com/trivago/prettier-plugin-sort-imports), [@ianvs/prettier-plugin-sort-imports](https://github.com/ianvs/prettier-plugin-sort-imports) and `organizeImports.ts` of [TypeScript](https://github.com/microsoft/TypeScript) (Copyright (c) Microsoft Corporation), through bun's port, under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0). They sort imports when `js.sortImports` or `js.plugins` asks for it.

It also ports the lexer and grammar of [@xml-tools/parser](https://github.com/SAP/xml-tools) 1.0.12, under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0). Copyright (c) SAP SE or an SAP affiliate company. The port, in `crates/tidy/src/xml`, is a hand-written lexer and parser instead of chevrotain's, rejects the same input instead of recovering from errors, and reads a hexadecimal character reference whole.
