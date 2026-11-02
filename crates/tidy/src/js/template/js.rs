//! JavaScript and TypeScript in markup and Markdown: Prettier's `textToDoc` for the parsers of `language-js`.
//!
//! bun writes the code into the document of the HTML around it. tidy's markup printers take text from
//! [`crate::embed`] instead, so the code is printed alone, inside as many indents as it starts at and between the
//! widths of the text around it on its lines, the way a hook prints it. A `v-for` value is printed whole, since its
//! aliases break by the width of what follows them.

use crate::embed::{Host, Kind, Request};
use crate::js::ast_nodes::node_as_ast_nodes;
use crate::js::base::*;
use crate::js::context::JsFormatContext;
use crate::js::ir::run::{self, Scratch};
use crate::js::options::{HtmlRoot, InHtml};
use crate::js::print::program::{FormatStatements, write_hashbang};
use crate::js::{format_args, sort_imports, strings, write};
use std::cell::{Cell, RefCell};

/// Prettier's parsers for programs.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Syntax {
  /// `babel`: JavaScript with JSX.
  Babel,
  /// `babel-ts`: TypeScript, with JSX if it parses that way.
  BabelTs,
  /// `typescript`, for a file whose name doesn't tell whether it has JSX.
  TypeScript,
}

/// `sourceType`
#[derive(Copy, Clone, PartialEq, Eq)]
enum SourceType {
  Script,
  Module,
  /// Try a module first, then a script.
  Unknown,
}

/// The name of a file whose text is `(`, an expression and `)`. See `needs_parentheses`.
const EXPRESSION_JSX: &[u8] = b"\0.jsx";
const EXPRESSION_TS: &[u8] = b"\0.ts";

/// The part that `printHtmlBinding` prints of a program that's a single declaration.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Binding {
  /// `__isVueBindings`: the parameters of `function _(..) {}`.
  Parameters,
  /// `__isVueForBindingLeft`: the same, in parentheses unless there's exactly one.
  ForLeft,
  /// `__isEmbeddedTypescriptGenericParameters`: the type parameters of `type T<..> = any`.
  TypeParameters,
}

/// What Prettier's parser reads the code as, and prints.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Root {
  Program(Syntax),
  /// `JsExpressionRoot`, of TypeScript or not.
  Expression(HtmlRoot, bool),
  /// The statements of a Vue event handler.
  EventBinding(Syntax),
  /// A declaration around the code, of TypeScript or not.
  Binding(Binding, bool),
}

impl Root {
  fn of(language: &str) -> Option<Root> {
    use Binding::*;
    Some(match language {
      "babel" => Root::Program(Syntax::Babel),
      "babel-ts" => Root::Program(Syntax::BabelTs),
      "typescript" => Root::Program(Syntax::TypeScript),
      "__js_expression" => Root::Expression(HtmlRoot::JsExpression, false),
      "__ts_expression" => Root::Expression(HtmlRoot::JsExpression, true),
      "__vue_expression" => Root::Expression(HtmlRoot::VueExpression, false),
      "__vue_ts_expression" => Root::Expression(HtmlRoot::VueExpression, true),
      "__ng_directive" => Root::Expression(HtmlRoot::NgDirective, false),
      "__vue_event_binding" => Root::EventBinding(Syntax::Babel),
      "__vue_ts_event_binding" => Root::EventBinding(Syntax::BabelTs),
      "__vue_bindings" => Root::Binding(Parameters, false),
      "__vue_ts_bindings" => Root::Binding(Parameters, true),
      "__vue_for_left" => Root::Binding(ForLeft, false),
      "__vue_ts_for_left" => Root::Binding(ForLeft, true),
      "__vue_generic" => Root::Binding(TypeParameters, true),
      _ => return None,
    })
  }

  fn html_root(self) -> HtmlRoot {
    match self {
      Root::Expression(root, _) => root,
      Root::EventBinding(_) => HtmlRoot::VueEventBinding,
      Root::Program(_) | Root::Binding(..) => HtmlRoot::Program,
    }
  }

  /// Prettier parses the code with `babel-ts`, whose tree has no `ChainExpression`.
  fn has_babel_tree(self) -> bool {
    matches!(
      self,
      Root::Program(Syntax::BabelTs) | Root::EventBinding(Syntax::BabelTs) | Root::Binding(_, true)
    )
  }
}

