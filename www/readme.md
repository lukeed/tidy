# www

The site at [tidy.lukeed.com](https://tidy.lukeed.com), built with [bongo](https://bongo.build) and served from Cloudflare Workers static assets.

```sh
bun install
bun run dev        # http://localhost:3000
bun run build      # writes public/
bun run preview    # wrangler dev, with the Worker
bun run deploy     # build, then wrangler deploy
bun run check      # the home page samples are still what tidy prints
bun run typecheck  # the build scripts, the browser code and the Worker
bun run build:wasm # just the playground's WebAssembly
```

## Layout

| Path        | What it is                                                                                   |
| ----------- | -------------------------------------------------------------------------------------------- |
| `content/`  | Pages. `index.hbs` is the home page; `docs/*.md` are the docs.                               |
| `layouts/`  | `html.hbs` wraps everything; `docs/index.hbs` is the docs shell.                             |
| `partials/` | The header, the wordmark, the paint pots, the search dialog.                                 |
| `assets/`   | Styles, the client scripts, fonts and images.                                                |
| `data/`     | `bench.ts` holds the benchmark numbers, `release.ts` reads the version from `Cargo.toml`.    |
| `samples/`  | The before and after on the home page, plus `check.ts`, which proves they are tidy's output. |
| `worker.ts` | The analytics proxy and the install script.                                                  |

Sidebar groups come from each page's `group` frontmatter, ordered by `order`. The group names and their order live in `GROUPS` in `utils.ts`.

## The look

Every colour comes from one of the seven schemes at the top of `assets/css/base.css`. The picker in the header writes `data-scheme` on `<html>` and remembers it, and the inline script in `layouts/html.hbs` reads it back before the first paint. Nothing else in the stylesheets names a colour.

The home page sits on the bright band; every other page is paper, which `body.paper` selects. The docs add `body.docs`, which shows the search button and the phone drawer. The header is `transition:persist`, so one markup serves every page and CSS decides what shows.

Two fonts, both subset to Latin plus the arrows and the tick the pages use: Bricolage Grotesque, with its width axis, for everything but code, and JetBrains Mono for code.

## The hero

`assets/home.ts` reads `samples/routes.input.ts` and `samples/routes.output.ts` out of the two `<pre>` tags the page renders, pairs their tokens by longest common subsequence, and springs each one from where it was to where tidy puts it. **Replay** puts the mess back and tidies it again. Without the script the card shows the tidied file, and the bars below it show their final times.

## Formatting

`hk` formats this folder with tidy itself, on commit and through `mise run lint`: the TypeScript, the Markdown, the stylesheets, the Handlebars templates and the SVG. Reformatting a template changes the whitespace of every page it renders, which HTML collapses, so the pages come out the same.

## The JSON Schema

`crates/tidy/src/schema.rs` describes every `tidy.json` option. `mise run schema` writes it to `crates/tidy_napi/tidy.schema.json`, which npm ships and `content/tidy.schema.json.ts` serves at `/tidy.schema.json`. hk's `schema` step fails when that file is stale, and `cargo test -p tidy --test schema` fails when the schema and the config parser stop agreeing.

## The playground

`/playground/` runs tidy in the browser. `crates/tidy_wasm` wraps the library for wasm-bindgen, and `mise run build:wasm` writes `tidy.js` and `tidy_bg.wasm` into `assets/wasm/`, which bongo then publishes under a content-hashed name.

The left column switches between the input and `tidy.json`, so the output stays beside whichever you edit. `bun run build` runs that task first, so a built site always carries a current binary. `bun run dev` does not: run `bun run build:wasm` once after a clone, or the playground page will fail to build.

The wasm-bindgen crate and CLI versions must match exactly. Both are pinned: the crate in `crates/tidy_wasm/Cargo.toml`, the CLI in `mise.toml`. Bump them together.

## The Worker

Everything is a static asset except four paths, listed in `run_worker_first`:

- `/x/script.js` and `/x` proxy [crowne.io](https://crowne.io), so that a blocker is less likely to drop them.
- `/install.sh` serves the install script.
- `/` serves the install script to a command line (`curl`, `wget`, …) and the home page to a browser, so `curl -fsSL https://tidy.lukeed.com | sh` works.

Serving the script counts a `download` event, with `piped` (whether it came from `/` or `/install.sh`) and `platform`. The archives themselves come from GitHub releases, so that event is the closest thing to a download count.

Analytics never block the response: a failed event is logged and dropped.

## Before deploying

1. Put the crowne.io site id in `CROWNE_SITE` in `wrangler.jsonc`.
2. Set the key as a secret: `bunx wrangler secret put CROWNE_KEY`.
3. Point `tidy.lukeed.com` at the Worker, which the `routes` entry claims as a custom domain.

Without both values the Worker still serves everything; it just counts nothing.

## Benchmark numbers

`data/bench.ts` is the single source for the home page and the Benchmarks page. It is transcribed from `bench/results/format`; update it when the benchmarks are re-run.
