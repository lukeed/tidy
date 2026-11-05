---
title: Languages
summary: Every language tidy formats, the file names it recognises, and whose style it follows.
group: Languages
order: 1
---

tidy formats these, all from the same binary. Nothing to install, nothing to register.

| Language                                     | Style follows            |
| -------------------------------------------- | ------------------------ |
| JavaScript, TypeScript, JSX, TSX             | Prettier                 |
| HTML, Vue, Handlebars, MJML                  | Prettier                 |
| CSS, SCSS, Less                              | Prettier                 |
| JSON, JSONC, JSON5                           | Prettier                 |
| YAML                                         | Prettier                 |
| GraphQL                                      | Prettier                 |
| Markdown                                     | Prettier                 |
| XML, SVG                                     | `@prettier/plugin-xml`   |
| TOML                                         | taplo                    |
| Dockerfiles                                  | dprint-plugin-dockerfile |
| INI, `.editorconfig`, `.npmrc`, `.gitconfig` | `prettier-plugin-ini`    |

Four of those are separate plugins in Prettier: XML, TOML, Dockerfiles and INI.

## File names

Walking a folder, tidy picks up:

| Language               | Names                                                                                                              |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------ |
| JavaScript, TypeScript | `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`, and the rest Prettier formats                        |
| HTML                   | `.html`, `.htm`                                                                                                    |
| Vue                    | `.vue`                                                                                                             |
| Handlebars             | `.hbs`, `.handlebars`                                                                                              |
| Style sheets           | `.css`, `.scss`, `.less`                                                                                           |
| JSON                   | `.json`, `.jsonc`, `.json5`, `package.json`, and the rest Prettier formats                                         |
| YAML                   | `.yml`, `.yaml`                                                                                                    |
| TOML                   | `.toml`                                                                                                            |
| GraphQL                | `.graphql`, `.gql`, `.graphqls`                                                                                    |
| Markdown               | `.md`, `.markdown`                                                                                                 |
| Dockerfile             | `Dockerfile`, `Containerfile`, `Dockerfile.dev`, `.dockerfile`                                                     |
| INI                    | `.ini`, `.cfg`, `.editorconfig`, `.npmrc`, `.gitconfig`, `.gitmodules`, and the rest `prettier-plugin-ini` formats |
| XML                    | `.xml`, `.svg`, `.plist`, `.csproj`, `pom.xml`, and the rest `@prettier/plugin-xml` formats                        |

A file whose name tidy does not recognise is skipped. It is never an error to point tidy at a folder of mixed content.

To format something whose name does not say what it is, name the parser:

```sh
$ tidy --parser yaml .github/dependabot.yaml.tpl
$ cat config | tidy --parser ini
```

## Parser names

`--parser` takes Prettier's names, so a Prettier config translates directly:

`html`, `vue`, `mjml`, `handlebars`, `tempura`, `css`, `scss`, `less`, `json`, `jsonc`, `json5`, `json-stringify`, `yaml`, `toml`, `dockerfile`, `ini`, `gitconfig`, `graphql`, `babel`, `typescript`, `babel-ts`, `xml`, `markdown`.

`babel` and `babel-ts` are Prettier's JavaScript parsers; in tidy they select the same printer as `typescript`, with the dialect the name implies.

## Dialects within a language

Some languages have variants that change parsing but share a config section:

- **`css`** covers CSS, SCSS and Less. The dialect comes from the file extension.
- **`json`** covers JSON, JSONC and JSON5.
- **`js`** covers JavaScript, TypeScript, JSX and TSX.
- **`ini`** covers plain INI and git's config dialect, which differ in how sections and values are written.
- **`handlebars`** has a `dialect` option: `auto`, `handlebars` or `tempura`.

## Not formatted

tidy has no plugin system, so this list is the list. Astro, Svelte, Java, PHP, Solidity, Ruby and everything else Prettier reaches through plugins are not supported and cannot be added.

If that is what you need, Prettier remains the right tool for those files. The two can live in the same repository: point tidy at what it knows, and Prettier at the rest.

## Code inside code

Styles and scripts inside HTML, Markdown code blocks, front matter, and some JavaScript template literals are formatted as their own language. See [Embedded code](/docs/embedded/).