/// True if `language` is the name of one of Prettier's parsers that [`format`] formats.
pub(crate) fn formats(language: &str) -> bool {
  Root::of(language).is_some()
}

/// The allocator and buffers of the last call on this thread.
#[derive(Default)]
struct Buffers {
  allocator: oxc_allocator::Allocator,
  scratch: Scratch,
}

thread_local! {
    static BUFFERS: RefCell<Buffers> = RefCell::default();
}

/// Formats `request`, as a hook does: its lines after the first are indented by `request.indent`. `None` if Prettier
/// doesn't format its language with `language-js`, or if it doesn't parse.
pub(crate) fn format(
  request: &Request<'_>,
  options: &crate::js::Options,
  host: Host,
) -> Option<String> {
  let root = Root::of(request.language)?;
  with_buffers(|buffers| format_in(request, options, host, root, buffers))
}

/// Formats `v-for="left operator right"`, where `left` asks for the aliases, as `printVueVForDirective` prints them: in
/// one group, so that the aliases break when the line can't hold them with the rest.
pub(crate) fn format_for(
  left: &Request<'_>,
  operator: &str,
  right: &str,
  options: &crate::js::Options,
) -> Option<String> {
  let root = Root::of(left.language)?;
  let Root::Binding(Binding::ForLeft, ts) = root else { return None };
  let language = if ts { "__ts_expression" } else { "__js_expression" };
  let right = Request { code: right, language, ..*left };
  with_buffers(|buffers| format_for_in(left, root, operator, &right, options, buffers))
}

/// `code`, the contents of a Vue file's `<script>`, with its imports sorted, as Prettier's plugins sort it before the
/// file is parsed. `None` if nothing changes or it doesn't parse.
pub(crate) fn sorted_script(code: &str, how: &sort_imports::SortImports) -> Option<String> {
  with_buffers(|buffers| {
    buffers.allocator.reset();
    let allocator = &buffers.allocator;
    let text = allocator.alloc_str(code);
    let paths: [&[u8]; 3] = [b"dummy.tsx", b"dummy.ts", b"dummy.jsx"];
    let sorted = paths.iter().find_map(|path| {
      let (file, _) = parse(allocator, text, path, false, SourceType::Module, true)?;
      Some(sort_imports::sorted_text(&file, how))
    })??;
    String::from_utf8(sorted).ok()
  })
}

fn with_buffers(run: impl FnOnce(&mut Buffers) -> Option<String>) -> Option<String> {
  BUFFERS.with(|buffers| match buffers.try_borrow_mut() {
    Ok(mut buffers) => run(&mut buffers),
    // Formatting code can format other code, as in an HTML template in a script in HTML.
    Err(_) => run(&mut Buffers::default()),
  })
}

fn format_in(
  request: &Request<'_>,
  options: &crate::js::Options,
  host: Host,
  root: Root,
  buffers: &mut Buffers,
) -> Option<String> {
  let Buffers { allocator, scratch } = buffers;
  allocator.reset();
  let is_script = matches!(root, Root::Program(_)) && request.kind == Kind::Block;
  let sorts = options.sort_imports.clone().filter(|_| is_script);
  let options =
    FormatOptions { sort_imports: sorts.clone(), ..options_for(request, options, host, root) };
  let allocator = &*allocator;
  let (mut file, mut errors) = parse_root(allocator, request.code, root, request.module)?;
  if let Some(sorted) = sorts.and_then(|how| sort_imports::sorted_text(&file, &how)) {
    let code = std::str::from_utf8(&sorted).ok()?;
    (file, errors) = parse_root(allocator, code, root, request.module)?;
  }
  let file = &file;
  let mut out = Vec::with_capacity(request.code.len() + 16);
  let written = Cell::new(true);
  let document = match root {
    Root::Program(_) => run::document(file, &options, scratch, |file, f| {
      write_placed(request, f, &|f| write_statements(file, f));
    }),
    _ => {
      let context = context_of(file, &options);
      run::write_with(context, file.text(), scratch, |f| {
        write_placed(request, f, &|f| written.set(write_root(root, file, f)));
      })
    }
  }
  .ok()?;
  if !written.get() {
    return None;
  }
  let options = match host {
    Host::Template => FormatOptions { is_in_markdown: true, ..options },
    _ => options,
  };
  run::print(document, file.text(), &options, scratch, &mut out).ok()?;
  let text = text_of(out, host)?;
  if !errors.is_empty() {
    // Where oxc's tree has no room for what an error is about, the printer drops it. See `js::keeps_errors`.
    let allocator = oxc_allocator::Allocator::default();
    let (_, again) = parse_root(&allocator, &text, root, request.module)?;
    crate::js::same_errors(&errors, &again).then_some(())?;
  }
  Some(text)
}

