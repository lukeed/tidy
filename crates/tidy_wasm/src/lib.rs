//! tidy as WebAssembly: the `tidyrs-wasm` package, for browsers, Workers and Deno, and for `tidyrs` where its native
//! addon doesn't load. `format` and `check` are those of `tidyrs`.

use js_sys::{Array, Function, JSON, Reflect};
use serde_json::{Map, Value};
use tidy::embed::Request;
use tidy::{Formatter, host};
use wasm_bindgen::prelude::*;

fn fail(message: impl AsRef<str>) -> JsValue {
  JsError::new(message.as_ref()).into()
}

/// A JS error for `problem`, with its `loc` where it has one, as Prettier's errors have.
fn throw(problem: host::Problem) -> JsValue {
  let error = js_sys::Error::new(&problem.message);
  if let Some(loc) = problem.loc().and_then(|it| JSON::parse(&it.to_string()).ok()) {
    let _ = Reflect::set(&error, &"loc".into(), &loc);
  }
  error.into()
}

/// The options of a call: the language, the file, the hook, and the rest, which are a `tidy.json`'s.
struct Input {
  lang: Option<String>,
  filepath: Option<String>,
  embed: Option<Function>,
  config: Map<String, Value>,
}

impl Input {
  /// The options as `tidyrs` reads them: `lang` and `filepath` are strings, `embed` a function, and the rest goes to
  /// the config, where `undefined` is no value.
  fn read(options: &JsValue) -> Result<Input, JsValue> {
    let mut input = Input { lang: None, filepath: None, embed: None, config: Map::new() };
    if options.is_undefined() || options.is_null() {
      return Ok(input);
    }
    if !options.is_object() || options.is_function() || Array::is_array(options) {
      return Err(fail("`options` must be an object"));
    }
    let embed = Reflect::get(options, &JsValue::from_str("embed"))?;
    if !embed.is_undefined() {
      let embed = embed.dyn_into::<Function>().map_err(|_| fail("`embed` must be a function"))?;
      input.embed = Some(embed);
    }
    // JSON.stringify leaves out `undefined` and functions, as `tidyrs` skips them.
    let text = JSON::stringify(options)?.as_string().unwrap_or_default();
    let Ok(Value::Object(mut config)) = serde_json::from_str(&text) else {
      return Err(fail("`options` must be an object"));
    };
    for (key, slot) in [("lang", &mut input.lang), ("filepath", &mut input.filepath)] {
      match config.remove(key) {
        None => {}
        Some(Value::String(text)) => *slot = Some(text),
        Some(_) => return Err(fail(format!("`{key}` must be a string"))),
      }
    }
    input.config = config;
    Ok(input)
  }
}

/// Format a file. `options.embed` receives each piece of code in another language first.
#[wasm_bindgen(skip_typescript)]
pub fn format(source: &str, options: JsValue) -> Result<String, JsValue> {
  let input = Input::read(&options)?;
  let (language, dialect) =
    host::language(input.lang.as_deref(), input.filepath.as_deref()).map_err(fail)?;
  let options = host::options(input.config, dialect).map_err(fail)?;
  let mut failure = None;
  let mut hook = |request: Request<'_>| -> Option<String> {
    let embed = input.embed.as_ref()?;
    if failure.is_some() {
      return None;
    }
    let answer = JSON::parse(&host::request(&request).to_string())
      .and_then(|request| embed.call1(&JsValue::UNDEFINED, &request));
    match answer {
      Ok(value) if value.is_undefined() || value.is_null() => None,
      Ok(value) => value.as_string().or_else(|| {
        failure = Some(fail("`embed` must return a string or undefined"));
        None
      }),
      Err(error) => {
        failure = Some(error);
        None
      }
    }
  };
  let output = Formatter::default()
    .format(source, language, &options, input.filepath.as_deref(), &mut hook)
    .map(str::to_owned)
    .map_err(|error| throw(host::Problem::of(&error, source)));
  match failure {
    Some(error) => Err(error),
    None => output,
  }
}

/// Whether a file is formatted already, with the same options as `format`.
#[wasm_bindgen(skip_typescript)]
pub fn check(source: &str, options: JsValue) -> Result<bool, JsValue> {
  Ok(format(source, options)? == source)
}

/// The options in the text of a `tidy.json`, `tidy.jsonc` or `tidy.toml`, to pass to `format`.
#[wasm_bindgen(skip_typescript)]
pub fn config(text: &str) -> Result<JsValue, JsValue> {
  let value = host::config(text).map_err(throw)?;
  JSON::parse(&value.to_string())
}

/// The version of tidy this build came from.
#[wasm_bindgen(skip_typescript)]
pub fn version() -> String {
  env!("CARGO_PKG_VERSION").to_owned()
}
