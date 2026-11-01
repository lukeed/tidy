# mdtidy handoff

State as of 2026-11-01. Nothing is pushed or published. Starship uses this repo through a symlink (see [Starship](#starship)).

## Picking up

1. Run `mise run check`. Done when fmt, Clippy, the Rust tests, the napi build, and the Bun tests are all green.
2. Run the semantics oracle on a real corpus (see [Oracles](#oracles)). Done when all three option sets pass with zero failures.
3. Pick the work:
   - **A formatting rule**: write the failing case in `crates/mdtidy/tests/format.rs` first, then change `writer.rs`. Done when the case passes, both oracles hold, and the Prettier match didn't drop.
   - **The Node binding**: see [napi-rs](#napi-rs). Done when `bun test` passes in `crates/mdtidy_napi` and `index.d.ts` reads as intended.
   - **The folder walk**: the CLI walks, reads, formats and writes through `tidy_walk`, a std-only crate shared with tidy, in `~/repos/oss/tidyrs` (design: `/tmp/tidy-walk-design.md`, revision 3). It is a path dependency, and tidyrs has no remote yet, so CI can't build this workspace until Luke decides how to share it. mdtidy sets `io` to the core count and `workers: 0`, which beat tidy_walk's macOS default here (CLI `--check`: A+B 33.8 ms, C 54 ms).
   - **Publishing**: ask Luke first. Every package is unpublished on purpose.
4. Commit each slice on its own: conventional subject under 28 characters, the reason in the body.

## Design

One pass over pulldown-cmark's offset events, writing straight into a `String`. No tree and no layout document. That is the whole speed story: keep it _streaming_.

- **Blocks are normalized, inlines are mostly copied.** Text, HTML, entities, escapes, and image alt text are written as source slices. A text event's range starts after a backslash escape, so the writer extends it back one byte (`writer/mod.rs`, `Event::Text`). Text loses runs of spaces (`collapse`, which uses memchr so plain text costs no copy), code spans get their delimiters rewritten (`span`), and link text goes through the same path as other inline content (`Link`). Inline link and image targets and definitions are written from their parsed values (`writer/inline.rs`: `tail`, `definition`), except where that would read differently: then the source stays.
- **The writer is a folder module.** `writer/mod.rs` holds the event loop; `layout.rs` (gaps and prefixes), `list.rs`, `survey.rs` (what lists need to know, read once per top-level list), `code.rs`, `inline.rs` (inline tags, links, labels), `define.rs` (definitions), `footnote.rs`, and `liquid.rs` (soft breaks and Liquid tags) are child modules that reach its private state. `front.rs` finds front matter, `opener.rs` finds lines that would open a block, and `width.rs` measures columns with an ASCII fast path. Keep each file under 800 lines: split again rather than grow `mod.rs`.
- **Containers are a prefix stack** (`Container`: quote, list item, footnote, indented code). A list item's marker replaces its indent on its first line.
- **Lookahead is a small buffer** (`Events`). It reads up to three ordered-list items for Prettier's numbering and alignment rules before the first item is written. Each top-level list is surveyed once (`survey.rs`) for every nested list's marker width and looseness and every item's looseness, so nested lists never read ahead again.
- **Blank lines between list items follow the source** (`Gap::Source`, `blank()`). Inside an item they follow the source too, unless the item is loose by Prettier's `isLooseListItem` (a blank line between direct children, or before the next item): then every block gets one, except a nested list.
- **Front matter is found by hand**, with Prettier's rule, only at the very start (`front.rs`). pulldown-cmark's metadata flags are off because they find front matter anywhere, even inside a list item.
- **An indented continuation line never opens a block** (`opener.rs`). Without its indent, a line starting with `>`, `#`, or a list marker joins the line before; a line of `=` or `-` gets a backslash; a fence, a break, or a block HTML tag keeps its break and indent at the top level, and joins inside containers.
- **Definitions have no events.** pulldown-cmark lists one per label, so the gap after each sibling block (`cursor`) is parsed for the ones it left out (`define.rs`). Definitions are flushed before a container ends and after a list ends, since a list's range stretches over the definitions after it.
- **Liquid** (`{{ }}`, `{% %}`, Hugo shortcodes) is a block of its own in Prettier's parser and paragraph text in pulldown-cmark's. mdtidy keeps pulldown-cmark's reading and copies Prettier's layout where it is safe (`liquid.rs`): no blank line beside a Liquid line, lazy Liquid lines stay at the margin, an HTML start after a Liquid line keeps the rest of the paragraph as written, and filling never breaks a Liquid tag or the lines beside one.
- **Footnotes** read ahead to their end to decide whether the text starts on the `[^1]:` line (`footnote.rs`).
- **Paragraphs are buffered only when wrapping** (`wrap.rs`). A line never starts with something that opens a block (`opens_block`).
- **Code blocks are buffered** for the `format_with` callback. Front matter goes to it before parsing starts.
- **Prettier rules** (from `printer-markdown`): adjacent sibling lists alternate `-`/`*` and `.`/`)`, git-diff-friendly numbering, `isAligned` with tab width 2, `_` emphasis falling back to `*` next to a word or inside emphasis, `---` (or `***` inside a list), backtick fences one longer than any run inside, aligned tables that only compact under `wrap: never`, the block after `<!-- prettier-ignore -->` on the very next line, code spans with the fewest backticks that no run inside matches, one space for each run of spaces in text, sibling definitions on adjacent lines, no trailing whitespace on code lines, and a task item's first paragraph indented past its box (`BOX`).

## Oracles

Both oracles live in the repo, with mise tasks to run them. Corpora don't: build one with `mise run oracle:corpus -- <out> <repo>...`, which copies the Markdown files each repo tracks. The 2026-11 runs used four corpora:

- A: vite + storybook + shadcn-svelte (872 files).
- B: starship + bun (2,142 files, without `node_modules`).
- C: angular (`angular-html-parser`), rolldown, jest, v, bootstrap, deno, deno_lint, oxc, Prettier's docs, and everything-claude-code (3,466 files, many not in English).
- D: Prettier's own `tests/format` fixtures (310 files). They are edge cases on purpose, so don't rank gaps by them.

- **Semantics**: `MDTIDY_CORPUS=<dir>[:<dir>...] mise run oracle:semantics`. Every output re-parses to the same events (adjacent text merged, soft breaks as spaces, whitespace collapsed) and settles after one pass. It runs the Prettier preset, the Preserve preset, `wrap: always` at width 40, and `wrap: never`. It caught every real bug so far: trust it over reading diffs. When a file fails, `mise run oracle:shrink -- <file> [mdtidy options...]` removes lines while it still fails and prints the few that are left. The events it compares come from `tests/support/events.rs`, shared by the test and the `events` example.
- **Speed**: `mise run bench:ab -- [--rev <rev>] <dir>...` builds the working tree and `<rev>` (HEAD by default, in a temporary worktree), times both checking the folders in both orders, and compares medians: means swing with machine load. Each fix on 2026-10-30 and 2026-11-01 was within noise; together they cost about 2–3% (about 38 to 39 ms on A + B).
- **Prettier**: the target is Prettier 3.9.10. `mise run oracle:prettier -- <corpus>... [--show <kind>]` formats one copy with Prettier (cached in `target/oracle`), one with mdtidy, counts identical files, and sorts the differences by kind. It hands mdtidy every file by name, because the CLI skips hidden folders on purpose. Last result: A 871/872, B 2,141/2,142, C 3,456/3,466, D 303/310. The semantics oracle passes on all four. With `--wrap always` (which starship uses for `lineBreaks: auto`): A 870/872, B 2,067/2,142, C 3,417/3,466, D 286/310. With `--wrap never`: A 870, B 2,142, C 3,444, D 292. Most of what's left there is Prettier changing the document (an inline `<div>`, `<script>` or `<!--` moved to a line start, which opens an HTML block; the first line of a paragraph left as a lone tag; a line break between Chinese or Japanese and an emoji turned into a space), bun's JSON-like fixtures with HTML, the pulldown-cmark tab bug, and Hugo shortcodes inside link destinations. `bun fmt` is close (8 misses, all inside code blocks) but it is not the reference.

## Known gaps against Prettier

Most files first. The rules are in Prettier's `src/language-markdown/print/` (`mdast.js`, `children.js`, `list.js`): read them before guessing.

What C still misses:

1. Deep Hugo templates (bootstrap's `examples/_index.md`): Prettier reads nested Liquid and HTML as HTML blocks and keeps their lines as written.
2. A `|` inside a code span in a table cell splits the cell in GFM, and Prettier trims the pieces differently.
3. Prettier escapes a `_` or `*` in text that could open emphasis (`\_#include`); mdtidy copies text as written, which reads the same.
4. Two pulldown-cmark bugs to report upstream, both with tabs: it doesn't close a fence followed by tabs (```` ```→→ ````), so the rest of the file reads as code; and it reads a tab-indented `>` after a quote line as a quote marker, though a tab is 4 columns. mdtidy follows pulldown-cmark's reading.

What D (Prettier's fixtures) still misses, beyond those: Prettier-only syntax (remark-math, the `@format` pragma), Liquid inside a table, a lazy `</example>` after a nested item, and `encodedLink`, where Prettier percent-encodes a `>` and so changes the URL.

Kept on purpose:

- HTML blocks in a task item keep their indent. Prettier indents their lines after the first past the task box, and adds 4 more spaces on every pass.
- Collapsing spaces, trimming code line ends, keeping HTML beside a paragraph or HTML block, and keeping HTML indents apply to every preset, not only Prettier's: all are layout, and none changes how the document parses.
- An indented continuation line that would open a fence, a thematic break, or an HTML block keeps its indent. Prettier drops the indent, which opens the block.
- With `wrap: never`, the line after a hard break in a one-line footnote keeps its indent. Prettier drops it, which reads the same but worse.
- Link targets and definitions stay as written where Prettier's form would read differently: unbalanced parentheses, `<`, `>`, a backslash, text that decodes as an entity, or a title with `(` in parentheses.
- An emphasis mark right after a `*` is `_`. Prettier writes `*`, which merges into `***` and nests the other way.
- A blank line before a nested list in a list item stays when it is all that keeps the list loose; Prettier drops it, and the list turns tight.
- A code span over two lines keeps its second line as written. Prettier writes it from the parsed value without the container's indent or `>`, so in a list item the indent then eats spaces that belong to the code.

## Wrapping

With wrapping on, an atom keeps its line breaks as written and a code span over two lines is written from its parsed value. An inline link's text never breaks; a reference link's text does, as in Prettier. A line may start with inline HTML or an autolink, but never with a tag or marker that would interrupt a paragraph (`opener::interrupts`): as in Prettier, the space before it never breaks, so it moves down with the word before it. A word like `=` or `---` only opens a block alone on its line, so only a line holding it alone is refused. A paragraph's first line is never a lone tag. A space between two texts, beside Chinese or Japanese, never breaks; beside markup (code, emphasis, links, HTML) it does, as in Prettier. Prettier's `[[wiki links]]` (`writer/wiki.rs`) never break, and a paragraph whose text has `[[` and a later `]]` or wiki link keeps its text and line breaks as written. When lines join, a break between two wide characters (not Korean) becomes nothing, as Prettier writes and CSS renders it, and a break between Chinese or Japanese and Latin text becomes a space that does not break, where Prettier sometimes writes nothing and changes the text. Inside a shortcut or collapsed link label, a break between wide characters stays a line break, because joining would change the label that matches the definition. A definition too wide for its line puts its destination and title on lines of their own. One edge differs from Prettier: after an atom whose last line is already past the width, mdtidy breaks before the next word and Prettier doesn't.

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
