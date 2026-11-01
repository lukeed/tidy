# Markdown: notes from mdtidy

tidy's Markdown formatter was mdtidy, a separate repo until 2026-11-01. Its history is merged into tidy, with each file at its new path, so `git log` and `git blame` follow it. These are its maintainer's notes, brought up to date for tidy.

The Prettier preset matches Prettier 3.9.10 byte for byte where that keeps the document's meaning, and differs on purpose where Prettier's output would read differently from its input.

## Open work

| Item | Status |
|---|---|
| Starship | `starship/vendor/mdtidy` is a symlink to `~/repos/oss/mdtidy`, which still exists, so starship builds. Moving starship to `tidy::markdown` (rename `mdtidy::` to `tidy::markdown::` in `ss_check`, and point the path at tidy) is a change in starship. Ask Luke. |
| Readme | `readme.md` here is mdtidy's, in Luke's voice. Its benchmark numbers are stale. Replace them with the final benchmarks (`mise run bench`). |
| pulldown-cmark bugs | Two, not reported. Luke said not to report them. See [Bugs](#bugs). |
| Publishing | Nothing is published. Ask Luke first, every time. |

## Gaps, divergences, and bugs

### Divergences

`markdown-divergences.md` (repo root) records each place where the Prettier preset differs from Prettier on purpose. It is generated: `mise run oracle:markdown:divergences` runs every case in `tests/oracle/markdown/divergences/*.md` through Prettier 3.9.10 (`bunx`) and tidy's Node binding, and writes the input and both outputs. The titles, reasons, options, and sign-offs are in the table at the top of `tests/oracle/markdown/divergences.ts`. A case where both outputs now agree is marked "stale", so remove it or fix the case.

Each entry has a kind:

- **Meaning**: Prettier's output reads differently from its input (another block opens, a link ends, emphasis nests the other way, or rendered text changes). These are signed off by Luke's standing rule: formatting never changes what a document means.
- **Layout**: two outputs that read the same, where tidy chose differently. Each needs Luke's sign-off, recorded as `signed: "pending"` until he gives it.

To add a divergence: add a minimal case file to `tests/oracle/markdown/divergences/`, add its row to the table, pin the output with a test in `crates/tidy/tests/markdown.rs`, run the task, and ask Luke for a layout entry. tidy's other languages keep their deviations in `deviations.md`, which is written by hand.

### Gaps

These are places where tidy should match Prettier and doesn't yet. Most files first:

1. **Deep Hugo templates** (bootstrap's `examples/_index.md`): Prettier reads nested Liquid and HTML as HTML blocks and keeps their lines as written. Hugo shortcodes inside a link destination (`(…/{{< param x >}})`) also differ.
2. **A `|` inside a code span in a table cell**: GFM splits the cell there, and Prettier trims the pieces differently.
3. **Emphasis escapes**: Prettier escapes a `_` or `*` in text that could open emphasis (`\_#include`). tidy copies the text as written, which reads the same.
4. **Prettier's own syntax in D**: remark-math, the `@format` pragma, Liquid inside a table, a lazy `</example>` after a nested item, and `encodedLink` (where Prettier percent-encodes `>` and so changes the URL, a meaning case).
5. **bun's JSON-like test fixtures** with `(kind: ok)` before inline HTML: Prettier puts the HTML at a line start (a meaning case). With wrapping on, tidy moves the word before it down too.

### Bugs

Two pulldown-cmark 0.13.6 bugs, both with tabs. tidy follows pulldown-cmark's reading, so its output is stable but differs from CommonMark:

1. A closing fence followed by tabs (```` ```→→ ````) doesn't close the fence, so the rest of the file reads as code (seen in `v/doc/c_and_v_type_interoperability.md`).
2. A tab-indented `>` after a quote line reads as a quote marker, though a tab is 4 columns.

## Tests, oracles, and benchmarks

Every command runs through mise, from the repo root.

- **Check everything**: `mise run check` runs rustfmt, Clippy with warnings denied, `cargo test --workspace`, the napi build, and the Bun tests (`crates/tidy_napi/markdown.test.ts`). Commit only when it passes.
- **Corpora**: `mise run oracle:markdown:corpora` builds A to D into `target/corpus/markdown` from local checkouts (see below). `mise run oracle:markdown:corpus -- <out> <repo>[:<folder>]...` makes another.
- **Semantics oracle**: `mise run oracle:markdown:semantics` (`crates/tidy/tests/markdown_semantics.rs`) checks A to D, or the folders in `TIDY_MARKDOWN_CORPUS`. Every output must re-parse to the same events and settle after one pass, with the Prettier preset, the Preserve preset, `wrap: always` at width 40, and `wrap: never`. The events are normalized in `crates/tidy/tests/support/events.rs`: adjacent text merged, soft breaks as spaces, whitespace collapsed, and a line break between two wide characters dropped, as CSS renders it. It has caught every real bug: trust it over reading diffs. `mise run oracle:markdown:shrink -- <file> [--<option> <value>...]` cuts a failing file down to the lines that still fail.
- **Prettier oracle**: `mise run oracle:markdown:prettier -- <corpus>... [--wrap preserve|always|never] [--show <kind>]` formats one copy with Prettier (cached in `target/oracle`; delete the cache after changing a corpus) and one with tidy's Node binding, with code blocks left as written on both sides, counts identical files, and groups the differences by kind. `--show` prints the hunks of one kind.
- **A/B speed**: `mise run bench:ab -- [--rev <rev>] <dir>...` builds the working tree and `<rev>` (HEAD by default, in a temporary worktree) and times both on `--check` with hyperfine, in both orders, by median. Bench every change; a fix that costs more than noise needs a reason.
- **Against other tools**: `mise run bench` times tidy against each language's incumbents and on all languages at once. For Markdown: Prettier, `bun fmt`, and dprint-plugin-markdown.
- **Divergences**: `mise run oracle:markdown:divergences`, see above.

### Corpora

Corpora are not in the repo. `mise run oracle:markdown:corpora` copies the Markdown files that each checkout tracks, skipping `node_modules`:

| Corpus | Repos | Files (2026-11-01) |
|---|---|---|
| A | `~/repos/oss/{vite,storybook,shadcn-svelte}` | 872 |
| B | `~/repos/oss/bun`, `~/repos/personal/starship.io/starship` | 2,142 |
| C | `~/repos/oss/{angular-html-parser,rolldown,jest,v,deno,deno_lint,oxc,prettier}`, `~/repos/testing/{bootstrap,everything-claude-code}` | 3,781 |
| D | `~/repos/oss/prettier`, only `tests/format` | 308 |

D is Prettier's edge-case fixtures: don't rank gaps by it. C has many files in Chinese and Japanese, which test wrapping.

### Last results

Files that match Prettier 3.9.10, by `--wrap`, measured in mdtidy before the merge. After the merge, with `wrap: preserve`: A 871/872, B 2,141/2,142, C 3,764/3,781, D 301/308.

| Wrap | A | B | C | D |
|---|---|---|---|---|
| preserve (default) | 871/872 | 2,141/2,142 | 3,456/3,466 | 303/310 |
| always | 870 | 2,067 | 3,417 | 286 |
| never | 870 | 2,142 | 3,444 | 292 |

The semantics oracle passes on all four. Library speed: about 54 ms single-threaded for A+B (9.9 MB).

## Design decisions and invariants

- **Streaming**: one pass over pulldown-cmark's offset events, writing straight into a `String`, with no tree and no layout document. That is where the speed comes from. Lookahead is a small buffer (`Events::peek`). A top-level list is surveyed once (`writer/survey.rs`), so nested lists and items never read ahead again. Paragraphs are buffered only when wrapping (`wrap.rs`), and code blocks only for the `format_with` callback.
- **Never change meaning**: every output must parse to the same events as its input and settle in one pass. Where copying Prettier would break that, tidy differs and records why in `divergences.md`.
- **Blocks are normalized, inlines are copied**: text, HTML, entities, escapes, and image alt text are written from source slices. A text event's range starts after a backslash escape, so the writer extends it back one byte. Text loses runs of spaces (`collapse`), code spans get the fewest backticks that work (`span`), and link targets and definitions are written from their parsed values unless that would read differently.
- **Presets**: `Preset::Prettier` (the default) normalizes to Prettier's style: `-` bullets alternating with `*` in adjacent lists, `_` emphasis, `**` strong, git-diff-friendly numbering, `---` rules, backtick fences, setext headings kept, and `prettier-ignore`. `Preset::Preserve` keeps every marker as written and uses `tidy-ignore`. Collapsing spaces, trimming code line ends, keeping HTML beside a paragraph or HTML block, and keeping HTML indents apply to both presets: all are layout, and none changes meaning. Options parse from lowercase names with `FromStr` (`options.rs`), which the CLI and the Node binding share.
- **Ignore directives** (`directive.rs`): `<!-- name -->` keeps the next block as written, `<!-- name-start -->` to `<!-- name-end -->` keeps a range at the top level only (as in Prettier), and `<!-- name-file -->` at the top of a file (after front matter) keeps the whole file. Starship sets the name to `starship-ignore`.
- **Front matter** is found by hand at the very start of the file, with Prettier's rule (`front.rs`). pulldown-cmark's metadata flags stay off, because they find front matter anywhere.
- **Openers** (`opener.rs`): an indented continuation line never opens a block. A line that would open a quote, heading, or list joins the line before; a line of `=` or `-` gets a backslash; a fence, break, or block HTML tag keeps its indent at the top level. Wrapping never starts a line with something that opens a block: the space before it never breaks, as Prettier does for its own markers.
- **Liquid and Hugo** (`writer/liquid.rs`) are paragraph text to pulldown-cmark and blocks to Prettier. tidy keeps pulldown-cmark's reading and copies Prettier's layout where it is safe. Filling never breaks a Liquid tag.
- **Wrapping** (`wrap.rs`, `writer/wiki.rs`): Prettier's `fill` with its word rules. A space beside Chinese or Japanese text breaks only next to markup. A line break between two wide characters (not Korean) joins with nothing, and one between Chinese or Japanese and Latin text joins as a space. Prettier's `[[wiki links]]` never break, and a paragraph whose text has `[[` and a later `]]` or wiki link keeps its text as written. Inline link text never breaks; reference link text does. A paragraph's first line is never a lone tag.
- **Definitions have no events**: pulldown-cmark lists one per label, so the gaps between blocks are parsed for the repeats it leaves out (`writer/define.rs`).
- **File size**: keep each file under 800 lines. `writer/mod.rs` is at 799: split it before adding to it.
- **Code style**: no comments except one-line ones; doc comments in Simplified Technical English. Commit subjects are conventional and under 28 characters, with the reason in the body.

## How the merge was done

- **Library**: `crates/mdtidy` is the `tidy::markdown` module (`crates/tidy/src/markdown`), with the same API: `format`, `format_with`, `check`, `Options`, `Preset`, and the option enums. `pulldown-cmark` stays pinned at exactly 0.13.6: its event ranges and the two tab bugs are part of the output, and an upgrade needs the semantics and Prettier oracles on all four corpora.
- **Language**: `Language::Markdown` (`.md`, `.markdown`), `Options.markdown`, and a `markdown` section in `tidy.json`: `preset`, then `wrap`, `bullet`, `emphasis`, `strong`, `numbering`, `rule`, `fence`, `heading`, and the shared `width` and `ignore`. `tidy::format` hands code blocks in a language Prettier knows to the embed hook, and formats style sheet blocks itself when the hook declines, as Prettier does.
- **CLI and Node binding**: `mdtidy_cli` and `mdtidy_napi` are folded into `tidy_cli` and `tidy_napi`, which walk and format Markdown with the other languages, with tidy_walk's default shape. Style options are in config files, not flags. The binding adds `check()`.
- **Names**: the Preserve preset's ignore directive is `tidy-ignore`. The npm package is `tidyrs`. The release workflow builds `tidy_napi`.
