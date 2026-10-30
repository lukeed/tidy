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

- **Blocks are normalized, inlines are copied.** Text, code spans, links, images, HTML, entities, and escapes are written as source slices. A text event's range starts after a backslash escape, so the writer extends it back one byte (`writer.rs`, `Event::Text`).
- **Containers are a prefix stack** (`Container`: quote, list item, footnote, indented code). A list item's marker replaces its indent on its first line.
- **Lookahead is a small buffer** (`Events`). It reads up to three ordered-list items for Prettier's numbering and alignment rules before the first item is written.
- **Blank lines inside and between list items follow the source** (`Gap::Source`, `blank()`), which is Prettier's `listItem.spread` rule. It also keeps list looseness by construction, so nothing tracks tightness.
- **Paragraphs are buffered only when wrapping** (`wrap.rs`). A line never starts with something that opens a block (`opens_block`).
- **Code blocks and front matter are buffered** for the `format_with` callback.
- **Prettier rules** (from `printer-markdown`): adjacent sibling lists alternate `-`/`*` and `.`/`)`, git-diff-friendly numbering, `isAligned` with tab width 2, `_` emphasis falling back to `*` next to a word or inside emphasis, `---` (or `***` inside a list), backtick fences one longer than any run inside, setext to ATX, aligned tables that only compact under `wrap: never`, and the block after `<!-- prettier-ignore -->` on the very next line.

## Oracles

Both oracles live in the repo. Corpora don't: rebuild them with `git ls-files '*.md'` from any repos at hand. The 2026-10 runs used vite + storybook + shadcn-svelte (865 files) and starship + bun (2,147 files).

- **Semantics**: `MDTIDY_CORPUS=<dir> cargo test -p mdtidy --release --test semantics`. Every output re-parses to the same events (adjacent text merged, soft breaks as spaces, whitespace collapsed) and settles after one pass. It runs the Prettier preset, the Preserve preset, `wrap: always` at width 40, and `wrap: never`. It caught every real bug so far: trust it over reading diffs.
- **Prettier**: format a copy with `bun fmt --no-config --no-editorconfig --no-script --embedded-language-formatting off` (Bun matches Prettier byte for byte) and another with `mdtidy <dir> --write`, then count identical files. Last result: 857/865 and 2,002/2,127. Most of the 125 left are Bun's react-compiler `.expect.md` fixtures.

## Known gaps against Prettier

- Code spans: Prettier picks the fewest backticks that work (`` ` ```ts ` `` over ``` `` ```ts `` ```).
- Link destinations: Prettier drops `<…>` around a destination that doesn't need it.
- Footnote definitions: continuation indentation.
- Leading whitespace before inline HTML at a paragraph start is dropped. pulldown-cmark reports it as a text event, CommonMark renders it as nothing, and the semantics oracle ignores whitespace-only text for that reason.
- `isAligned` ignores Prettier's `hasIndentedCodeblock` case.

## napi-rs

`crates/mdtidy_napi` is a `cdylib` built with napi 3.16, napi-derive 3.7, and `@napi-rs/cli` 3.10.9.

- **Build**: `mise run build:napi` (`napi build --platform --release --esm`) writes `index.js`, `index.d.ts`, and `mdtidy.<platform>.node`. All three are generated and git-ignored; `mise run test` builds before it tests.
- **API**: `format(source, options?, code?)` and `check(source, options?)`. Options are camelCase strings and numbers, parsed with the library's `FromStr` impls, so an unknown value throws `unknown <option> \`<value>\``. A callback that throws makes `format` throw the same error.
- **Typing**: each option field carries `#[napi(ts_type = "'a' | 'b'")]`. Both functions carry a hand-written `ts_args_type`, because napi's own signature for the callback is unreadable. Read `index.d.ts` after any change to `src/lib.rs`.
- **The callback** is `Function<FnArgs<(String, String, u32)>, Option<String>>`, aliased as `Callback`. `FnArgs` spreads the tuple into three JS arguments; a bare tuple arrives as one array argument.
- **Package**: `package.json` is `"private": true` so nothing publishes by accident. Its `napi.targets` lists macOS arm64/x64, Linux gnu arm64/x64, and Windows x64.
- **Publishing, when Luke says so**: drop `private`, run `napi create-npm-dirs` for the per-platform packages, build each target in CI (a GitHub Actions matrix on the native runner per target), run `napi prepublish` to wire `optionalDependencies`, then publish the platform packages before `mdtidy`. The npm name `mdtidy` was free on 2026-10-30.

## Starship

`starship/vendor/mdtidy` is a committed symlink to `/Users/lukee/repos/oss/mdtidy`, and `ss_check` depends on `vendor/mdtidy/crates/mdtidy` by path. Changes here show up in starship's next build. A machine or CI without this repo at that path can't build starship until mdtidy is published.

Starship formats with the Prettier preset. It maps `lineBreaks: preserve` to `Wrap::Preserve` and `auto` to `Wrap::Always`, sets `ignore` to `starship-ignore`, and sends code fences and front matter back through its own formatters. Changing a default here changes starship's output: check its Markdown with `starship format --format minimal` before committing there.

## Pitfalls

- **`CARGO_TARGET_DIR` leaks** from a starship shell into this repo, and builds land in starship's `target/`. `mise.toml` pins it to `{{config_root}}/target`; set it explicitly when running `cargo` outside mise.
- **An untrusted `mise.toml` fails the `cd`** into the repo, and anything after it in a `&&` chain is skipped. Run `mise trust` once per checkout.
- **napi's macros emit `unsafe`**, so the workspace lint is `unsafe_code = "deny"`. `forbid` can't be overridden and breaks the build.
- **Doc comments on `#[napi]` items** become `index.d.ts` JSDoc, and a `\n` in them becomes a real line break. Keep examples free of escapes.
- **The crates.io name `mdtidy`** was free on 2026-10-30. `mdfmt` is taken there by an abandoned crate.
