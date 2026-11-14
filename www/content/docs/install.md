---
title: Install
summary: Install the tidy command, or the tidyrs package for the Node API, and check that it works.
group: Usage
order: 1
---

<!-- prettier-ignore -->
> **tidy has not had its first tagged release yet.** Until it does, build the
> command from source with cargo. The install script and the prebuilt binaries
> below describe how it will work, and will start answering the moment a release
> is tagged.

## From source

Building needs a Rust toolchain. It takes a few minutes the first time.

```sh
$ cargo install --git https://github.com/lukeed/tidy tidy
```

Or from a clone of the repository:

```sh
$ git clone https://github.com/lukeed/tidy
$ cd tidy
$ cargo install --path crates/tidy
```

Either way the binary is called `tidy`, and lands in `~/.cargo/bin`.

## npm

The `tidyrs` package is the [Node API](/docs/node/), not the command. It ships a native addon for your platform as an optional dependency, so nothing is compiled on install.

```sh
$ npm install --save-dev tidyrs
```

```js
import { format } from "tidyrs";

format("<p   >hi</p>", { filepath: "index.html" });
//=> "<p>hi</p>\n"
```

It has no `bin`, so `npx tidy` does not work. Use the command from source, above.

## Install script

```sh
$ curl -fsSL https://tidy.lukeed.com | sh
```

The script detects your platform, downloads the matching archive from the latest GitHub release, checks its SHA-256, and installs it to `~/.local/bin`. The same URL in a browser shows the home page; `curl` gets the script, because the server answers on the user agent.

Read it before you run it:

```sh
$ curl -fsSL https://tidy.lukeed.com/install.sh | less
```

Two environment variables change what it does:

| Variable       | What it does                                                          |
| -------------- | --------------------------------------------------------------------- |
| `TIDY_VERSION` | Install a specific release, such as `v0.1.0`. Defaults to the latest. |
| `TIDY_INSTALL` | Where the binary goes. Defaults to `~/.local/bin`.                    |

```sh
$ TIDY_INSTALL=/usr/local/bin curl -fsSL https://tidy.lukeed.com | sh
```

## Prebuilt binaries

Each release on [GitHub](https://github.com/lukeed/tidy/releases) carries an archive per platform, with a `.sha256` beside it in the format `sha256sum -c` reads:

| Platform             | Archive                                  |
| -------------------- | ---------------------------------------- |
| macOS, Apple silicon | `tidy-aarch64-apple-darwin.tar.gz`       |
| macOS, Intel         | `tidy-x86_64-apple-darwin.tar.gz`        |
| Linux, x86-64        | `tidy-x86_64-unknown-linux-musl.tar.gz`  |
| Linux, ARM64         | `tidy-aarch64-unknown-linux-musl.tar.gz` |
| Windows, x86-64      | `tidy-x86_64-pc-windows-msvc.zip`        |
| Windows, ARM64       | `tidy-aarch64-pc-windows-msvc.zip`       |

The Linux builds link against musl, so they run on any distribution and inside a `scratch` container.

## Verified installs

Every release carries `packslip.sigstore.json`, a [packslip](https://packslip.dev) manifest. It is signed by the release workflow's own identity, with no long-lived key, and it states each archive's digest, platform and the executables inside it.

That lets an installer verify what it downloads instead of trusting a file name or a checksum served beside the binary:

```sh
$ packslip install github.com/lukeed/tidy
```

mise reads the same manifest through its packslip backend:

```sh
$ mise use -g packslip:github.com/lukeed/tidy
```

To check an archive you already have:

```sh
$ packslip verify github.com/lukeed/tidy
```

<!-- prettier-ignore -->
> The install script above checks the `.sha256` published beside each archive,
> which catches a truncated download, not a hostile one — a POSIX shell cannot
> verify a signature. Where it matters, install through packslip or mise, which
> can.

## Check it works

```sh
$ tidy --version
$ echo '<p   >hi</p>' | tidy --stdin=index.html
```

The second command prints `<p>hi</p>`. If the shell cannot find `tidy`, the install directory is not on your `PATH`:

```sh
$ export PATH="$HOME/.local/bin:$PATH"
```

Add that to your shell profile to make it stick. For a cargo install, the directory is `~/.cargo/bin`.

## Next

- [CLI](/docs/cli/) — every flag.
- [Configuration file](/docs/configuration/) — `tidy.json`, and where it is looked for.
- [CI](/docs/ci/) — failing a build on unformatted files.
