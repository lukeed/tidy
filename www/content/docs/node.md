---
title: Node API
summary: The tidyrs package: format and check a string from JavaScript, with no child process.
group: API
order: 1
---

`tidyrs` is a native addon. It calls the same Rust code as the command, in process, so there is no child process and no startup cost per file.

```sh
$ npm install --save-dev tidyrs
```

The package has no `bin`. For the command, see [Install](/docs/install/).

## format

```js
import { format } from "tidyrs";

let output = format("<p   >hi</p>", { filepath: "index.html" });
//=> "<p>hi</p>\n"
```

```ts
function format(
  source: string,
  options?: FormatOptions,
  embed?: (request: EmbedRequest) => string | undefined,
): string;
```

It throws if the source does not parse. The error message carries the line and column.

## check

```js
import { check } from "tidyrs";

check("<p>hi</p>\n", { filepath: "index.html" });
//=> true
```

```ts
function check(
  source: string,
  options?: FormatOptions,
  embed?: (request: EmbedRequest) => string | undefined,
): boolean;
```

`check` answers whether the source is already formatted. It is the API behind `tidy --check`, and cheaper than comparing `format(source) === source` yourself.

## Options

`FormatOptions` takes the keys of a [`tidy.json`](/docs/options/), plus two that say what is being formatted:

| Key        | Type   | What it does                                                  |
| ---------- | ------ | ------------------------------------------------------------- |
| `filepath` | string | The file's path, which decides the language and some details. |
| `parser`   | string | Force a language instead of using `filepath`.                 |

```js
format(source, {
  filepath: "app.ts",
  width: 100,
  js: { semi: false, quote: "single" },
});
```

With neither `filepath` nor `parser`, the source is treated as HTML.

There is no config file search: options come from the object you pass. If you want a project's `tidy.json`, read it yourself and spread it in.

## Embedded code

By default, `format` handles the embedded languages it knows and leaves the rest. The third argument is a hook that gets the pieces it declines, so a host can format languages tidy does not know:

```js
import { format } from "tidyrs";

format(source, { filepath: "README.md" }, (request) => {
  if (request.language !== "scss") return undefined;
  return mySassFormatter(request.code, request.width - request.indent * 2);
});
```

The request describes the hole the code sits in:

| Field      | What it is                                                                                                   |
| ---------- | ------------------------------------------------------------------------------------------------------------ |
| `code`     | The source to format.                                                                                        |
| `language` | Prettier's parser name: `babel`, `typescript`, `css`, `json`, …                                              |
| `kind`     | `block`, `expression`, `attribute` or `frontmatter`.                                                         |
| `indent`   | How many indents the code starts at. Indent every line after the first by this much.                         |
| `prefix`   | The width before the code on its first line, after the indents — such as `name="` before an attribute value. |
| `suffix`   | The width after the code on its last line — such as the `"` that closes it.                                  |
| `width`    | The configured line width.                                                                                   |
| `module`   | Whether JavaScript here is a module rather than a classic script.                                            |

Return `undefined` to leave that piece as written.

Set `embedded: false` to turn the whole mechanism off, including the hook.

## Platforms

The package picks a prebuilt addon for your platform through an optional dependency. There is no build step and no `node-gyp`.

macOS on Apple silicon and Intel; Linux on x86-64 and ARM64, against both glibc and musl; Windows on x86-64 and ARM64.

## In a formatter plugin

Because `check` and `format` take a string and return a string, wrapping tidy for an editor or a bundler is a few lines:

```js
import { format } from "tidyrs";

export function transform(code, id) {
  return { code: format(code, { filepath: id }), map: null };
}
```