fn format_for_in(
  left: &Request<'_>,
  root: Root,
  operator: &str,
  right: &Request<'_>,
  options: &crate::js::Options,
  buffers: &mut Buffers,
) -> Option<String> {
  let Buffers { allocator, scratch } = buffers;
  allocator.reset();
  let Root::Binding(binding, ts) = root else { return None };
  let expression = Root::Expression(HtmlRoot::JsExpression, ts);
  let (options, inner) = (
    options_for(left, options, Host::Markup, root),
    options_for(right, options, Host::Markup, expression),
  );
  let allocator = &*allocator;
  let (file, _) = &parse_root(allocator, left.code, root, true)?;
  let (other, _) = &parse_root(allocator, right.code, expression, true)?;
  let e = expression_of(other)?;
  let written = Cell::new(true);
  let document = run::write_with(context_of(file, &options), file.text(), scratch, |f| {
    write_placed(left, f, &|f| {
      let content = format_with(|f| {
        written.set(write_binding(binding, ts, file, f));
        write!(f, [" ", text(operator.as_bytes()), " "]);
        f.write_embedded(context_of(other, &inner), other.text(), |f| write!(f, group(&e)));
      });
      write!(f, group(&content));
    });
  })
  .ok()?;
  if !written.get() {
    return None;
  }
  let mut out = Vec::with_capacity(left.code.len() + right.code.len() + 16);
  run::print(document, file.text(), &options, scratch, &mut out).ok()?;
  text_of(out, Host::Markup)
}

fn context_of<'a>(file: &'a File<'a>, options: &FormatOptions) -> JsFormatContext<'a> {
  let comments = crate::js::comments::for_file(file, options);
  JsFormatContext::new(file, options.clone(), comments)
}

/// The printed text, without the line break and indents before it.
fn text_of(out: Vec<u8>, host: Host) -> Option<String> {
  let out = String::from_utf8(out).ok()?;
  let out = out.strip_prefix('\n').unwrap_or(&out);
  let out = out.trim_start_matches([' ', '\t']).trim_end();
  Some(match host {
    Host::Markup | Host::Template => out.to_owned(),
    // The printer marks the line breaks in texts for Markdown, which indents every line alike.
    Host::Markdown => out.replace("\r\n", "\n"),
  })
}

/// Parses `code` as what `root` reads, or returns `None` if it has errors.
fn parse_root<'a>(
  allocator: &'a oxc_allocator::Allocator,
  code: &str,
  root: Root,
  module: bool,
) -> Option<(File<'a>, Errors)> {
  let (text, paths, source_type) = match root {
    Root::Program(syntax) => {
      let source_type = match module || syntax != Syntax::Babel {
        true => SourceType::Unknown,
        false => SourceType::Script,
      };
      (allocator.alloc_str(code), paths_of(syntax, code), source_type)
    }
    Root::Expression(_, ts) => {
      let path = if ts { EXPRESSION_TS } else { EXPRESSION_JSX };
      (allocator.alloc_str(&format!("({code}\n)")), &[path][..], SourceType::Module)
    }
    Root::EventBinding(syntax) => {
      (allocator.alloc_str(code), paths_of(syntax, code), SourceType::Unknown)
    }
    Root::Binding(binding, ts) => {
      let text = match binding {
        Binding::Parameters | Binding::ForLeft => format!("function _({code}) {{}}"),
        Binding::TypeParameters => format!("type T<{code}> = any"),
      };
      let syntax = if ts { Syntax::BabelTs } else { Syntax::Babel };
      let paths = paths_of(syntax, &text);
      (allocator.alloc_str(&text), paths, SourceType::Unknown)
    }
  };
  let kinds: &[bool] = match source_type {
    SourceType::Script => &[true],
    SourceType::Module => &[false],
    SourceType::Unknown => &[false, true],
  };
  paths.iter().find_map(|path| {
    let is_program = matches!(root, Root::Program(_));
    kinds
      .iter()
      .find_map(|&is_script| parse(allocator, text, path, is_script, source_type, is_program))
  })
}

