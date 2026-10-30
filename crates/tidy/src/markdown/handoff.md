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
- **Prettier rules** (from `printer-markdown`): adjacent sibling lists alternate `-`/`*` and `.`/`)`, git-diff-friendly numbering, `isAligned` with tab width 2, `_` emphasis falling back to `*` next to a word or inside emphasis, `---` (or `***` inside a list), backtick fences one longer than any run inside, aligned tables that only compact under `wrap: never`, and the block after `<!-- prettier-ignore -->` on the very next line.

## Oracles

Both oracles live in the repo. Corpora don't: rebuild them with `git ls-files '*.md'` from any repos at hand. The 2026-10 runs used vite + storybook + shadcn-svelte (865 files) and starship + bun (2,147 files).

- **Semantics**: `MDTIDY_CORPUS=<dir> cargo test -p mdtidy --release --test semantics`. Every output re-parses to the same events (adjacent text merged, soft breaks as spaces, whitespace collapsed) and settles after one pass. It runs the Prettier preset, the Preserve preset, `wrap: always` at width 40, and `wrap: never`. It caught every real bug so far: trust it over reading diffs.
- **Prettier**: the target is Prettier 3.9.10. Format one copy with `prettier --no-config --no-editorconfig --ignore-path= --embedded-language-formatting=off --write .` and another with `fd -H -e md -X mdtidy --write`, then count identical files. The CLI skips hidden folders on purpose, so give it every file by name. Last result: 856/872 and 2,016/2,145. `bun fmt` is close (8 misses, all inside code blocks) but it is not the reference.

## Known gaps against Prettier

Most files first. 113 of the misses are Bun's react-compiler `.expect.md` fixtures, all from the first gap.

1. HTML right after a paragraph or another HTML block: Prettier keeps them on adjacent lines. mdtidy adds a blank line.
2. Code spans: Prettier picks the fewest backticks that work (`` ` ```ts ` `` over ``` `` ```ts `` ```).
3. Runs of spaces or tabs in text: Prettier writes one space.
4. Consecutive link reference definitions: Prettier puts no blank line between them.
5. Trailing whitespace in code blocks: Prettier removes it, even with embedded formatting off.
6. HTML blocks indented up to 3 spaces: Prettier keeps the indent.
7. Task list items: Prettier aligns continuation lines after `- [ ] `.
8. Link destinations: Prettier prints them from the parsed URL: `()` becomes `(<>)`, `<z>` becomes `z`, and a URL with parentheses gets `<…>`.
9. A list item followed by a blank line: Prettier puts a blank line between all blocks in that item (`isLooseListItem` checks the item's last character).
10. A code span over two lines in a list item or quote: Prettier writes the second line with no indent and no `>`.
11. Footnote definitions with more than one line: Prettier starts the text on the next line, indented 4.
12. `isAligned` ignores Prettier's `hasIndentedCodeblock` case. The corpus doesn't hit it.

Kept on purpose:

- Setext headings become ATX. Prettier 3.9 keeps setext.
- Leading whitespace before inline HTML at a paragraph start is dropped. pulldown-cmark reports it as a text event, CommonMark renders it as nothing, and the semantics oracle ignores whitespace-only text for that reason.

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
