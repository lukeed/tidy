use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tidy::config::{Config, Resolver};
use tidy::embed::Verbatim;
use tidy::{
  Error, Formatter, Language, Options, Shared, Whitespace, css, doc, handlebars, ini, js, json,
};
use tidy_walk::Output;

const HELP: &str = "\
tidy: a fast formatter for markup, scripts, style sheets and data

Usage
  $ tidy [paths...] [options]          Print the formatted file
  $ tidy up [paths...] [options]       Rewrite files in place
  $ tidy check [paths...] [options]    List files that aren't formatted, and exit 1 if any
  $ tidy schema                        Print the JSON Schema of a tidy.json

  Without paths, `tidy` reads stdin and prints the result, and `up` and `check` take the
  current folder. Folders are searched for .html, .htm, .vue, .hbs, .handlebars, .css, .scss,
  .less, .md, and .markdown files, for JavaScript and TypeScript files: .js, .jsx, .mjs, .cjs,
  .ts, .tsx, .mts, .cts, and the others that Prettier formats, for JSON files: .json, .jsonc,
  .json5, package.json, and the others that Prettier formats, for .yml, .yaml, .toml, .graphql,
  .gql and .graphqls files, for Dockerfile, Containerfile, Dockerfile.dev and .dockerfile files,
  for INI files: .ini, .cfg, .editorconfig, .npmrc, .gitconfig, .gitmodules, and the others that
  prettier-plugin-ini formats, and for XML files: .xml, .svg, .plist, .csproj, pom.xml, and the
  others that @prettier/plugin-xml formats. Style sheets in <style> and style=\"\", JSON in
  scripts and code blocks, YAML and TOML in code blocks and front matter, Dockerfiles, INI and
  GraphQL in code blocks, and scripts, template expressions and JavaScript and TypeScript code
  blocks are formatted. Other code blocks are left as written.

  Each file takes its options from the nearest tidy.json, tidy.jsonc or tidy.toml, and from
  .editorconfig. Options given here override them.

Choosing files
  --staged                        Only files that are staged in git; `up` stages them again
  --changed[=<ref>]               Only files that differ from a branch or commit
                                  (default: the default branch)
  --parser <name>                 html, vue, mjml, handlebars, tempura, css, scss, less, json,
                                  jsonc, json5, json-stringify, yaml, toml, dockerfile, ini,
                                  gitconfig, graphql, babel, typescript, babel-ts, xml, or
                                  markdown (default: by file name)
  --stdin-filepath <path>         The path of the text on stdin, for its language and its configs
  --threads <n>                   How many files to format at once (default: all cores)

Config
  --config <path>                 Use this config for every file instead of the nearest one
  --no-config                     Read no config files and no .editorconfig

Style
  --width <n>                     Line width (default: 80, and 120 for Dockerfiles)
  --indent <n>                    Indent width (default: 2)
  --tabs                          Indent with tabs
  --eol <eol>                     lf (default), crlf, or cr
  --whitespace <how>              strict, auto, or ignore (default: auto, and strict for XML)
  --quote <quote>                 double (default) or single, in Handlebars, style sheets, JSON5,
                                  YAML and JS
  --no-trailing-comma             No trailing commas in SCSS, JSONC, JSON5, YAML, TOML and JS
  --no-bracket-spacing            No spaces inside the braces of JSON and JS objects and YAML
                                  flow mappings
  --single-attribute-per-line     Put each attribute on its own line
  --bracket-same-line             Keep the > of a multi-line tag on the last line
  --ignore <name>                 The ignore directive (default: prettier-ignore)
  --no-embedded                   Leave the code of other languages in a file as it is written

Other
  -v, --version                   Print the version
  -h, --help                      Print this help

Examples
  $ tidy index.html
  $ tidy check src
  $ tidy up . --width 100
  $ tidy up --staged
  $ tidy check --changed=main
";

fn main() -> ExitCode {
  match run() {
    Ok(code) => code,
    Err(error) => {
      eprintln!("tidy: {error}");
      ExitCode::from(2)
    }
  }
}

/// What to do with the files: print them, rewrite them, or just name the ones that need it.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
  Print,
  Up,
  Check,
}