/// The options to format `request` with: Prettier's `textToDoc` passes the host's, with what the parser tells.
fn options_for(
  request: &Request<'_>,
  options: &crate::js::Options,
  host: Host,
  root: Root,
) -> FormatOptions {
  let options = FormatOptions::from(options);
  let is_in_attribute = request.kind == Kind::Attribute;
  let in_html = InHtml {
    root: root.html_root(),
    is_in_attribute,
    is_in_interpolation: request.kind == Kind::Expression,
    is_inline_event_handler: is_in_attribute && root == Root::Program(Syntax::Babel),
    quote_style: options.quote_style,
    has_babel_tree: root.has_babel_tree(),
    ..InHtml::default()
  };
  FormatOptions {
    // In an attribute, double quotes would have to be written as entities.
    quote_style: if is_in_attribute { QuoteStyle::Single } else { options.quote_style },
    in_html,
    // The file name is the host's, so `<T,>() => {}` keeps its comma.
    filepath: Some(Box::default()),
    is_in_markdown: host == Host::Markdown,
    // prettier-plugin-jsdoc wraps Prettier's parsers of programs, not those of expressions and bindings.
    jsdoc: options.jsdoc.filter(|_| matches!(root, Root::Program(_))),
    ..options
  }
}

/// Parses `text` as the file at `path`, or returns `None` if it has errors.
fn parse<'a>(
  allocator: &'a oxc_allocator::Allocator,
  text: &'a str,
  path: &'a [u8],
  is_script: bool,
  source_type: SourceType,
  is_program: bool,
) -> Option<(File<'a>, Errors)> {
  let extension = path.rsplit(|byte| *byte == b'.').next().unwrap_or_default();
  let kind = match extension {
    b"ts" => oxc_span::SourceType::ts(),
    b"tsx" => oxc_span::SourceType::tsx(),
    _ => oxc_span::SourceType::jsx(),
  };
  let kind = match is_script {
    true => kind.with_script(true),
    false => kind.with_module(true),
  };
  let options = oxc_parser::ParseOptions {
    preserve_parens: true,
    allow_return_outside_function: true,
    ..oxc_parser::ParseOptions::default()
  };
  let parsed = oxc_parser::Parser::new(allocator, text, kind).with_options(options).parse();
  // Prettier's parsers recover from some errors in programs, as in a whole file.
  let rejected = match is_program {
    true => crate::js::rejection(&parsed).is_some(),
    false => !parsed.diagnostics.is_empty(),
  };
  if rejected {
    return None;
  }
  // Babel only treats `<!--` and `-->` as comments in code that's known to be a script.
  let is_html_like = |comment: &oxc_ast::Comment| {
    matches!(text.as_bytes().get(comment.span.start as usize), Some(b'<' | b'-'))
  };
  if source_type != SourceType::Script && parsed.program.comments.iter().any(is_html_like) {
    return None;
  }
  let errors = parsed.diagnostics.to_vec();
  let program = allocator.alloc(parsed.program);
  Some((File::new(text, path, kind, program, allocator), errors))
}

/// The errors that a program was parsed with. Prettier's parsers recover from them.
type Errors = Vec<oxc_diagnostics::OxcDiagnostic>;

fn paths_of(syntax: Syntax, code: &str) -> &'static [&'static [u8]] {
  match syntax {
    Syntax::Babel => &[b"dummy.jsx"],
    Syntax::BabelTs => &[b"dummy.tsx", b"dummy.ts"],
    Syntax::TypeScript => match is_probably_jsx(code.as_bytes()) {
      true => &[b"dummy.tsx", b"dummy.ts"],
      false => &[b"dummy.ts", b"dummy.tsx"],
    },
  }
}

/// Prettier's `isProbablyJsx`: `/(?:^[^"'`]*<\/|^[^/]{2}.*\/>)/m`
fn is_probably_jsx(text: &[u8]) -> bool {
  // `</` with no quote between it and the start of its line.
  let mut rest: &[u8] = if strings::contains(text, b"</") { text } else { b"" };
  let mut is_after_quote = false;
  while let Some(at) = strings::index_of_any(rest, b"\"'`\n<") {
    match rest[at] {
      b'\n' => is_after_quote = false,
      b'<' if is_after_quote || rest.get(at + 1) != Some(&b'/') => {}
      b'<' => return true,
      _ => is_after_quote = true,
    }
    rest = &rest[at + 1..];
  }
  strings::split(text, b"\n").any(|line| {
    line.len() >= 4 && line[0] != b'/' && line[1] != b'/' && strings::contains(&line[2..], b"/>")
  })
}

