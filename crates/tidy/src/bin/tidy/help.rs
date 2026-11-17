//! The help of `tidy` and of each command, in sade's layout.

const DOCS: &str = "https://tidy.lukeed.com/docs/cli/";

const FORMATTING: &str = "  Formatting Options
    --width <n>                    Line width  (default 80)
    --indent <n>                   Indent width  (default 2)
    --tabs                         Indent with tabs
    --eol <eol>                    lf, crlf or cr  (default lf)
    --quote <quote>                double or single  (default double)
    --whitespace <how>             strict, auto or ignore  (default auto)
    --no-trailing-comma            No trailing commas
    --no-bracket-spacing           No spaces inside braces
    --single-attribute-per-line    One attribute per line
    --bracket-same-line            Keep a tag's > on its last line
    --ignore <name>                Ignore directive  (default prettier-ignore)
    --no-embedded                  Skip embedded code
";

const LANGUAGES: &str = "  Languages
    css, scss, less, html, vue, mjml, hbs, tempura, js, jsx, ts, tsx, json, jsonc,
    json5, yaml, toml, md, graphql, xml, dockerfile, ini, gitconfig
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
    up        Format files in place
    check     List unformatted files
    schema    Print the config's JSON Schema
    init      Write a config file

  For more info, run any command with the `--help` flag
    $ tidy up --help
    $ tidy check --help

  Options
    --version, -v    Displays current version
    --help, -h       Displays this message

  Examples
    $ tidy up src
    $ tidy check
    $ cat a.ts | tidy up --stdin=src/a.ts
    $ cat a.ts | tidy up --stdin --lang ts
    $ tidy init

"
  )
}

/// The options, formatting options and languages that `up` and `check` share.
fn files(stdin: &str, staged: &str) -> String {
  format!(
    "  Options
    --stdin[=<path>]     {stdin}
    --staged             {staged}
    --changed[=<ref>]    Only files changed from <ref>  (default: main branch)
    --no-gitignore       Include gitignored files
    --threads <n>        Threads to use  (default: all cores)
    --lang <name>        Force a language, as in a code fence
    --config <path>      Use this config file
    --no-config          Ignore config files
    --skip-unknown       Skip unknown languages
    --help, -h           Displays this message

{FORMATTING}
{LANGUAGES}"
  )
}

/// The help of `tidy check`.
pub(crate) fn check() -> String {
  format!(
    "
  Description
    List unformatted files, and exit 1 if any.
    Documentation: {DOCS}

  Usage
    $ tidy check [paths] [options]

{options}
  Examples
    $ tidy check
    $ tidy check src --changed=main
    $ cat a.ts | tidy check --stdin=src/a.ts
    $ cat a.ts | tidy check --stdin --lang ts

",
    options = files("Check stdin", "Only staged files"),
  )
}

/// The help of `tidy up`.
pub(crate) fn up() -> String {
  format!(
    "
  Description
    Format files in place.
    Documentation: {DOCS}

  Usage
    $ tidy up [paths] [options]

{options}
  Examples
    $ tidy up
    $ tidy up --staged
    $ cat a.ts | tidy up --stdin=src/a.ts
    $ cat a.ts | tidy up --stdin --lang ts

",
    options = files("Format stdin, and print it", "Only staged files, and restage them"),
  )
}

/// The help of `tidy init`.
pub(crate) fn init() -> String {
  format!(
    "
  Description
    Write a config file, migrated from Prettier or oxfmt if one is here.
    Documentation: {DOCS}

  Usage
    $ tidy init [options]

  Options
    --json           Write a JSON file
    --toml           Write a TOML file
    --from <file>    Migrate this config
    --help, -h       Displays this message

  Examples
    $ tidy init
    $ tidy init --toml
    $ tidy init --from .prettierrc.json

"
  )
}

/// The help of `tidy schema`.
pub(crate) fn schema() -> String {
  format!(
    "
  Description
    Print the config's JSON Schema.
    Documentation: {DOCS}

  Usage
    $ tidy schema

"
  )
}
