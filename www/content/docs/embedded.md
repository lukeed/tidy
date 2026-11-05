---
title: Embedded code
summary: How tidy formats the code of one language inside another, and how to turn it off.
group: Languages
order: 2
---

A `<style>` block in an HTML file is CSS. A fenced block in Markdown is whatever its info string says. tidy formats those too, in the same pass, with the right printer and the right indent.

## What gets formatted

| Host                   | Embedded                                                    |
| ---------------------- | ----------------------------------------------------------- |
| HTML, Vue              | `<style>`, `<script>`, `style=""` attributes                |
| Markdown               | fenced code blocks, YAML and TOML front matter              |
| JavaScript, TypeScript | CSS, GraphQL, HTML and Markdown in tagged template literals |
| Any front matter       | YAML, TOML                                                  |

In Markdown, a fenced block whose language tidy knows is formatted; one it does not know is left exactly as written. So a ` ```rust ` block is untouched, and a ` ```json ` block is formatted.

````md
```json
{ "a": 1 }
```
````

becomes

````md
```json
{ "a": 1 }
```
````

## Template literals

tidy formats a tagged template when the tag says what the contents are, the way Prettier does:

```js
const styles = css`
  .a {
    color: red;
  }
`;

const query = gql`
  query User {
    id
  }
`;
```

The rules are Prettier's: the tag name, or a `/* HTML */` style comment before the literal. An untagged template is a string, and tidy leaves it alone.

An expression inside the embedded code — `${...}` — is a hole the other language's parser cannot see. tidy formats around it the way Prettier does, and where that is not possible it leaves the literal as written.

## Width and indent

Embedded code is printed to fit the remaining width at its indent, not the full width. A CSS block indented four levels inside HTML has four fewer levels of room, so it wraps earlier than the same CSS in its own file. That is Prettier's behaviour and the reason the two agree.

## Turning it off

```sh
$ tidy . --write --no-embedded
```

```json [tidy.json]
{
  "embedded": false
}
```

This is Prettier's `embeddedLanguageFormatting`. With it off, tidy formats the host language and leaves everything inside untouched.

It can be set per language, so Markdown keeps its blocks as written while HTML still formats its styles:

```json [tidy.json]
{
  "markdown": { "embedded": false }
}
```

## Benchmarks and embedded code

The [benchmark](/docs/benchmarks/) numbers on this site exclude embedded code, so that every tool does the same amount of work — not all of them format it. With embedded code on, tidy takes 286 ms over the same corpus where Prettier takes 30.9 s.

## Per-language options still apply

Embedded code uses the options of its own language, from the same config. CSS inside HTML takes the `css` section:

```json [tidy.json]
{
  "css": { "quote": "single" }
}
```

That applies to `.css` files and to `<style>` blocks alike.
