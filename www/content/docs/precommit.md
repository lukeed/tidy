---
title: Pre-commit
summary: Format staged files before each commit, with a hook, lint-staged, husky or pre-commit.
group: Usage
order: 4
---

Formatting on commit keeps the main branch formatted without anybody remembering to run anything. The one rule: format only what is staged, and stage the result.

## A git hook, with nothing else

```sh [.githooks/pre-commit]
#!/bin/sh
set -eu

tidy up --staged
```

That is the whole hook. `--staged` asks git which files are in the index, formats only those, and stages what it rewrote, so the commit holds the formatted version. A commit that touches no formattable file does nothing and succeeds.

A hook in `.git/hooks` is not committed, so every clone needs it again. Keep them in the repository instead:

```sh
$ mkdir .githooks
$ git config core.hooksPath .githooks
$ chmod +x .githooks/pre-commit
```

`core.hooksPath` still has to be set once per clone, so put that line in whatever your project uses to set itself up.

## lint-staged

```json [package.json]
{
  "lint-staged": {
    "*": "tidy up"
  }
}
```

`lint-staged` stages what the command rewrote, so nothing else is needed. The `*` pattern is safe: tidy leaves alone any file whose name it does not recognise.

If tidy is the only thing in your `lint-staged` config, `tidy up --staged` does the same job with nothing installed.

With husky:

```sh [.husky/pre-commit]
npx lint-staged
```

## pre-commit

```yaml [.pre-commit-config.yaml]
repos:
  - repo: local
    hooks:
      - id: tidy
        name: tidy
        entry: tidy up
        language: system
        pass_filenames: true
```

`language: system` uses the `tidy` already on the machine, so everyone needs it installed. Until there are prebuilt binaries, that means a cargo build on each developer's machine.

## Check instead of write

A hook that rewrites files during a commit can surprise people, because the commit is not quite what they staged. The quieter alternative refuses the commit and leaves the fix to them:

```sh [.githooks/pre-commit]
#!/bin/sh
set -eu

if ! tidy check --staged; then
	echo
	echo "Run: tidy up --staged"
	exit 1
fi
```

## Do not drop CI

A pre-commit hook is a convenience, not a guarantee: `git commit --no-verify` skips it, and so does any commit made outside the working copy. Keep the [CI check](/docs/ci/) as the thing that actually holds the line.
