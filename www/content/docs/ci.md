---
title: Continuous integration
summary: Fail a build when something is not formatted, on GitHub Actions and elsewhere.
group: Usage
order: 5
---

`tidy check` is the CI command. It changes nothing, prints the paths that are not formatted, and exits 1 if there are any.

```sh
$ tidy check .
```

Exit codes matter here: `1` means something is unformatted, `2` means something failed — a file would not parse, or an option was wrong. A step that treats any non-zero exit as "needs formatting" will mislabel a syntax error, so report them apart if you can.

## GitHub Actions

Until there is a tagged release, build the command in the job:

```yaml [.github/workflows/ci.yml]
name: ci

on: [push, pull_request]

jobs:
  format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: cargo-bins/cargo-binstall@main
      - run: cargo binstall --no-confirm tidy
      - run: tidy check .
```

cargo-binstall installs the prebuilt archive of the latest release, so nothing is compiled. Without a Rust toolchain, the install script does the same:

```yaml
- run: curl -fsSL https://tidy.lukeed.com | sh
- run: ~/.local/bin/tidy check .
```

## Speed

tidy uses every core by default. On a two-core GitHub runner, checking a 12,000-file repository is well under a second, so the job is dominated by checkout and install, not by formatting.

If you want it to stay out of the way of other work on the runner, cap it:

```sh
$ tidy check . --threads 2
```

## Checking only what changed

On a large repository, checking the whole tree is still fast enough that filtering is rarely worth the complexity. If you want it anyway:

```sh
$ tidy check --changed=origin/main
```

`--changed` asks git which files differ from that ref, including ones you have not committed, and formats only those. See [CLI](/docs/cli/#only-what-git-says-changed).

## Other services

tidy is one binary and one command, so it fits anywhere:

```yaml [.gitlab-ci.yml]
format:
  image: alpine:latest
  script:
    - apk add --no-cache curl
    - curl -fsSL https://tidy.lukeed.com | sh
    - ~/.local/bin/tidy check .
```

```sh [Docker]
RUN curl -fsSL https://tidy.lukeed.com | sh
```

The Linux builds are statically linked against musl, so they also run in a `scratch` or `distroless` image with nothing else in it.