/// Writes a line break and `content` inside `levels` indents, so that the text after the line break starts where the
/// host prints it.
fn write_indented<'a>(levels: usize, f: &mut Formatter<'a>, content: &dyn Fn(&mut Formatter<'a>)) {
  match levels {
    0 => {
      write!(f, hard_line_break());
      content(f);
    }
    _ => write!(f, indent(&format_with(|f| write_indented(levels - 1, f, content)))),
  }
}

/// Writes `content` where `request` places it: after its indents and `prefix`, and before its `suffix`.
fn write_placed<'a>(
  request: &Request<'_>,
  f: &mut Formatter<'a>,
  content: &dyn Fn(&mut Formatter<'a>),
) {
  let pad = |f: &mut Formatter<'a>, width: usize| {
    if width > 0 {
      f.write_built_text(|text| text.resize(text.len() + width, b' '));
    }
  };
  write_indented(request.indent, f, &|f| {
    pad(f, request.prefix);
    content(f);
    pad(f, request.suffix);
  });
}

/// Writes what Prettier prints for a `Program`, without the final line break.
fn write_statements<'b>(file: &'b File<'b>, f: &mut Formatter<'b>) {
  let program = node_as_ast_nodes(Node::File(file));
  let owned = f.comments().of_node(program);
  let has_statements = (file.body().iter()).any(|it| !matches!(it.kind(), StmtKind::Empty));
  let is_hashbang_last = !has_statements && owned.leading.is_empty() && owned.dangling.is_empty();
  write_hashbang(is_hashbang_last, f);
  // The program only owns comments if there's no statement to attach them to.
  write!(f, [leading_comments(program), FormatStatements(file.body()), dangling_comments(program)]);
}

/// Writes the code of a root other than a program. Returns false if the code isn't what the root holds.
fn write_root<'b>(root: Root, file: &'b File<'b>, f: &mut Formatter<'b>) -> bool {
  match root {
    Root::Program(_) | Root::EventBinding(_) => {
      write_statements(file, f);
      true
    }
    Root::Expression(..) => match expression_of(file) {
      Some(e) => {
        write!(f, e);
        true
      }
      None => false,
    },
    Root::Binding(binding, ts) => write_binding(binding, ts, file, f),
  }
}

/// Returns the expression in `file` if its text is `(`, an expression, a line break and `)`.
fn expression_of<'b>(file: &'b File<'b>) -> Option<Expr<'b>> {
  let mut statements = file.body().iter();
  let (Some(statement), None) = (statements.next(), statements.next()) else {
    return None;
  };
  let StmtKind::Expr(e) = statement.kind() else {
    return None;
  };
  let outer = e.outer_span();
  (e.span().start > 0 && outer.start == 0 && outer.end as usize == file.text().len()).then_some(e)
}

/// `printHtmlBinding`
fn write_binding<'b>(
  binding: Binding,
  ts: bool,
  file: &'b File<'b>,
  f: &mut Formatter<'b>,
) -> bool {
  let Some(statement) = file.body().iter().next() else {
    return false;
  };
  let separator = format_with(|f| write!(f, [",", soft_line_break_or_space()]));
  match (binding, statement.kind()) {
    (Binding::TypeParameters, StmtKind::TypeAlias(alias)) => {
      let list = format_with(|f| {
        f.join_with(&separator).entries(alias.type_params().iter());
      });
      write!(f, group(&list));
    }
    (Binding::Parameters | Binding::ForLeft, StmtKind::Fn(func)) => {
      // Only TypeScript has a `this` parameter.
      if func.this_param().is_some() && !ts {
        return false;
      }
      let list = format_with(|f| {
        f.join_with(&separator).entries(func.params_with_this());
      });
      match binding == Binding::ForLeft && func.params_with_this().count() != 1 {
        true => write!(
          f,
          group(&format_args!(
            "(",
            indent(&format_args!(soft_line_break(), group(&list))),
            soft_line_break(),
            ")"
          ))
        ),
        false => write!(f, group(&list)),
      }
    }
    _ => return false,
  }
  true
}
