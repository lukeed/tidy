---
title: WebAssembly
summary: The tidyrs-wasm package: tidy in browsers, Workers and Deno, with the same calls as tidyrs.
group: API
order: 2
---

`tidyrs-wasm` is tidy compiled to WebAssembly. It has the calls and the types of [`tidyrs`](/docs/node/), so code moves between the two unchanged. Use it where a native addon can't load: in a browser, a Worker or Deno. The [playground](/playground/) runs on it.

```sh
$ npm install tidyrs-wasm
```

The wasm is 3.5 MB, and 1.25 MB with gzip. It takes about 2 to 3 times as long as the native addon to format the same file.

## In a browser

Call `init` once, before the first `format`. It fetches `tidy_bg.wasm` from next to the module:

```js
import init, { format } from "tidyrs-wasm";

await init();

format("a{color:red}", { lang: "css" });
//=> "a {\n  color: red;\n}\n"
```

If a bundler or a CDN puts the wasm somewhere else, pass its URL, its bytes or a compiled module:

```js
await init({ module_or_path: "/assets/tidy_bg.wasm" });
```

## In a Worker

Import the wasm as a module, and give it to `init`:

```js
import init, { format } from "tidyrs-wasm";
import wasm from "tidyrs-wasm/tidy_bg.wasm";

await init({ module_or_path: wasm });
```

## In Node, Bun and Deno

The wasm loads as the package is imported, so `format` works at once. `await init()` does nothing there, so the same code runs in a browser and on a server.

```js
import { format } from "tidyrs-wasm";

format("let  a=1", { lang: "ts" });
//=> "let a = 1;\n"
```

`require("tidyrs-wasm")` works too.

## format and check

```ts
function format(source: string, options?: Options): string;
function check(source: string, options?: Options): boolean;
```

The same as in [`tidyrs`](/docs/node/#options): the keys of a `tidy.json`, plus `filepath`, `lang` and the [`embed`](/docs/node/#embedded-code) hook. A syntax error has its position in `loc.start`, [as in `tidyrs`](/docs/node/#format).

## config

```ts
function config(text: string): Options;
```

The options in the text of a `tidy.json`, `tidy.jsonc` or `tidy.toml`, for `format`. Text that starts with `{` or a comment is JSON; anything else is TOML. It throws on an option tidy doesn't know, and on a syntax error, with its `loc`.

```js
import { config, format } from "tidyrs-wasm";

let options = config("width = 100\n\n[js]\nsemi = false\n");
format(source, { ...options, filepath: "app.ts" });
```

## version

```ts
function version(): string;
```

The version of tidy the wasm was built from.

## As the fallback of tidyrs

Where no native addon of `tidyrs` loads, as on a platform it has no build for or in a WebContainer, `tidyrs` loads `tidyrs-wasm` if it is installed. Set `TIDYRS_WASM=1` to make it use the wasm anyway.
