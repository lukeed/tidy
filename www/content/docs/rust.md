---
title: Rust API
summary: The tidy crate: format a string, reuse a formatter across files, and handle embedded code yourself.
group: API
order: 2
---

```toml [Cargo.toml]
[dependencies]
tidy = "0"
```

## format

```rust
use tidy::{Language, Options, embed::Verbatim};

let language = Language::of_path("App.vue").unwrap();
let output = tidy::format(source, language, &Options::default(), Some("App.vue"), &mut Verbatim)?;
```

```rust
pub fn format(
    text: &str,
    language: Language,
    options: &Options,
    path: Option<&str>,
    embed: &mut dyn Embed,
) -> Result<String, Error>
```

`path` is optional, and only decides details a name settles — whether a `.hbs` file is HTML, for instance. `Language::of_path` returns `None` for a name tidy does not format.

## Formatter

`format` allocates fresh buffers each call. Formatting many files reuses them:

```rust
use tidy::{Formatter, Language, Options, embed::Verbatim};

let mut formatter = Formatter::default();
let options = Options::default();

for path in paths {
    let source = std::fs::read_to_string(&path)?;
    let Some(language) = Language::of_path(&path) else { continue };
    let output = formatter.format(&source, language, &options, Some(&path), &mut Verbatim)?;
    std::fs::write(&path, output)?;
}
```

`Formatter::format` returns a `&str` borrowed from the formatter, so it is valid until the next call. That borrow is what makes the reuse possible; `tidy::format` copies it into a `String` for you.

A `Formatter` is not `Sync`. Give each thread its own.

## Options

`Options` holds every language's options. `Options::default()` is Prettier's defaults. `Shared` is the subset that applies to all of them:

```rust
use tidy::Options;

let mut options = Options::default();
options.share(&tidy::Shared { width: Some(100), ..Default::default() });
options.js.semi = false;
```

To read a project's config instead, use `tidy::config`, which needs the `config` feature:

```toml [Cargo.toml]
[dependencies]
tidy = { version = "0", features = ["config"] }
```

```rust
use tidy::config::Resolver;

let resolver = Resolver::default();
let options = resolver.options(std::path::Path::new("src/app.ts"))?;
```

A `Resolver` looks at each folder once and is safe to share across threads, so one serves a whole walk.

## One language at a time

Each language is its own module, with its own `Options` and `format`:

```rust
let output = tidy::css::format(source, &tidy::css::Options::default(), &mut Verbatim)?;
```

The modules are `css`, `dockerfile`, `graphql`, `handlebars`, `html`, `ini`, `js`, `json`, `markdown`, `toml`, `xml` and `yaml`. Use these when you know the language and want to skip the dispatch.

## Embedded code

The last argument implements `Embed`:

```rust
pub trait Embed {
    fn format(&mut self, request: Request<'_>) -> Option<String>;
}
```

`embed::Verbatim` leaves every embedded piece as written. A closure works too:

```rust
let mut embed = |request: tidy::embed::Request<'_>| {
    (request.language == "scss").then(|| my_scss_formatter(request.code))
};
let output = tidy::format(source, language, &options, None, &mut embed)?;
```

Returning `None` leaves that piece as written.

## Errors

```rust
pub enum Error {
    Syntax { message: String, at: usize },
    TooDeep,
}
```

`at` is a byte offset into the input. `TooDeep` means the input nests deeper than the parser's stack allows, which is a guard against hostile input rather than something real code hits.

## Features

| Feature  | Default | What it adds                                                                                   |
| -------- | ------- | ---------------------------------------------------------------------------------------------- |
| `js`     | on      | oxc's parser, for JavaScript and TypeScript. Without it, scripts go to the embed hook instead. |
| `config` | off     | `tidy::config`: reading `tidy.json`, `tidy.toml` and `.editorconfig`.                          |

Markup, style sheets, data formats and Markdown are always built in. Turning `js` off makes the crate smaller and much faster to build:

```toml [Cargo.toml]
[dependencies]
tidy = { version = "0", default-features = false }
```

## The command

The CLI lives in its own crate, `tidy_cli`, and installs a binary called `tidy`:

```sh
$ cargo install --git https://github.com/lukeed/tidy tidy_cli
```
