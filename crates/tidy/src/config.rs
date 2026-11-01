//! Options from files: the nearest `tidy.json`, `tidy.jsonc` or `tidy.toml`, and `.editorconfig`.
//!
//! A config sets shared choices at the top, and each language's own in its section, where the shared ones can be set
//! again for that language only:
//!
//! ```toml
//! width = 100
//! ignore = "tidy-ignore"
//!
//! [xml]
//! whitespace = "ignore"
//! sortAttributes = true
//! ```
//!
//! `.editorconfig` sets `indent_style`, `indent_size`, `tab_width`, `max_line_length` and `end_of_line`, as Prettier
//! reads them. A tidy config overrides it.

use crate::options::{Options, Shared};
use crate::{Whitespace, doc, handlebars, xml};
use rustc_hash::FxHashMap;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// The names of a config file, in the order they are looked for.
pub const NAMES: [&str; 3] = ["tidy.json", "tidy.jsonc", "tidy.toml"];

/// The options of a config file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
  pub shared: Shared,
  pub html: Shared,
  pub vue: Vue,
  pub handlebars: Handlebars,
  pub css: Css,
  pub xml: Xml,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vue {
  pub shared: Shared,
  /// `indentBlocks`: Prettier's `vueIndentScriptAndStyle`.
  pub indent_blocks: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Handlebars {
  pub shared: Shared,
  pub dialect: Option<handlebars::Dialect>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Css {
  pub shared: Shared,
  /// `trailingComma`: SCSS maps and lists that break get a trailing comma.
  pub trailing_comma: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Xml {
  pub shared: Shared,
  /// `selfClosingSpace`
  pub self_closing_space: Option<bool>,
  /// `sortAttributes`
  pub sort_attributes: Option<bool>,
  /// `attributeQuotes`
  pub quotes: Option<xml::Quotes>,
}

impl Config {
  /// Reads the config file at `path`, TOML if its name ends with `.toml`, and JSON otherwise.
  pub fn read(path: &Path) -> Result<Config, String> {
    let text =
      std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let config = match path.extension().is_some_and(|extension| extension == "toml") {
      true => Config::toml(&text),
      false => Config::json(&text),
    };
    config.map_err(|error| format!("{}: {error}", path.display()))
  }

  /// Reads a config from the text of a JSON file, which may have comments and trailing commas.
  pub fn json(text: &str) -> Result<Config, String> {
    let options = jsonc_parser::ParseOptions::default();
    let value: Option<Value> =
      jsonc_parser::parse_to_serde_value(text, &options).map_err(|error| error.to_string())?;
    Config::from_value(&value.unwrap_or_default())
  }

  /// Reads a config from the text of a TOML file.
  pub fn toml(text: &str) -> Result<Config, String> {
    let table: toml::Table = toml::from_str(text).map_err(|error| error.message().to_owned())?;
    let value = serde_json::to_value(table).map_err(|error| error.to_string())?;
    Config::from_value(&value)
  }

  /// Reads a config from a JSON value, as the Node binding receives its options.
  pub fn from_value(value: &Value) -> Result<Config, String> {
    let Some(map) = value.as_object() else { return Err("the config must be an object".into()) };
    let mut config = Config::default();
    for (key, value) in map {
      let section =
        |name: &str| value.as_object().ok_or_else(|| format!("`{name}` must be an object"));
      match key.as_str() {
        "html" => config.html = shared_only(section("html")?, "html")?,
        "vue" => config.vue = vue(section("vue")?)?,
        "handlebars" => config.handlebars = handlebars(section("handlebars")?)?,
        "css" => config.css = css(section("css")?)?,
        "xml" => config.xml = xml(section("xml")?)?,
        _ if shared(&mut config.shared, key, value, "")? => {}
        _ => return Err(format!("unknown option `{key}`")),
      }
    }
    Ok(config)
  }

  /// Applies the config to `options`: the shared choices to every language, and then each language's own.
  pub fn apply(&self, options: &mut Options) {
    options.share(&self.shared);
    self.html.html(&mut options.html);
    self.vue.shared.html(&mut options.vue);
    if let Some(indent) = self.vue.indent_blocks {
      options.vue.vue_indent_script_and_style = indent;
    }
    self.handlebars.shared.handlebars(&mut options.handlebars);
    if let Some(dialect) = self.handlebars.dialect {
      options.handlebars.dialect = dialect;
    }
    self.css.shared.css(&mut options.css);
    if let Some(comma) = self.css.trailing_comma {
      options.css.trailing_comma = comma;
    }
    self.xml.shared.xml(&mut options.xml);
    if let Some(space) = self.xml.self_closing_space {
      options.xml.self_closing_space = space;
    }
    if let Some(sort) = self.xml.sort_attributes {
      options.xml.sort_attributes = sort;
    }
    if let Some(quotes) = self.xml.quotes {
      options.xml.quotes = quotes;
    }
  }
}

/// Reads a shared option into `shared`. Returns false if `key` isn't one.
fn shared(shared: &mut Shared, key: &str, value: &Value, section: &str) -> Result<bool, String> {
  let name = match section {
    "" => key.to_owned(),
    section => format!("{section}.{key}"),
  };
  match key {
    "width" => shared.width = Some(number(value, &name)?),
    "indent" => shared.indent = Some(number(value, &name)?),
    "tabs" => shared.tabs = Some(flag(value, &name)?),
    "eol" => {
      shared.eol = Some(match text(value, &name)? {
        "lf" => doc::Eol::Lf,
        "crlf" => doc::Eol::Crlf,
        "cr" => doc::Eol::Cr,
        _ => return Err(format!("`{name}` must be \"lf\", \"crlf\" or \"cr\"")),
      })
    }
    "whitespace" => {
      shared.whitespace = Some(match text(value, &name)? {
        "strict" => Whitespace::Strict,
        "auto" => Whitespace::Auto,
        "ignore" => Whitespace::Ignore,
        _ => return Err(format!("`{name}` must be \"strict\", \"auto\" or \"ignore\"")),
      })
    }
    "quote" => {
      shared.single_quote = Some(match text(value, &name)? {
        "double" => false,
        "single" => true,
        _ => return Err(format!("`{name}` must be \"double\" or \"single\"")),
      })
    }
    "bracketSameLine" => shared.bracket_same_line = Some(flag(value, &name)?),
    "singleAttributePerLine" => shared.single_attribute_per_line = Some(flag(value, &name)?),
    "ignore" => match text(value, &name)? {
      "" => return Err(format!("`{name}` must not be empty")),
      ignore => shared.ignore = Some(ignore.to_owned()),
    },
    _ => return Ok(false),
  }
  Ok(true)
}

fn shared_only(map: &Map<String, Value>, section: &str) -> Result<Shared, String> {
  let mut options = Shared::default();
  for (key, value) in map {
    if !shared(&mut options, key, value, section)? {
      return Err(format!("unknown option `{section}.{key}`"));
    }
  }
  Ok(options)
}

fn vue(map: &Map<String, Value>) -> Result<Vue, String> {
  let mut vue = Vue::default();
  for (key, value) in map {
    match key.as_str() {
      "indentBlocks" => vue.indent_blocks = Some(flag(value, "vue.indentBlocks")?),
      _ if shared(&mut vue.shared, key, value, "vue")? => {}
      _ => return Err(format!("unknown option `vue.{key}`")),
    }
  }
  Ok(vue)
}

fn handlebars(map: &Map<String, Value>) -> Result<Handlebars, String> {
  let mut options = Handlebars::default();
  for (key, value) in map {
    match key.as_str() {
      "dialect" => {
        options.dialect = Some(match text(value, "handlebars.dialect")? {
          "auto" => handlebars::Dialect::Auto,
          "handlebars" => handlebars::Dialect::Handlebars,
          "tempura" => handlebars::Dialect::Tempura,
          _ => {
            return Err(
              "`handlebars.dialect` must be \"auto\", \"handlebars\" or \"tempura\"".into(),
            );
          }
        })
      }
      _ if shared(&mut options.shared, key, value, "handlebars")? => {}
      _ => return Err(format!("unknown option `handlebars.{key}`")),
    }
  }
  Ok(options)
}

fn css(map: &Map<String, Value>) -> Result<Css, String> {
  let mut css = Css::default();
  for (key, value) in map {
    match key.as_str() {
      "trailingComma" => css.trailing_comma = Some(flag(value, "css.trailingComma")?),
      _ if shared(&mut css.shared, key, value, "css")? => {}
      _ => return Err(format!("unknown option `css.{key}`")),
    }
  }
  Ok(css)
}

fn xml(map: &Map<String, Value>) -> Result<Xml, String> {
  let mut options = Xml::default();
  for (key, value) in map {
    match key.as_str() {
      "selfClosingSpace" => options.self_closing_space = Some(flag(value, "xml.selfClosingSpace")?),
      "sortAttributes" => options.sort_attributes = Some(flag(value, "xml.sortAttributes")?),
      "attributeQuotes" => {
        options.quotes = Some(match text(value, "xml.attributeQuotes")? {
          "preserve" => xml::Quotes::Preserve,
          "single" => xml::Quotes::Single,
          "double" => xml::Quotes::Double,
          _ => {
            return Err(
              "`xml.attributeQuotes` must be \"preserve\", \"single\" or \"double\"".into(),
            );
          }
        })
      }
      _ if shared(&mut options.shared, key, value, "xml")? => {}
      _ => return Err(format!("unknown option `xml.{key}`")),
    }
  }
  Ok(options)
}

fn number(value: &Value, name: &str) -> Result<usize, String> {
  value
    .as_u64()
    .map(|number| number as usize)
    .ok_or_else(|| format!("`{name}` must be a whole number"))
}

fn flag(value: &Value, name: &str) -> Result<bool, String> {
  value.as_bool().ok_or_else(|| format!("`{name}` must be true or false"))
}

fn text<'v>(value: &'v Value, name: &str) -> Result<&'v str, String> {
  value.as_str().ok_or_else(|| format!("`{name}` must be a string"))
}

// ───────────────────────────── files ─────────────────────────────

/// Finds the options of each file from the config files around it. It looks at each folder once, so one resolver
/// can serve a whole walk, from several threads.
pub struct Resolver {
  /// The config of every file, instead of the nearest one.
  fixed: Option<Arc<Config>>,
  /// The folder that relative paths are in.
  cwd: PathBuf,
  folders: Mutex<FxHashMap<OsString, Arc<Folder>>>,
  /// The config files that a walk found in each folder, so that the resolver doesn't look for them again.
  listings: Mutex<FxHashMap<OsString, Listing>>,
}

/// What a resolver knows about a folder.
#[derive(Default)]
struct Folder {
  /// The `.editorconfig` files that apply to the files in it, and the folder of each, nearest last.
  editors: Vec<(PathBuf, Arc<Editor>)>,
  /// Its nearest config.
  config: Option<Arc<Config>>,
}

/// The config files in a folder.
#[derive(Clone, Copy, Default)]
struct Listing {
  editorconfig: bool,
  /// A bit for each of [`NAMES`].
  configs: u8,
}

/// An `.editorconfig` file.
struct Editor {
  is_root: bool,
  sections: Vec<ec4rs::Section>,
}

impl Default for Resolver {
  fn default() -> Self {
    Resolver {
      fixed: None,
      cwd: std::env::current_dir().unwrap_or_default(),
      folders: Mutex::default(),
      listings: Mutex::default(),
    }
  }
}

impl Resolver {
  /// A resolver that gives every file the options of `config`, and of its `.editorconfig` files.
  pub fn fixed(config: Config) -> Resolver {
    Resolver { fixed: Some(Arc::new(config)), ..Resolver::default() }
  }

  /// Notes the names of the files in `dir`, as a walk lists them, so that the resolver doesn't look for config files
  /// there again.
  pub fn listing<'n>(&self, dir: &Path, names: impl IntoIterator<Item = &'n OsStr>) {
    let mut listing = Listing::default();
    for name in names {
      if name == ".editorconfig" {
        listing.editorconfig = true;
      } else if let Some(index) = NAMES.iter().position(|it| name == *it) {
        listing.configs |= 1 << index;
      }
    }
    let key = self.absolute(dir).into_os_string();
    self.listings.lock().unwrap_or_else(PoisonError::into_inner).insert(key, listing);
  }

  /// The options of the file at `path`: Prettier's, then `.editorconfig`, then the nearest tidy config.
  pub fn options(&self, path: &Path) -> Result<Options, String> {
    let path = self.absolute(path);
    let folder = match path.parent() {
      Some(dir) => self.folder(dir)?,
      None => Arc::default(),
    };
    let mut options = Options::default();
    options.share(&editor_settings(&properties(&folder.editors, &path)));
    if let Some(config) = self.fixed.as_ref().or(folder.config.as_ref()) {
      config.apply(&mut options);
    }
    Ok(options)
  }

  /// `path` from the root, without `.` in it.
  fn absolute(&self, path: &Path) -> PathBuf {
    match path.is_absolute() {
      true => path.components().collect(),
      false => self.cwd.join(path).components().collect(),
    }
  }

  fn folder(&self, dir: &Path) -> Result<Arc<Folder>, String> {
    if let Some(found) =
      self.folders.lock().unwrap_or_else(PoisonError::into_inner).get(dir.as_os_str())
    {
      return Ok(found.clone());
    }
    let parent = match dir.parent() {
      Some(parent) => self.folder(parent)?,
      None => Arc::default(),
    };
    let listing =
      self.listings.lock().unwrap_or_else(PoisonError::into_inner).get(dir.as_os_str()).copied();
    let listing = listing.unwrap_or_else(|| look(dir));
    let mut folder = Folder { editors: Vec::new(), config: parent.config.clone() };
    let editor = match listing.editorconfig {
      true => editor(&dir.join(".editorconfig"))?,
      false => None,
    };
    match editor {
      Some(editor) if editor.is_root => folder.editors.push((dir.to_owned(), editor)),
      Some(editor) => {
        folder.editors = parent.editors.clone();
        folder.editors.push((dir.to_owned(), editor));
      }
      None => folder.editors = parent.editors.clone(),
    }
    let mut names = NAMES.iter().enumerate().filter(|(index, _)| listing.configs & 1 << index != 0);
    if let Some((_, name)) = names.next() {
      if let Some((_, other)) = names.next() {
        return Err(format!(
          "{}: more than one config in this folder, with {other}",
          dir.join(name).display()
        ));
      }
      folder.config = Some(Arc::new(Config::read(&dir.join(name))?));
    }
    let folder = Arc::new(folder);
    self
      .folders
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .insert(dir.as_os_str().to_owned(), folder.clone());
    Ok(folder)
  }
}

