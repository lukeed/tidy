# mdtidy handoff

State as of 2026-11-01, for the merge into tidy (`~/repos/oss/tidyrs`). Nothing is pushed or published. The tree is clean, and `mise run check` passes.

mdtidy is a Markdown formatter in Rust. Its default preset matches Prettier 3.9.10 byte for byte where that keeps the document's meaning, and it differs on purpose where Prettier's output would read differently from its input.

## Open work

| Item | Status |
|---|---|
| Merge into tidy | Not started. The tidy session does it and keeps this repo's history. |
| `tidy_walk` dependency | Done in `mdtidy_cli` as a path dependency on `../../../tidyrs/crates/tidy_walk`. The merge replaces it with a workspace dependency. Until then, CI can't build this repo, because tidyrs has no remote. |
| Walk shape | `mdtidy_cli` sets `io` to the core count and `workers: 0`, the fastest shape on Markdown alone (CLI `--check`: A+B 33.8 ms, C 54 ms, D 5.2 ms on an M1 Max). After the merge, when one walk covers every language, drop this override and use tidy_walk's default: one pool is 42% slower on trees of many small files. |
| Readme benchmarks | Stale: they predate tidy_walk and the latest fixes. Replace them with the final benchmarks after the merge. |
| Layout divergences | Three entries in `divergences.md` wait for Luke's sign-off: `nested-blank`, `footnote-never`, and `wrap-atom`. |
| pulldown-cmark bugs | Two, not reported yet. Ask Luke before reporting: it is outward-facing. See [Bugs](#bugs). |
| Publishing | Nothing is published. Ask Luke first, every time. |
| `mdtidy_napi` | Builds and passes its Bun tests here. Never released; the release workflow has never run. |

## Gaps, divergences, and bugs

### Divergences

`divergences.md` (repo root) records each place where mdtidy's Prettier preset differs from Prettier on purpose. It is generated: `mise run oracle:divergences` runs every case in `oracle/divergences/*.md` through Prettier 3.9.10 (`bunx`) and the release CLI, and writes the input and both outputs. The titles, reasons, options, and sign-offs are in the table at the top of `oracle/divergences.ts`. A case where both outputs now agree is marked "stale", so remove it or fix the case.

Each entry has a kind:

- **Meaning**: Prettier's output reads differently from its input (another block opens, a link ends, emphasis nests the other way, or rendered text changes). These are signed off by Luke's standing rule: formatting never changes what a document means.
- **Layout**: two outputs that read the same, where mdtidy chose differently. Each needs Luke's sign-off, recorded as `signed: "pending"` until he gives it.

To add a divergence: add a minimal case file to `oracle/divergences/`, add its row to the table, pin mdtidy's output with a test in `crates/mdtidy/tests/format.rs`, run the task, and ask Luke for a layout entry.

### Gaps

These are places where mdtidy should match Prettier and doesn't yet. Most files first:

1. **Deep Hugo templates** (bootstrap's `examples/_index.md`): Prettier reads nested Liquid and HTML as HTML blocks and keeps their lines as written. Hugo shortcodes inside a link destination (`(…/{{< param x >}})`) also differ.
2. **A `|` inside a code span in a table cell**: GFM splits the cell there, and Prettier trims the pieces differently.
3. **Emphasis escapes**: Prettier escapes a `_` or `*` in text that could open emphasis (`\_#include`). mdtidy copies the text as written, which reads the same.
4. **Prettier's own syntax in D**: remark-math, the `@format` pragma, Liquid inside a table, a lazy `</example>` after a nested item, and `encodedLink` (where Prettier percent-encodes `>` and so changes the URL, a meaning case).
5. **bun's JSON-like test fixtures** with `(kind: ok)` before inline HTML: Prettier puts the HTML at a line start (a meaning case). With wrapping on, mdtidy moves the word before it down too.

### Bugs

Two pulldown-cmark 0.13.6 bugs, both with tabs. mdtidy follows pulldown-cmark's reading, so its output is stable but differs from CommonMark:

1. A closing fence followed by tabs (```` ```→→ ````) doesn't close the fence, so the rest of the file reads as code (seen in `v/doc/c_and_v_type_interoperability.md`).
2. A tab-indented `>` after a quote line reads as a quote marker, though a tab is 4 columns.

## Tests, oracles, and benchmarks

Every command runs through mise. Run `mise trust` once per checkout.

- **Check everything**: `mise run check` runs rustfmt, Clippy with warnings denied, `cargo test --workspace`, the napi build, and the Bun tests. Commit only when it passes.
- **Semantics oracle**: `MDTIDY_CORPUS=<dir>[:<dir>...] mise run oracle:semantics` (`crates/mdtidy/tests/semantics.rs`). Every output must re-parse to the same events and settle after one pass, with the Prettier preset, the Preserve preset, `wrap: always` at width 40, and `wrap: never`. The events are normalized in `tests/support/events.rs`: adjacent text merged, soft breaks as spaces, whitespace collapsed, and a line break between two wide characters dropped, as CSS renders it. It has caught every real bug: trust it over reading diffs. `mise run oracle:shrink -- <file> [mdtidy options...]` cuts a failing file down to the lines that still fail.
- **Prettier oracle**: `mise run oracle:prettier -- <corpus>... [--wrap preserve|always|never] [--show <kind>]` formats one copy with Prettier (cached in `target/oracle`; delete the cache after changing a corpus) and one with mdtidy, counts identical files, and groups the differences by kind. `--show` prints the hunks of one kind.
- **A/B speed**: `mise run bench:ab -- [--rev <rev>] <dir>...` builds the working tree and `<rev>` (HEAD by default, in a temporary worktree) and times both on `--check` with hyperfine, in both orders, by median. Bench every change; a fix that costs more than noise needs a reason.
- **Against other tools**: `mise run bench -- <dir> [--runs <n>]` compares mdtidy with `bun fmt` (Bun's Prettier-compatible formatter, with code blocks left alone). `bun fmt` is fast and close to Prettier (8 misses on A+B, all inside code blocks), but Prettier is the reference. For the final benchmarks, add Prettier itself and any other incumbents, such as dprint's Markdown plugin.
- **Divergences**: `mise run oracle:divergences`, see above.

### Corpora

Corpora are not in the repo. `mise run oracle:corpus -- <out> <repo>...` copies the Markdown files that each git repo tracks into `<out>/<repo name>`, skipping `node_modules`. The checkouts are local:

| Corpus | Path used | Repos | Files |
|---|---|---|---|
| A | `/tmp/mdtidy-bench/corpus/a` | `~/repos/oss/{vite,storybook,shadcn-svelte}` | 872 |
| B | `/tmp/mdtidy-bench/corpus/b` | `~/repos/oss/bun`, `~/repos/personal/starship.io/starship` | 2,142 |
| C | `/tmp/mdtidy-corpus/c` | `~/repos/oss/{angular-html-parser,rolldown,jest,v,deno,deno_lint,oxc,prettier}`, `~/repos/testing/{bootstrap,everything-claude-code}` | 3,466 |
| D | `/tmp/mdtidy-corpus/d` | `~/repos/oss/prettier`, only `tests/format` | 310 |

D is Prettier's edge-case fixtures: don't rank gaps by it. C has many files in Chinese and Japanese, which test wrapping.

### Last results

Files that match Prettier 3.9.10, by `--wrap`:

| Wrap | A | B | C | D |
|---|---|---|---|---|
| preserve (default) | 871/872 | 2,141/2,142 | 3,456/3,466 | 303/310 |
| always | 870 | 2,067 | 3,417 | 286 |
| never | 870 | 2,142 | 3,444 | 292 |

The semantics oracle passes on all four. Library speed: about 54 ms single-threaded for A+B (9.9 MB).

## Design decisions and invariants

- **Streaming**: one pass over pulldown-cmark's offset events, writing straight into a `String`, with no tree and no layout document. That is where the speed comes from. Lookahead is a small buffer (`Events::peek`). A top-level list is surveyed once (`writer/survey.rs`), so nested lists and items never read ahead again. Paragraphs are buffered only when wrapping (`wrap.rs`), and code blocks only for the `format_with` callback.
- **Never change meaning**: every output must parse to the same events as its input and settle in one pass. Where copying Prettier would break that, mdtidy differs and records why in `divergences.md`.
- **Blocks are normalized, inlines are copied**: text, HTML, entities, escapes, and image alt text are written from source slices. A text event's range starts after a backslash escape, so the writer extends it back one byte. Text loses runs of spaces (`collapse`), code spans get the fewest backticks that work (`span`), and link targets and definitions are written from their parsed values unless that would read differently.
- **Presets**: `Preset::Prettier` (the default) normalizes to Prettier's style: `-` bullets alternating with `*` in adjacent lists, `_` emphasis, `**` strong, git-diff-friendly numbering, `---` rules, backtick fences, setext headings kept, and `prettier-ignore`. `Preset::Preserve` keeps every marker as written and uses `mdtidy-ignore`. Collapsing spaces, trimming code line ends, keeping HTML beside a paragraph or HTML block, and keeping HTML indents apply to both presets: all are layout, and none changes meaning. Options parse from lowercase names with `FromStr` (`options.rs`), which the CLI and the Node binding share.
- **Ignore directives** (`directive.rs`): `<!-- name -->` keeps the next block as written, `<!-- name-start -->` to `<!-- name-end -->` keeps a range at the top level only (as in Prettier), and `<!-- name-file -->` at the top of a file (after front matter) keeps the whole file. Starship sets the name to `starship-ignore`.
- **Front matter** is found by hand at the very start of the file, with Prettier's rule (`front.rs`). pulldown-cmark's metadata flags stay off, because they find front matter anywhere.
- **Openers** (`opener.rs`): an indented continuation line never opens a block. A line that would open a quote, heading, or list joins the line before; a line of `=` or `-` gets a backslash; a fence, break, or block HTML tag keeps its indent at the top level. Wrapping never starts a line with something that opens a block: the space before it never breaks, as Prettier does for its own markers.
- **Liquid and Hugo** (`writer/liquid.rs`) are paragraph text to pulldown-cmark and blocks to Prettier. mdtidy keeps pulldown-cmark's reading and copies Prettier's layout where it is safe. Filling never breaks a Liquid tag.
- **Wrapping** (`wrap.rs`, `writer/wiki.rs`): Prettier's `fill` with its word rules. A space beside Chinese or Japanese text breaks only next to markup. A line break between two wide characters (not Korean) joins with nothing, and one between Chinese or Japanese and Latin text joins as a space. Prettier's `[[wiki links]]` never break, and a paragraph whose text has `[[` and a later `]]` or wiki link keeps its text as written. Inline link text never breaks; reference link text does. A paragraph's first line is never a lone tag.
- **Definitions have no events**: pulldown-cmark lists one per label, so the gaps between blocks are parsed for the repeats it leaves out (`writer/define.rs`).
- **File size**: keep each file under 800 lines. `writer/mod.rs` is at 799: split it before adding to it.
- **Code style**: no comments except one-line ones; doc comments in Simplified Technical English. Commit subjects are conventional and under 28 characters, with the reason in the body.

## What the merge must carry over

- **Crates**: `crates/mdtidy` (library, crates.io name `mdtidy`, free on 2026-10-30), `crates/mdtidy_cli` (binary `mdtidy`), `crates/mdtidy_napi` (`cdylib`, `publish = false`). The workspace pins Rust 1.98, edition 2024, MIT, `unsafe_code = "deny"` (napi's macros emit `unsafe`, so `forbid` breaks the build), Clippy `all = warn` with `-D warnings` in CI, and a release profile with `lto = "fat"`, `codegen-units = 1`, `strip = true`. Dependencies: `memchr`, `pulldown-cmark` 0.13.6 (no default features), `unicode-width`, `pico-args`, napi 3.16 and napi-derive 3.7.
- **mise tasks** (`mise.toml`): `build`, `build:napi`, `fmt`, `fmt:fix`, `clippy`, `test` (depends on `build:napi`), `check`, `bench`, `bench:ab`, `oracle:corpus`, `oracle:divergences`, `oracle:prettier`, `oracle:semantics`, `oracle:shrink`. Tools: Rust 1.98.0 with clippy and rustfmt, Bun 1.4.4. `[env] CARGO_TARGET_DIR = "{{config_root}}/target"` stops a starship shell's target dir from leaking in.
- **Scripts**: `bench/run.ts`, `bench/ab.ts`, `oracle/*.ts`, and `oracle/divergences/`. They find the repo root from their own path and build `-p mdtidy_cli`; rename the package there if it changes.
- **Tests**: `crates/mdtidy/tests/format.rs` (rules), `semantics.rs` (the oracle, which skips without `MDTIDY_CORPUS`), `support/events.rs` (shared with `examples/events.rs`), `fixtures/`, `crates/mdtidy_cli/tests/cli.rs`, and `crates/mdtidy_napi/index.test.ts`.
- **npm**: the napi crate's `package.json` is the published `mdtidy` package (Luke owns the name), ESM, built with `@napi-rs/cli` 3.10.9. `napi.binaryName` is `mdtidy`, and the platform packages take napi's default unscoped names (`mdtidy-darwin-arm64` and so on). `@mdtidy` on npm belongs to someone else. `repository.directory` is `crates/mdtidy_napi`: update it with the new path. `index.js`, `index.d.ts`, `*.node`, `artifacts/`, and `npm/` are generated and git-ignored.
- **Release** (`.github/workflows/release.yml`, `workflow_dispatch` only): a matrix of 8 targets (macOS arm64 and x64, Linux gnu and musl on arm64 and x64, Windows arm64 and x64), with `--use-napi-cross` for Linux gnu (glibc 2.17) and `-x` (zig) for musl. The publish job runs `napi create-npm-dirs`, `napi artifacts`, and `napi pre-publish -t npm --no-gh-release`, then `npm publish`. It needs an `NPM_TOKEN` secret, because trusted publishing can't be set up before a package exists. Actions are pinned by SHA. There is no CI workflow for pushes or pull requests.
- **Starship**: `starship/vendor/mdtidy` is a committed symlink to `/Users/lukee/repos/oss/mdtidy`, and starship's `ss_check` depends on `vendor/mdtidy/crates/mdtidy` by path. After the merge, point the symlink, or the path, at the new location, or starship stops building. Starship uses the Prettier preset, maps `lineBreaks: preserve` to `Wrap::Preserve` and `auto` to `Wrap::Always`, sets `ignore` to `starship-ignore`, and sends code fences and front matter back through its own formatters. Check its Markdown with `starship format --format minimal` after any change to defaults.
- **Repository URL**: `workspace.package.repository` is `https://github.com/lukeed/mdtidy`. Update it to tidy's.

## What I would do in the merge

- **Keep the three crates as they are**, as `crates/mdtidy`, `crates/mdtidy_cli`, and `crates/mdtidy_napi`, if tidy's layout allows it. Their names are the published names, and the scripts build `-p mdtidy_cli`.
- **Make the tidy CLI call `mdtidy::format_with` directly** for `.md` and `.markdown` in the same walk, and send code fences back through tidy's other formatters with the callback, as starship does. Keep `mdtidy_cli` as a thin standalone binary only if Luke wants both.
- **Keep `pulldown-cmark` pinned exactly** (`0.13.6`): its event ranges and the two tab bugs are part of mdtidy's behavior, and an upgrade needs the semantics and Prettier oracles run on all four corpora.
- **Move the corpus paths into one place** (a mise env var or a small config) instead of `/tmp`. They are lost on reboot today.
- **Keep the oracles as mise tasks** under a `markdown:` or `md:` prefix if tidy has its own `oracle:` tasks, and run `oracle:semantics` in CI on a small committed sample.
