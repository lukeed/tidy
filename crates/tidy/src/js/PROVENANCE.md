# Provenance

The files here start as a verbatim copy of bun's JavaScript and TypeScript formatter, `src/format` at commit `f7607a3d350b9c9488d41eab6a18d6cac74794e6` (2026-10-29), under the MIT license. Each later commit adapts them to tidy: oxc's parser through `js::ast`, tidy's printers for embedded code, and tidy's options. To take a later fix from bun, diff bun's file against this commit and apply the change here.

| tidy | bun `src/format` |
| --- | --- |
| `js/*` but the rows below | `js/*` (`js/mod.rs` is `bun_mod.rs`) |
| `js/ir` | `ir` |
| `js/options.rs`, `text.rs`, `sort.rs`, `stack.rs` | the files of the same names |
| `js/embed/css.rs`, `markdown.rs`, `graphql.rs`, `html.rs` | `css/embed.rs`, `markdown/embed.rs`, `graphql/embed.rs`, `html/in_js.rs` |
| `js/ast` | not ported: it gives oxc's tree the handles of bun's `bun_lint::ast` (`src/lint/ast`), which bun's printer reads |
| `js/strings.rs`, `js/jsdoc/strings.rs` | the helpers of bun's `bun_core::strings` that they use, and tidy's own |
| `js/jsdoc/mdast.rs` | not ported: it builds from pulldown-cmark's events the tree that bun's `markdown::parse_plain` builds |
| `js/embed/doc.rs` | not ported: it turns the `crate::doc` stream of tidy's printers into `js::ir`, which bun's printers write directly |
| `js/template/mod.rs`, `js.rs` | `html/js.rs`, printing text for `crate::embed`; `html/vue.rs` is `html/embed.rs`'s `vue` |
| `js/sort_imports/node.rs`, `oxfmt/directives.rs`, the V8 sort in `sort.rs` | bun's `src/lint/utils/node.rs`; `is_file_level_comment` in `src/lint/fix.rs` and `has_pragma` in `pragma.rs`; `src/lint/utils/array.rs` |
| `js/sort_imports/collate.rs`, `regex.rs` | not ported: they stand in for ICU's collator, with the root collation's order of ASCII, and for `bun_lint::regex`, with regress |

Not ported: Flow (`print/flow.rs`, `attach/flow.rs`), cursor and range formatting (`cursor.rs`, `range.rs`), and bun's `lib.rs` and `js/mod.rs`, which `js/mod.rs` merges.