/// The config files in `dir`, from the file system.
fn look(dir: &Path) -> Listing {
  let mut listing = Listing { editorconfig: dir.join(".editorconfig").is_file(), configs: 0 };
  for (index, name) in NAMES.iter().enumerate() {
    if dir.join(name).is_file() {
      listing.configs |= 1 << index;
    }
  }
  listing
}

fn editor(path: &Path) -> Result<Option<Arc<Editor>>, String> {
  match ec4rs::ConfigFile::open(path) {
    Err(ec4rs::ParseError::Io(_)) => Ok(None),
    Err(error) => Err(format!("{}: {error}", path.display())),
    Ok(mut file) => {
      let is_root = file.reader.is_root;
      let sections = file
        .by_ref()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", path.display()))?;
      Ok(Some(Arc::new(Editor { is_root, sections })))
    }
  }
}

/// The properties that the `.editorconfig` files set for the file at `path`, as lower case.
fn properties<'k>(editors: &[(PathBuf, Arc<Editor>)], path: &Path) -> HashMap<&'k str, String> {
  let mut properties = HashMap::new();
  for (dir, editor) in editors {
    let relative = path.strip_prefix(dir).unwrap_or(path);
    for section in editor.sections.iter().filter(|section| section.applies_to(relative)) {
      for key in ["indent_style", "indent_size", "tab_width", "max_line_length", "end_of_line"] {
        let value = section.props().get_raw_for_key(key).into_str();
        if !value.is_empty() {
          properties.insert(key, value.to_ascii_lowercase());
        }
      }
    }
  }
  properties
}

/// Prettier's `editorconfig-to-prettier`.
fn editor_settings(properties: &HashMap<&str, String>) -> Shared {
  let get = |key: &str| properties.get(key).map(String::as_str);
  let number = |key: &str| get(key).and_then(|value| value.parse::<usize>().ok());
  let mut shared = Shared::default();
  if let Some(style) = get("indent_style") {
    shared.tabs = Some(style == "tab");
  }
  if get("indent_size") == Some("tab") {
    shared.tabs = Some(true);
  }
  shared.indent = match shared.tabs {
    Some(true) => number("tab_width").or(number("indent_size")),
    _ => number("indent_size").or(number("tab_width")),
  };
  shared.width = match get("max_line_length") {
    Some("off") => Some(usize::MAX),
    _ => number("max_line_length"),
  };
  shared.eol = match get("end_of_line") {
    Some("lf") => Some(doc::Eol::Lf),
    Some("crlf") => Some(doc::Eol::Crlf),
    Some("cr") => Some(doc::Eol::Cr),
    _ => None,
  };
  shared
}
