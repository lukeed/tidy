use napi::bindgen_prelude::{Function, JsObjectValue, Object, Unknown};
use napi::{Env, ValueType};
use napi_derive::napi;
use serde_json::{Map, Value};
use tidy::embed::Request;
use tidy::{Formatter, host};

/// Called with each piece of embedded code, as an `Embed.Request`; returns its formatted text, or `undefined`.
type Hook<'a> = Function<'a, Unknown<'a>, Option<String>>;

/// The options of a call: the language, the file, the hook, and the rest, which are a `tidy.json`'s.
#[derive(Default)]
struct Input<'a> {
  lang: Option<String>,
  filepath: Option<String>,
  embed: Option<Hook<'a>>,
  config: Map<String, Value>,
}

impl<'a> Input<'a> {
  fn read(env: &Env, options: Option<Object<'a>>) -> napi::Result<Input<'a>> {
    let mut input = Input::default();
    let Some(options) = options else { return Ok(input) };
    for key in Object::keys(&options)? {
      let value: Unknown = options.get_named_property(&key)?;
      let kind = value.get_type()?;
      if kind == ValueType::Undefined {
        continue;
      }
      let expect = |want: ValueType, what: &str| match kind == want {
        true => Ok(()),
        false => Err(napi::Error::from_reason(format!("`{key}` must be {what}"))),
      };
      match key.as_str() {
        "lang" => {
          expect(ValueType::String, "a string")?;
          input.lang = Some(options.get_named_property(&key)?);
        }
        "filepath" => {
          expect(ValueType::String, "a string")?;
          input.filepath = Some(options.get_named_property(&key)?);
        }
        "embed" => {
          expect(ValueType::Function, "a function")?;
          input.embed = Some(options.get_named_property(&key)?);
        }
        _ => {
          let value = env.from_js_value(value)?;
          input.config.insert(key, value);
        }
      }
    }
    Ok(input)
  }
}

/// Format a file. `options.embed` receives each piece of code in another language first.
#[napi(ts_args_type = "source: string, options?: Options")]
pub fn format(env: Env, source: String, options: Option<Object<'_>>) -> napi::Result<String> {
  let input = Input::read(&env, options)?;
  let fail = napi::Error::from_reason;
  let (language, dialect) =
    host::language(input.lang.as_deref(), input.filepath.as_deref()).map_err(fail)?;
  let options = host::options(input.config, dialect).map_err(fail)?;
  let mut failure = None;
  let mut hook = |request: Request<'_>| -> Option<String> {
    let embed = input.embed.as_ref()?;
    if failure.is_some() {
      return None;
    }
    let result = env.to_js_value(&host::request(&request)).and_then(|it| embed.call(it));
    result.unwrap_or_else(|error| {
      failure = Some(error);
      None
    })
  };
  let output = Formatter::default()
    .format(&source, language, &options, input.filepath.as_deref(), &mut hook)
    .map(str::to_owned)
    .map_err(|error| fail(host::describe(&error, &source)))?;
  match failure {
    Some(error) => Err(error),
    None => Ok(output),
  }
}

/// Whether a file is formatted already, with the same options as `format`.
#[napi(ts_args_type = "source: string, options?: Options")]
pub fn check(env: Env, source: String, options: Option<Object<'_>>) -> napi::Result<bool> {
  let formatted = format(env, source.clone(), options)?;
  Ok(formatted == source)
}
