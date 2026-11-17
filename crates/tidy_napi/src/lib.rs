use napi::bindgen_prelude::{Function, JsObjectValue, Object, Unknown};
use napi::{Env, ValueType};
use napi_derive::napi;
use serde_json::{Map, Value, json};
use tidy::config::Config;
use tidy::embed::{Kind, Request};
use tidy::{Formatter, Language, Options};

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
  let error = |error: &dyn std::fmt::Display| napi::Error::from_reason(error.to_string());
  let (language, dialect) = language(&input)?;
  let mut config = Config::from_value(&Value::Object(input.config)).map_err(|it| error(&it))?;
  if dialect.is_some() {
    config.handlebars.dialect = dialect;
  }
  let mut options = Options::default();
  config.apply(&mut options);
  let mut failure = None;
  let mut hook = |request: Request<'_>| -> Option<String> {
    let embed = input.embed.as_ref()?;
    if failure.is_some() {
      return None;
    }
    let result = env.to_js_value(&request_json(&request)).and_then(|it| embed.call(it));
    result.unwrap_or_else(|error| {
      failure = Some(error);
      None
    })
  };
  let output = Formatter::default()
    .format(&source, language, &options, input.filepath.as_deref(), &mut hook)
    .map(str::to_owned)
    .map_err(|it| error(&it))?;
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

/// The request as the hook gets it, an `Embed.Request`, without `host` where there is none.
fn request_json(request: &Request<'_>) -> Value {
  let kind = match request.kind {
    Kind::Block => "block",
    Kind::Expression => "expression",
    Kind::Event => "event",
    Kind::Bindings => "bindings",
    Kind::For => "for",
    Kind::Generic => "generic",
    Kind::Directive => "directive",
    Kind::FrontMatter => "frontmatter",
  };
  let mut value = json!({
    "code": request.code,
    "lang": request.lang,
    "kind": kind,
    "attribute": request.attribute,
    "indent": request.indent,
    "prefix": request.prefix,
    "suffix": request.suffix,
    "width": request.width,
    "module": request.module,
  });
  if let (Some(host), Value::Object(map)) = (request.host, &mut value) {
    map.insert("host".into(), host.into());
  }
  value
}

/// The language of `lang`, or of `filepath`, and the Handlebars dialect that `lang` asks for, as the command line
/// reads `--lang`.
fn language(input: &Input<'_>) -> napi::Result<(Language, Option<tidy::handlebars::Dialect>)> {
  let fail = |message: String| napi::Error::from_reason(message);
  match (input.lang.as_deref(), input.filepath.as_deref()) {
    // A dialect of Handlebars, which tidy otherwise tells from the template.
    (Some("tempura"), _) => Ok((Language::Handlebars, Some(tidy::handlebars::Dialect::Tempura))),
    (Some(name), _) => Language::hint(name)
      .map(|it| (it, None))
      .ok_or_else(|| fail(format!("unknown language: {name}"))),
    (None, Some(path)) => Language::of_path(path)
      .map(|it| (it, None))
      .ok_or_else(|| fail(format!("{path}: can't tell the language of this file, pass `lang`"))),
    (None, None) => Err(fail("can't tell the language: pass `lang` or `filepath`".into())),
  }
}
