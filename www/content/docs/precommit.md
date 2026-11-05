---
title: Pre-commit
summary: Format staged files before each commit, with a hook, lint-staged, husky or pre-commit.
group: Usage
order: 4
---

Formatting on commit keeps the main branch formatted without anybody remembering to run anything. The one rule: format only what is staged, and stage the result.

## A git hook, with nothing else

```sh [.git/hooks/pre-commit]
#!/bin/sh
set -eu

files=$(git diff --cached --name-only --diff-filter=d)
[ -n "$files" ] || exit 0

echo "$files" | xargs tidy --write
echo "$files" | xargs git add
```

Make it executable with `chmod +x .git/hooks/pre-commit`. `--diff-filter=d` leaves out files the commit deletes, which would otherwise fail to open.

A hook in `.git/hooks` is not committed, so every clone needs it again. `git config core.hooksPath .githooks` moves them into the repository, where they are shared:

```sh
$ mkdir .githooks && git config core.hooksPath .githooks
```

## lint-staged

```json [package.json]
{
  "lint-staged": {
    "*": "tidy --write"
  }
}
```

`lint-staged` stages what the command rewrote, so nothing else is needed. The `*` pattern is safe: tidy leaves alone any file whose name it does not recognise.

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
        entry: tidy --write
        language: system
        pass_filenames: true
```

`language: system` uses the `tidy` already on the machine, so everyone needs it installed. Until there are prebuilt binaries, that means a cargo build on each developer's machine.

## Check instead of write

A hook that rewrites files during a commit can surprise people, because the commit is not what they staged. The quieter alternative refuses the commit and leaves the fix to them:

```sh [.githooks/pre-commit]
#!/bin/sh
set -eu

files=$(git diff --cached --name-only --diff-filter=d)
[ -n "$files" ] || exit 0

if ! echo "$files" | xargs tidy --check; then
	echo
	echo "Run: tidy . --write"
	exit 1
fi
```

## Do not drop CI

A pre-commit hook is a convenience, not a guarantee: `git commit --no-verify` skips it, and so does any commit made outside the working copy. Keep the [CI check](/docs/ci/) as the thing that actually holds the line.
