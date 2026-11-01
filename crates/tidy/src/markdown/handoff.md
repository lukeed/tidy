# mdtidy handoff

State as of 2026-10-30. Nothing is pushed or published. Starship uses this repo through a symlink (see [Starship](#starship)).

## Picking up

1. Run `mise run check`. Done when fmt, Clippy, the Rust tests, the napi build, and the Bun tests are all green.
2. Run the semantics oracle on a real corpus (see [Oracles](#oracles)). Done when all three option sets pass with zero failures.
3. Pick the work:
   - **A formatting rule**: write the failing case in `crates/mdtidy/tests/format.rs` first, then change `writer.rs`. Done when the case passes, both oracles hold, and the Prettier match didn't drop.
   - **The Node binding**: see [napi-rs](#napi-rs). Done when `bun test` passes in `crates/mdtidy_napi` and `index.d.ts` reads as intended.
   - **Publishing**: ask Luke first. Every package is unpublished on purpose.
4. Commit each slice on its own: conventional subject under 28 characters, the reason in the body.

## Design

One pass over pulldown-cmark's offset events, writing straight into a `String`. No tree and no layout document. That is the whole speed story: keep it _streaming_.

- **Blocks are normalized, inlines are mostly copied.** Text, HTML, entities, escapes, and image alt text are written as source slices. A text event's range starts after a backslash escape, so the writer extends it back one byte (`writer/mod.rs`, `Event::Text`). Text loses runs of spaces (`collapse`, which uses memchr so plain text costs no copy), code spans get their delimiters rewritten (`span`), and link text goes through the same path as other inline content (`Link`). Inline link and image targets and definitions are written from their parsed values (`writer/inline.rs`: `tail`, `definition`), except where that would read differently: then the source stays.
- **The writer is a folder module.** `writer/mod.rs` holds the event loop and layout; `list.rs`, `code.rs`, and `inline.rs` are child modules that reach its private state. `front.rs` finds front matter and `opener.rs` finds lines that would open a block.
- **Containers are a prefix stack** (`Container`: quote, list item, footnote, indented code). A list item's marker replaces its indent on its first line.
- **Lookahead is a small buffer** (`Events`). It reads up to three ordered-list items for Prettier's numbering and alignment rules before the first item is written, and each item reads ahead to its end to decide whether it is loose (`loose`).
- **Blank lines between list items follow the source** (`Gap::Source`, `blank()`). Inside an item they follow the source too, unless the item is loose by Prettier's `isLooseListItem` (a blank line between direct children, or before the next item): then every block gets one, except a nested list.
- **Front matter is found by hand**, with Prettier's rule, only at the very start (`front.rs`). pulldown-cmark's metadata flags are off because they find front matter anywhere, even inside a list item.
- **An indented continuation line never opens a block** (`opener.rs`). Without its indent, a line starting with `>`, `#`, a list marker, a fence, a break, or a block HTML tag joins the line before; a line of `=` or `-` gets a backslash.
- **Paragraphs are buffered only when wrapping** (`wrap.rs`). A line never starts with something that opens a block (`opens_block`).
- **Code blocks are buffered** for the `format_with` callback. Front matter goes to it before parsing starts.
- **Prettier rules** (from `printer-markdown`): adjacent sibling lists alternate `-`/`*` and `.`/`)`, git-diff-friendly numbering, `isAligned` with tab width 2, `_` emphasis falling back to `*` next to a word or inside emphasis, `---` (or `***` inside a list), backtick fences one longer than any run inside, aligned tables that only compact under `wrap: never`, the block after `<!-- prettier-ignore -->` on the very next line, code spans with the fewest backticks that no run inside matches, one space for each run of spaces in text, sibling definitions on adjacent lines, no trailing whitespace on code lines, and a task item's first paragraph indented past its box (`BOX`).

## Oracles

Both oracles live in the repo. Corpora don't: rebuild them with `git ls-files '*.md'` from any repos at hand. The 2026-11 runs used four corpora:

- A: vite + storybook + shadcn-svelte (872 files).
- B: starship + bun (2,142 files, without `node_modules`).
- C: angular (`angular-html-parser`), rolldown, jest, v, bootstrap, deno, deno_lint, oxc, Prettier's docs, and everything-claude-code (3,466 files, many not in English).
- D: Prettier's own `tests/format` fixtures (310 files). They are edge cases on purpose, so don't rank gaps by them.

- **Semantics**: `MDTIDY_CORPUS=<dir> cargo test -p mdtidy --release --test semantics`. Every output re-parses to the same events (adjacent text merged, soft breaks as spaces, whitespace collapsed) and settles after one pass. It runs the Prettier preset, the Preserve preset, `wrap: always` at width 40, and `wrap: never`. It caught every real bug so far: trust it over reading diffs. When a file fails, shrink it: remove lines while the structure events of input and output still differ, until a few lines are left.
- **Speed**: build the old and the new CLI, then time both with `hyperfine -N --warmup 20 --runs 60 '<bin> <corpus> --check'` in both orders. Compare medians: means swing with machine load. Each fix on 2026-10-30 and 2026-11-01 was within noise; together they cost about 2–3% (about 38 to 39 ms on A + B).
- **Prettier**: the target is Prettier 3.9.10. Format one copy with `prettier --no-config --no-editorconfig --ignore-path= --embedded-language-formatting=off --write .` and another with `fd -H -e md -X mdtidy --write`, then count identical files. The CLI skips hidden folders on purpose, so give it every file by name. Last result: A 870/872, B 2,140/2,142, C 3,412/3,466, D 272/310. The semantics oracle passes on A and B; C and D fail only on the bugs listed below. `bun fmt` is close (8 misses, all inside code blocks) but it is not the reference.

## Known gaps against Prettier

Most files first. The rules are in Prettier's `src/language-markdown/print/` (`mdast.js`, `children.js`, `list.js`): read them before guessing.

1. Hugo and Liquid templates (bootstrap): Prettier parses `{{ }}` and `{% %}` as their own nodes, keeps their paragraph lines as written, and puts no blank line around them.
2. Tables: Prettier doesn't pad a row with fewer cells than the header. A `|` inside a code span in a cell splits the cell in GFM, and the two tools write the pieces differently.
3. Footnote definitions: Prettier keeps the text on the `[^1]:` line only for one paragraph that is on one line (or with `wrap: never`). Otherwise the text starts on the next line, indented 4.
4. A code span over two lines in a list item or quote: Prettier writes the second line with no indent and no `>`. That still parses the same but reads worse, so it isn't planned.

Kept on purpose:

- Setext headings become ATX. Prettier 3.9 keeps setext.
- HTML blocks in a task item keep their indent. Prettier indents their lines after the first past the task box, and adds 4 more spaces on every pass.
- Collapsing spaces, trimming code line ends, keeping HTML beside a paragraph or HTML block, and keeping HTML indents apply to every preset, not only Prettier's: all are layout, and none changes how the document parses.
- An indented continuation line that would open a fence, a thematic break, or an HTML block joins the line before. Prettier keeps the line break there, which changes the document.
- Link targets and definitions stay as written where Prettier's form would read differently: unbalanced parentheses, `<`, `>`, a backslash, text that decodes as an entity, or a title with `(` in parentheses.
- An emphasis mark right after a `*` is `_`. Prettier writes `*`, which merges into `***` and nests the other way.

## Known bugs that change meaning

Found by corpora C and D. The first two show up only with `wrap: always` or `wrap: never`, and they are the only ones C hits.

- Joining lines inside inline HTML or a code span changes its content: the spaces after the line break collapse.
- Filling can move a block HTML tag to the start of a line, where it opens an HTML block (`opens_block` in `wrap.rs` doesn't know HTML; `opener.rs` does).

These four D fixtures also fail with the default options:

- `blockquote/prose-wrap-always/encoded-newline.md`: an encoded newline (`&NewLine;`) in a quote.
- `ignore/top-level-range.md`: an ignore range at the top level doesn't settle after one pass.
- `list/followed-by-indented-things.md`: a list item with a wide marker, followed by indented code.
- `list/huge-first-number.md`: renumbering past 9 digits (`999999999.` becomes `1000000000.`, which isn't a list marker).

## napi-rs

`crates/mdtidy_napi` is a `cdylib` built with napi 3.16, napi-derive 3.7, and `@napi-rs/cli` 3.10.9.

- **Build**: `mise run build:napi` (`napi build --platform --release --esm`) writes `index.js`, `index.d.ts`, and `mdtidy.<platform>.node`. All three are generated and git-ignored; `mise run test` builds before it tests.
- **API**: `format(source, options?, code?)` and `check(source, options?)`. Options are camelCase strings and numbers, parsed with the library's `FromStr` impls, so an unknown value throws `unknown <option> \`<value>\``. A callback that throws makes `format` throw the same error.
- **Typing**: each option field carries `#[napi(ts_type = "'a' | 'b'")]`. Both functions carry a hand-written `ts_args_type`, because napi's own signature for the callback is unreadable. Read `index.d.ts` after any change to `src/lib.rs`.
- **The callback** is `Function<FnArgs<(String, String, u32)>, Option<String>>`, aliased as `Callback`. `FnArgs` spreads the tuple into three JS arguments; a bare tuple arrives as one array argument.
- **Package**: the crate's `package.json` is the published `mdtidy`, as in oxc's `napi/parser`. `napi.targets` lists macOS arm64/x64, Linux gnu and musl arm64/x64, and Windows arm64/x64. Platform packages take napi's default unscoped names (`mdtidy-darwin-arm64`). `@mdtidy` on npm belongs to someone else.
- **Release**: `.github/workflows/release.yml`, run by hand. Each target builds in a matrix: macOS cross-compiles x64, Linux gnu uses `--use-napi-cross` (glibc 2.17), musl uses `-x` (zig), and Windows cross-compiles arm64. The publish job runs `napi create-npm-dirs`, `napi artifacts --output-dir artifacts`, and `napi pre-publish -t npm --no-gh-release`, which publishes every platform package and adds them to `optionalDependencies`. Then it publishes `mdtidy`. Both `napi artifacts` and `napi pre-publish` fail if any target is missing.
- **Publishing, when Luke says so**: add an `NPM_TOKEN` secret first. Trusted publishing can't be set up for packages that don't exist yet. Luke owns `mdtidy` on npm.

## Starship

`starship/vendor/mdtidy` is a committed symlink to `/Users/lukee/repos/oss/mdtidy`, and `ss_check` depends on `vendor/mdtidy/crates/mdtidy` by path. Changes here show up in starship's next build. A machine or CI without this repo at that path can't build starship until mdtidy is published.

Starship formats with the Prettier preset. It maps `lineBreaks: preserve` to `Wrap::Preserve` and `auto` to `Wrap::Always`, sets `ignore` to `starship-ignore`, and sends code fences and front matter back through its own formatters. Changing a default here changes starship's output: check its Markdown with `starship format --format minimal` before committing there.

## Pitfalls

- **`CARGO_TARGET_DIR` leaks** from a starship shell into this repo, and builds land in starship's `target/`. `mise.toml` pins it to `{{config_root}}/target`; set it explicitly when running `cargo` outside mise.
- **An untrusted `mise.toml` fails the `cd`** into the repo, and anything after it in a `&&` chain is skipped. Run `mise trust` once per checkout.
- **napi's macros emit `unsafe`**, so the workspace lint is `unsafe_code = "deny"`. `forbid` can't be overridden and breaks the build.
- **Doc comments on `#[napi]` items** become `index.d.ts` JSDoc, and a `\n` in them becomes a real line break. Keep examples free of escapes.
- **The crates.io name `mdtidy`** was free on 2026-10-30. `mdfmt` is taken there by an abandoned crate.
