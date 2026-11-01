use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tidy::config::{Config, Resolver};
use tidy::embed::Verbatim;
use tidy::{Error, Formatter, Language, Options, Shared, Whitespace, css, doc, handlebars, json};
use tidy_walk::Output;

const HELP: &str = "\
tidy: a fast formatter for markup, style sheets and data

Usage
  $ tidy [paths...] [options]

  Without paths, reads stdin and prints the result.
  With one file, prints the result unless --check or --write is set.
  Folders are searched for .html, .htm, .vue, .hbs, .handlebars, .css, .scss, .less, .md, and .markdown
  files, for JSON files: .json, .jsonc, .json5, package.json, and the others that Prettier formats, for .toml
  files, and for XML files: .xml, .svg, .plist, .csproj, pom.xml, and the others that @prettier/plugin-xml
  formats. Style sheets in <style> and style=\"\", JSON in scripts and code blocks, and TOML in code blocks and
  front matter are formatted. Other scripts and code blocks are left as written.

  Each file takes its options from the nearest tidy.json, tidy.jsonc or tidy.toml, and from .editorconfig.
  Options given here override them.

Options
  --check                         List files that aren't formatted, and exit 1 if any
  -w, --write                     Rewrite files in place
  --parser <name>                 html, vue, handlebars, tempura, css, scss, less, json, jsonc, json5,
                                  json-stringify, toml, xml, or markdown (default: by file name)
  --stdin-filepath <path>         The path of the text on stdin, for its language and its config files
  --config <path>                 Use this config for every file instead of the nearest one
  --no-config                     Read no config files and no .editorconfig
  --width <n>                     Line width (default: 80)
  --indent <n>                    Indent width (default: 2)
  --tabs                          Indent with tabs
  --eol <eol>                     lf (default), crlf, or cr
  --whitespace <how>              strict, auto, or ignore (default: auto, and strict for XML)
  --quote <quote>                 double (default) or single, in Handlebars, style sheets and JSON5
  --no-trailing-comma             No trailing commas in SCSS, JSONC, JSON5 and TOML
  --single-attribute-per-line     Put each attribute on its own line
  --bracket-same-line             Keep the > of a multi-line tag on the last line
  --ignore <name>                 The ignore directive (default: prettier-ignore)
  --threads <n>                   How many files to format at once (default: all cores); 1 uses one thread
  -v, --version                   Print the version
  -h, --help                      Print this help

Examples
  $ tidy index.html
  $ tidy src --check
  $ tidy . --write --width 100
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

fn run() -> Result<ExitCode, String> {
  let mut args = pico_args::Arguments::from_env();
  if args.contains(["-h", "--help"]) {
    print!("{HELP}");
    return Ok(ExitCode::SUCCESS);
  }
  if args.contains(["-v", "--version"]) {
    println!("tidy {}", env!("CARGO_PKG_VERSION"));
    return Ok(ExitCode::SUCCESS);
  }
  let error = |error: pico_args::Error| error.to_string();
  let check = args.contains("--check");
  let write = args.contains(["-w", "--write"]);
  let threads: Option<usize> = args.opt_value_from_str("--threads").map_err(error)?;
  let mut flags = Config::default();
  let parser: Option<String> = args.opt_value_from_str("--parser").map_err(error)?;
  let language = match parser.as_deref() {
    None => None,
    Some(name) => {
      let (language, dialect) = match name {
        "html" => (Language::Html, None),
        "vue" => (Language::Vue, None),
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
      std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut formatter = Formatter::default();
    let output = sources.format(&mut formatter, &source, language, path)?;
    std::io::stdout().write_all(output.as_bytes()).map_err(|error| error.to_string())?;
    return Ok(ExitCode::SUCCESS);
  }
  if !check && !write {
    return Err("pass --check or --write to format more than one file".into());
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
        return Some(Output {
          value: Err(format!("{}: {error}", file.path.display())),
          write: None,
        });
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
      Ok(()) => listed.push_str(&format!("{}\n", path.display())),
      Err(error) => errors.push_str(&format!("tidy: {error}\n")),
    }
  }
  for (path, error) in &report.errors {
    errors.push_str(&format!("tidy: {}: {error}\n", path.display()));
  }
  std::io::stdout().write_all(listed.as_bytes()).map_err(|error| error.to_string())?;
  std::io::stderr().write_all(errors.as_bytes()).map_err(|error| error.to_string())?;
  if !errors.is_empty() {
    return Ok(ExitCode::from(2));
  }
  Ok(if check && !report.results.is_empty() { ExitCode::FAILURE } else { ExitCode::SUCCESS })
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
    bracket_same_line: flag(args, "--bracket-same-line"),
    single_attribute_per_line: flag(args, "--single-attribute-per-line"),
    ignore: args.opt_value_from_str("--ignore").map_err(error)?,
    format_embedded: None,
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