/// Which files to take from git, instead of walking the paths given.
enum Scope {
  All,
  /// Staged in the index. `up` stages what it rewrote.
  Staged,
  /// Different from a branch or commit; `None` is the default branch.
  Changed(Option<String>),
}

fn run() -> Result<ExitCode, String> {
  let mut raw: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();

  if raw.first().and_then(|arg| arg.to_str()) == Some("schema") {
    let text = tidy::schema::schema();
    // A closed pipe, as in `tidy schema | head`, is not an error worth reporting.
    match std::io::stdout().write_all(text.as_bytes()) {
      Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
      other => other.map_err(|error| error.to_string())?,
    }
    return Ok(ExitCode::SUCCESS);
  }

  let mode = match raw.first().and_then(|arg| arg.to_str()) {
    Some("up") => Mode::Up,
    Some("check") => Mode::Check,
    _ => Mode::Print,
  };
  if mode != Mode::Print {
    raw.remove(0);
  }

  // `--changed` takes its value with `=`, because `--changed main` cannot be told
  // apart from `--changed` followed by the path `main`.
  let (mut staged, mut changed, mut rest) = (false, None, Vec::with_capacity(raw.len()));
  for arg in raw {
    match arg.to_str() {
      Some("--staged") => staged = true,
      Some("--changed") => changed = Some(None),
      Some(text) if text.starts_with("--changed=") => {
        changed = Some(Some(text["--changed=".len()..].to_owned()));
      }
      _ => rest.push(arg),
    }
  }
  let scope = match (staged, changed) {
    (true, Some(_)) => return Err("--staged and --changed can't be used together".into()),
    (true, None) => Scope::Staged,
    (false, Some(base)) => Scope::Changed(base),
    (false, None) => Scope::All,
  };

  let mut args = pico_args::Arguments::from_vec(rest);
  if args.contains(["-h", "--help"]) {
    print!("{HELP}");
    return Ok(ExitCode::SUCCESS);
  }
  if args.contains(["-v", "--version"]) {
    println!("tidy {}", env!("CARGO_PKG_VERSION"));
    return Ok(ExitCode::SUCCESS);
  }
  let error = |error: pico_args::Error| error.to_string();
  let (check, write) = (mode == Mode::Check, mode == Mode::Up);
  let threads: Option<usize> = args.opt_value_from_str("--threads").map_err(error)?;
  let mut flags = Config::default();
  let parser: Option<String> = args.opt_value_from_str("--parser").map_err(error)?;
  let language = match parser.as_deref() {
    None => None,
    Some(name) => {
      let (language, dialect) = match name {
        "html" => (Language::Html, None),
        "vue" => (Language::Vue, None),
        "mjml" => (Language::Mjml, None),
        "handlebars" => (Language::Handlebars, None),
        "tempura" => (Language::Handlebars, Some(handlebars::Dialect::Tempura)),
        "css" => (Language::Css(css::Dialect::Css), None),
        "scss" => (Language::Css(css::Dialect::Scss), None),
        "less" => (Language::Css(css::Dialect::Less), None),
        "json" => (Language::Json(json::Dialect::Json), None),
        "jsonc" => (Language::Json(json::Dialect::Jsonc), None),
        "json5" => (Language::Json(json::Dialect::Json5), None),
        "json-stringify" => (Language::Json(json::Dialect::Stringify), None),
        "toml" => (Language::Toml, None),
        "dockerfile" => (Language::Dockerfile, None),
        "ini" => (Language::Ini(ini::Dialect::Ini), None),
        "gitconfig" => (Language::Ini(ini::Dialect::Git), None),
        "graphql" => (Language::Graphql, None),
        "babel" => (Language::Js(js::Dialect::Js), None),
        "typescript" | "babel-ts" => (Language::Js(js::Dialect::Ts), None),
        "yaml" => (Language::Yaml, None),
        "xml" => (Language::Xml, None),
        "markdown" => (Language::Markdown, None),
        other => return Err(format!("unknown parser `{other}`")),
      };
      flags.handlebars.dialect = dialect;
      Some(language)
    }
  };
  let stdin: Option<PathBuf> = args.opt_value_from_str("--stdin-filepath").map_err(error)?;
  let fixed: Option<PathBuf> = args.opt_value_from_str("--config").map_err(error)?;
  let no_config = args.contains("--no-config");
  flags.shared = shared(&mut args)?;
  let mut paths = Vec::new();
  for arg in args.finish() {
    let text = arg.to_string_lossy();
    if text.starts_with('-') && text != "-" {
      return Err(format!("unknown flag `{text}`, see --help"));
    }
    paths.push(PathBuf::from(arg));
  }
  let resolver = match (no_config, fixed) {
    (true, _) => None,
    (false, Some(path)) => Some(Resolver::fixed(Config::read(&path)?)),
    (false, None) => Some(Resolver::default()),
  };
  let sources = Sources { resolver, flags };

  // git names the files instead of a walk, filtered by the paths given, if any.
  let listed = match &scope {
    Scope::All => None,
    _ if mode == Mode::Print => {
      return Err("--staged and --changed need `tidy up` or `tidy check`".into());
    }
    Scope::Staged => {
      Some(git_listed(&[&["diff", "--name-only", "--cached", "--diff-filter=ACMR"]])?)
    }
    Scope::Changed(base) => {
      let base = match base {
        Some(base) => base.clone(),
        None => default_branch()?,
      };
      let from =
        git(&["merge-base", &base, "HEAD"]).ok().map(|text| text.trim().to_owned()).unwrap_or(base);
      // A file that is new and not yet added still differs from the branch.
      Some(git_listed(&[
        &["diff", "--name-only", "--diff-filter=ACMR", &from],
        &["ls-files", "--others", "--exclude-standard"],
      ])?)
    }
  };

  let paths = match listed {
    None => paths,
    Some(listed) => {
      let under = |path: &Path| paths.is_empty() || paths.iter().any(|root| path.starts_with(root));
      let wanted = listed.into_iter().filter(|path| under(path)).collect::<Vec<_>>();
      // Nothing to do is not a failure: a commit that touches no formattable file is fine.
      if wanted.is_empty() {
        return Ok(ExitCode::SUCCESS);
      }
      wanted
    }
  };

  // `tidy` alone reads stdin; `up` and `check` alone take the current folder.
  let paths = match paths.is_empty() && mode != Mode::Print && stdin.is_none() {
    true => vec![PathBuf::from(".")],
    false => paths,
  };

  if paths.is_empty() || paths == [Path::new("-")] {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).map_err(|error| error.to_string())?;
    let language = language
      .or_else(|| stdin.as_deref().and_then(|path| Language::of_path(&path.to_string_lossy())))
      .unwrap_or(Language::Html);
    let options = sources.options(stdin.as_deref())?;
    let path = stdin.as_deref().map(|path| path.to_string_lossy());
    let mut formatter = Formatter::default();
    let output = formatter
      .format(&source, language, &options, path.as_deref(), &mut Verbatim)
      .map_err(|error| describe(&error, &source, path.as_deref().unwrap_or("stdin")))?;
    if check {
      return Ok(if output == source { ExitCode::SUCCESS } else { ExitCode::FAILURE });
    }
    std::io::stdout().write_all(output.as_bytes()).map_err(|error| error.to_string())?;
    return Ok(ExitCode::SUCCESS);
  }

  if !check && !write && paths.len() == 1 && paths[0].is_file() {
    let path = &paths[0];
    let source =
      std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", shown(path)))?;
    let mut formatter = Formatter::default();
    let output = sources.format(&mut formatter, &source, language, path)?;
    std::io::stdout().write_all(output.as_bytes()).map_err(|error| error.to_string())?;
    return Ok(ExitCode::SUCCESS);
  }
  if !check && !write {
    return Err("use `tidy up` or `tidy check` to format more than one file".into());
  }

  let mut options = tidy_walk::Options { wanted: &wanted, ..tidy_walk::Options::default() };
  match threads.map(|threads| threads.max(1)) {
    Some(1) => (options.io, options.workers) = (1, 0),
    Some(threads) if options.workers == 0 => options.io = threads,
    Some(threads) => (options.io, options.workers) = (options.io.min(threads), threads),
    None => {}
  }
  let listing = |dir: &Path, entries: &[tidy_walk::Entry]| {
    if let Some(resolver) = &sources.resolver {
      let configs = entries.iter().filter(|entry| entry.kind == tidy_walk::Kind::File);
      resolver.listing(dir, configs.map(|entry| entry.name));
    }
  };
  let report = tidy_walk::run(&paths, &options, listing, Formatter::default, |formatter, file| {
    let source = match std::str::from_utf8(&file.bytes) {
      Ok(source) => source,
      Err(error) => {
        return Some(Output { value: Err(format!("{}: {error}", shown(&file.path))), write: None });
      }
    };
    match sources.format(formatter, source, language, &file.path) {
      Ok(output) if output == source => None,
      Ok(output) => {
        Some(Output { value: Ok(()), write: write.then(|| output.as_bytes().to_vec()) })
      }
      Err(error) => Some(Output { value: Err(error), write: None }),
    }
  });

  let (mut listed, mut errors) = (String::new(), String::new());
  for (path, outcome) in &report.results {
    match outcome {
      Ok(()) => listed.push_str(&format!("{}\n", shown(path))),
      Err(error) => errors.push_str(&format!("tidy: {error}\n")),
    }
  }
  for (path, error) in &report.errors {
    errors.push_str(&format!("tidy: {}: {error}\n", shown(path)));
  }
  std::io::stdout().write_all(listed.as_bytes()).map_err(|error| error.to_string())?;
  std::io::stderr().write_all(errors.as_bytes()).map_err(|error| error.to_string())?;
  if !errors.is_empty() {
    return Ok(ExitCode::from(2));
  }

  // What was staged is what should be committed, so stage the rewrite too.
  if write && matches!(scope, Scope::Staged) {
    let rewritten: Vec<&Path> = report
      .results
      .iter()
      .filter(|(_, outcome)| outcome.is_ok())
      .map(|(path, _)| path.as_path())
      .collect();
    if !rewritten.is_empty() {
      let mut command = std::process::Command::new("git");
      command.arg("add").arg("--").args(&rewritten);
      let status = command.status().map_err(|error| format!("can't run git: {error}"))?;
      if !status.success() {
        return Err("git add failed for the files tidy rewrote".into());
      }
    }
  }
  Ok(if check && !report.results.is_empty() { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

/// Runs git in the current folder and returns its stdout.
fn git(args: &[&str]) -> Result<String, String> {
  let output = std::process::Command::new("git")
    .args(args)
    .output()
    .map_err(|error| format!("can't run git: {error}"))?;
  if !output.status.success() {
    let message = String::from_utf8_lossy(&output.stderr);
    return Err(format!("git {}: {}", args[0], message.trim()));
  }
  String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

/// The files that git names, in a language tidy formats and still on disk.
fn git_listed(lists: &[&[&str]]) -> Result<Vec<PathBuf>, String> {
  let root = PathBuf::from(git(&["rev-parse", "--show-toplevel"])?.trim());
  let here = std::env::current_dir().unwrap_or_default();
  let mut paths = Vec::new();
  for args in lists {
    for line in git(args)?.lines() {
      if line.is_empty() || Language::of_path(line).is_none() {
        continue;
      }
      // git names a path from the root of the repository, wherever tidy was run.
      let path = root.join(line);
      if !path.is_file() {
        continue;
      }
      // Print it the way it was typed, when it is below the current folder.
      let path = path.strip_prefix(&here).map(Path::to_path_buf).unwrap_or(path);
      if !paths.contains(&path) {
        paths.push(path);
      }
    }
  }
  Ok(paths)
}

/// The branch that `--changed` compares against when none is named.
fn default_branch() -> Result<String, String> {
  if let Ok(head) = git(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) {
    let head = head.trim();
    if !head.is_empty() {
      return Ok(head.to_owned());
    }
  }
  for name in ["main", "master"] {
    if git(&["rev-parse", "--verify", "--quiet", name]).is_ok() {
      return Ok(name.to_owned());
    }
  }
  Err("can't tell which branch is the default one, pass --changed=<ref>".into())
}

/// A path as tidy prints it: with `/` between its parts on every system, as Prettier and git print it.
fn shown(path: &Path) -> String {
  let text = path.to_string_lossy();
  match std::path::MAIN_SEPARATOR {
    '/' => text.into_owned(),
    separator => text.replace(separator, "/"),
  }
}

/// Whether to format a file found in a folder: whether its language is known.
fn wanted(_: &Path, name: &std::ffi::OsStr) -> bool {
  Language::of_path(&name.to_string_lossy()).is_some()
}

/// The shared choices that the flags make.
fn shared(args: &mut pico_args::Arguments) -> Result<Shared, String> {
  let error = |error: pico_args::Error| error.to_string();
  let mut choice = |name: &'static str, options: &[&str]| -> Result<Option<String>, String> {
    let value: Option<String> = args.opt_value_from_str(name).map_err(error)?;
    match value {
      Some(value) if !options.contains(&value.as_str()) => {
        Err(format!("unknown {name} `{value}`, expected one of: {}", options.join(", ")))
      }
      value => Ok(value),
    }
  };
  let eol = choice("--eol", &["lf", "crlf", "cr"])?;
  let whitespace = choice("--whitespace", &["strict", "auto", "ignore"])?;
  let quote = choice("--quote", &["double", "single"])?;
  let flag =
    |args: &mut pico_args::Arguments, name: &'static str| args.contains(name).then_some(true);
  Ok(Shared {
    width: args.opt_value_from_str("--width").map_err(error)?,
    indent: args.opt_value_from_str("--indent").map_err(error)?,
    tabs: flag(args, "--tabs"),
    eol: eol.map(|eol| match eol.as_str() {
      "crlf" => doc::Eol::Crlf,
      "cr" => doc::Eol::Cr,
      _ => doc::Eol::Lf,
    }),
    whitespace: whitespace.map(|how| match how.as_str() {
      "strict" => Whitespace::Strict,
      "ignore" => Whitespace::Ignore,
      _ => Whitespace::Auto,
    }),
    single_quote: quote.map(|quote| quote == "single"),
    trailing_comma: args.contains("--no-trailing-comma").then_some(false),
    bracket_spacing: args.contains("--no-bracket-spacing").then_some(false),
    bracket_same_line: flag(args, "--bracket-same-line"),
    single_attribute_per_line: flag(args, "--single-attribute-per-line"),
    ignore: args.opt_value_from_str("--ignore").map_err(error)?,
    format_embedded: args.contains("--no-embedded").then_some(false),
  })
}

/// Where the options of each file come from: Prettier's, then the config files around it, then the flags.
struct Sources {
  /// `None` with `--no-config`.
  resolver: Option<Resolver>,
  flags: Config,
}

impl Sources {
  fn options(&self, path: Option<&Path>) -> Result<Options, String> {
    let mut options = match (&self.resolver, path) {
      (Some(resolver), Some(path)) => resolver.options(path)?,
      _ => Options::default(),
    };
    self.flags.apply(&mut options);
    Ok(options)
  }

  /// Formats the file at `path`, in `language` or the language of its name.
  fn format<'f>(
    &self,
    formatter: &'f mut Formatter,
    source: &str,
    language: Option<Language>,
    path: &Path,
  ) -> Result<&'f str, String> {
    let name = path.to_string_lossy();
    let language = language
      .or_else(|| Language::of_path(&name))
      .ok_or_else(|| format!("{name}: can't tell the language of this file, pass --parser"))?;
    let options = self.options(Some(path)).map_err(|error| format!("{name}: {error}"))?;
    formatter
      .format(source, language, &options, Some(&name), &mut Verbatim)
      .map_err(|error| describe(&error, source, &name))
  }
}

/// An error, with the line and column of a syntax error.
fn describe(error: &Error, source: &str, name: &str) -> String {
  match error {
    Error::Syntax { message, at } => {
      let before = &source[..(*at).min(source.len())];
      let line = before.matches('\n').count() + 1;
      let column = before.rsplit('\n').next().unwrap_or_default().chars().count() + 1;
      format!("{name}:{line}:{column}: {message}")
    }
    Error::TooDeep => format!("{name}: {error}"),
  }
}
