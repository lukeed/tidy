//! The help of `tidy` and of each command, in sade's layout.

const DOCS: &str = "https://tidy.lukeed.com/docs/cli/";

const FORMATTING: &str = "  Formatting Options
    --width <n>                    Line width  (default 80, and 120 for Dockerfiles)
    --indent <n>                   Indent width  (default 2)
    --tabs                         Indent with tabs
    --eol <eol>                    lf, crlf or cr  (default lf)
    --quote <quote>                double or single  (default double)
    --whitespace <how>             strict, auto or ignore  (default auto, and strict for XML)
    --no-trailing-comma            Leave out trailing commas
    --no-bracket-spacing           No spaces inside the braces of objects
    --single-attribute-per-line    Put each attribute on its own line
    --bracket-same-line            Keep the > of a multi-line tag on its last line
    --ignore <name>                The ignore directive  (default prettier-ignore)
    --no-embedded                  Leave the code of other languages as it is
";

const PARSERS: &str = "  Parsers
    babel, babel-ts, css, dockerfile, gitconfig, graphql, handlebars, html, ini, json,
    json-stringify, json5, jsonc, less, markdown, mjml, scss, tempura, toml, typescript,
    vue, xml, yaml
";

/// The help of `tidy` alone.
pub(crate) fn global() -> String {
  format!(
    "
  Description
    A fast formatter for markup, scripts, style sheets and data.
    Documentation: {DOCS}

  Usage
    $ tidy <command> [options]

  Available Commands
    check     List the files that aren't formatted, and exit 1 if any
    up        Rewrite the files that aren't formatted
    init      Write a config in this folder
    schema    Print the JSON Schema of tidy.json

  For more info, run any command with the `--help` flag
    $ tidy check --help
    $ tidy up --help

  Options
    -v, --version    Displays current version
    -h, --help       Displays this message

  Examples
    $ tidy check
    $ tidy up src
    $ cat a.ts | tidy up --stdin=src/a.ts
    $ tidy init

"
  )
}

/// The options, formatting options and parsers that `up` and `check` share, worded for each.
fn files(verb: &str, stdin: &str) -> String {
  format!(
    "  Options
    --stdin[=<path>]     {stdin}
    --staged             Only the files staged in git{staged}
    --changed[=<ref>]    Only the files that differ from <ref>  (default: the default branch)
    --no-gitignore       Also {verb} what .gitignore leaves out
    --threads <n>        Files to {verb} at once  (default: all cores)
    --parser <name>      {Verb} as this language, from Parsers below
    --config <path>      Use this config for every file
    --no-config          Read no tidy.json and no .editorconfig
    --skip-unknown       Leave text in a language tidy can't tell as it is
    -h, --help           Displays this message

{FORMATTING}
{PARSERS}",
    staged = if verb == "format" { ", and stage them again" } else { "" },
    Verb = if verb == "format" { "Format" } else { "Check" },
  )
}

/// The help of `tidy check`.
pub(crate) fn check() -> String {
  format!(
    "
  Description
    List the files that aren't formatted, and exit 1 if any.
    Documentation: {DOCS}

  Usage
    $ tidy check [paths] [options]

{options}
  Examples
    $ tidy check
    $ tidy check src --changed=main
    $ cat a.ts | tidy check --stdin=src/a.ts
    $ cat a.ts | tidy check --stdin --parser typescript

",
    options = files("check", "Check stdin as <path>, or as --parser: exit 1 if it isn't formatted"),
  )
}

/// The help of `tidy up`.
pub(crate) fn up() -> String {
  format!(
    "
  Description
    Rewrite the files that aren't formatted, in place.
    Documentation: {DOCS}

  Usage
    $ tidy up [paths] [options]

{options}
  Examples
    $ tidy up
    $ tidy up --staged
    $ cat a.ts | tidy up --stdin=src/a.ts
    $ cat a.ts | tidy up --stdin --parser typescript

",
    options = files("format", "Format stdin as <path>, or as --parser, and print it"),
  )
}

/// The help of `tidy init`.
pub(crate) fn init() -> String {
  format!(
    "
  Description
    Write a config with the default options in this folder: tidy.jsonc, unless asked for another.
    Documentation: {DOCS}

  Usage
    $ tidy init [options]

  Options
    --json        Write tidy.json, which can't hold the comment that links to the docs
    --toml        Write tidy.toml
    -h, --help    Displays this message

  Examples
    $ tidy init
    $ tidy init --toml

"
  )
}

/// The help of `tidy schema`.
pub(crate) fn schema() -> String {
  format!(
    "
  Description
    Print the JSON Schema of tidy.json.
    Documentation: {DOCS}

  Usage
    $ tidy schema

"
  )
}
