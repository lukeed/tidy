---
title: Ignoring code
summary: Keep a file, a range or one node as it is written, with a directive in the file itself.
group: Usage
order: 3
---

tidy has no ignore file. What it has is Prettier's directive: a comment that keeps the thing after it as written.

## The directive

Put `prettier-ignore` in a comment on its own line. The comment syntax is the one the language uses.

```js
// prettier-ignore
const matrix = [
  1,0,0,
  0,1,0,
];
const b = 1;
```

The array keeps its shape. The line after it is formatted as usual.

| Language               | Comment                    |
| ---------------------- | -------------------------- |
| JavaScript, TypeScript | `// prettier-ignore`       |
| CSS, SCSS, Less        | `/* prettier-ignore */`    |
| HTML, Vue, Markdown    | `<!-- prettier-ignore -->` |
| YAML                   | `# prettier-ignore`        |
| GraphQL                | `# prettier-ignore`        |
| JSX                    | `{/* prettier-ignore */}`  |

```css
/* prettier-ignore */
.a   {   color : red   }
.b {
  color: blue;
}
```

```html
<!-- prettier-ignore -->
<div   class="a"  >keep</div>
<div class="b">fix</div>
```

## What it covers

The directive applies to the next node, not the next line. In JavaScript that is the next statement or expression; in CSS the next rule; in HTML the next element with everything inside it; in YAML the next node, which for a top-level comment is usually the whole document.

## Ranges and whole files in Markdown

Markdown has three forms:

```md
<!-- prettier-ignore -->
keeps the next block

<!-- prettier-ignore-start -->
keeps everything
between these two
<!-- prettier-ignore-end -->
```

```md
<!-- prettier-ignore-file -->
keeps the whole file, when it is the first thing in it
```

## Renaming the directive

If `prettier-ignore` reads wrong in a repository that has left Prettier behind, rename it:

```json [tidy.json]
{
  "ignore": "tidy-ignore"
}
```

Or for one run:

```sh
$ tidy . --write --ignore tidy-ignore
```

The name is per language too, so `"markdown": { "ignore": "tidy-ignore" }` changes it for Markdown alone.

## Leaving paths out

tidy skips `node_modules` and every folder whose name starts with `.` when it walks a folder. It does not read `.gitignore`, `.prettierignore` or any other ignore file.

To format part of a repository, name the parts:

```sh
$ tidy src tests --check
```

To exclude a path inside one you do want, use your shell or your task runner to build the list:

```sh
$ git ls-files '*.ts' ':!:src/generated' | xargs tidy --check
```

That form has the useful property of formatting exactly what git tracks, which is usually what you meant by "the project".
