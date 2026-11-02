//! JavaScript and TypeScript in markup and Markdown: Prettier's `textToDoc` for the parsers of `language-js`.
//!
//! bun writes the code into the document of the HTML around it. tidy's markup printers take text from
//! [`crate::embed`] instead, so the code is printed alone, inside as many indents as it starts at, the way a hook
//! prints it.

use crate::embed::{Kind, Request};
use crate::js::ast_nodes::node_as_ast_nodes;
use crate::js::base::*;
use crate::js::context::JsFormatContext;
use crate::js::ir::run::{self, Scratch};
use crate::js::options::{HtmlRoot, InHtml};
use crate::js::print::program::{FormatStatements, write_hashbang};
use crate::js::{format_args, strings, write};
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

/// Where the code is.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Host {
  Markup,
  /// A code block, where Prettier's `parentParser` is `markdown`.
  Markdown,
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
  BUFFERS.with(|buffers| match buffers.try_borrow_mut() {
    Ok(mut buffers) => format_in(request, options, host, root, &mut buffers),
    // Formatting code can format other code, as in an HTML template in a script in HTML.
    Err(_) => format_in(request, options, host, root, &mut Buffers::default()),
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
  let options = options_for(request, options, host, root);
  let code = request.code;
  let (text, paths, source_type) = match root {
    Root::Program(syntax) => {
      let source_type = match request.module || syntax != Syntax::Babel {
        true => SourceType::Unknown,
        false => SourceType::Script,
      };
      (code.to_owned(), paths_of(syntax, code), source_type)
    }
    Root::Expression(_, ts) => {
      let path = if ts { EXPRESSION_TS } else { EXPRESSION_JSX };
      (format!("({code}\n)"), &[path][..], SourceType::Module)
    }
    Root::EventBinding(syntax) => (code.to_owned(), paths_of(syntax, code), SourceType::Unknown),
    Root::Binding(binding, ts) => {
      let text = match binding {
        Binding::Parameters | Binding::ForLeft => format!("function _({code}) {{}}"),
        Binding::TypeParameters => format!("type T<{code}> = any"),
      };
      let syntax = if ts { Syntax::BabelTs } else { Syntax::Babel };
      let paths = paths_of(syntax, &text);
      (text, paths, SourceType::Unknown)
    }
  };
  let kinds: &[bool] = match source_type {
    SourceType::Script => &[true],
    SourceType::Module => &[false],
    SourceType::Unknown => &[false, true],
  };
  let allocator = &*allocator;
  let file = paths.iter().find_map(|path| {
    kinds.iter().find_map(|&is_script| parse(allocator, &text, path, is_script, source_type))
  })?;
  let file = &file;
  let mut out = Vec::with_capacity(text.len() + 16);
  let levels = request.indent;
  let written = Cell::new(true);
  match root {
    Root::Program(_) => {
      run::format(file, &options, scratch, &mut out, |file, f| {
        write_indented(levels, f, &|f| write_statements(file, f));
      })
      .ok()?;
    }
    _ => {
      let comments = crate::js::comments::for_file(file, &options);
      let context = JsFormatContext::new(file, options.clone(), comments);
      let document = run::write_with(context, file.text(), scratch, |f| {
        write_indented(levels, f, &|f| written.set(write_root(root, file, f)));
      })
      .ok()?;
      if !written.get() {
        return None;
      }
      run::print(document, file.text(), &options, scratch, &mut out).ok()?;
    }
  }
  let out = String::from_utf8(out).ok()?;
  let out = out.strip_prefix('\n').unwrap_or(&out);
  let out = out.trim_start_matches([' ', '\t']).trim_end();
  Some(match host {
    Host::Markup => out.to_owned(),
    // The printer marks the line breaks in texts for Markdown, which indents every line alike.
    Host::Markdown => out.replace("\r\n", "\n"),
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
) -> Option<File<'a>> {
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
  if !parsed.diagnostics.is_empty() {
    return None;
  }
  // Babel only treats `<!--` and `-->` as comments in code that's known to be a script.
  let is_html_like = |comment: &oxc_ast::Comment| {
    matches!(text.as_bytes().get(comment.span.start as usize), Some(b'<' | b'-'))
  };
  if source_type != SourceType::Script && parsed.program.comments.iter().any(is_html_like) {
    return None;
  }
  let program = allocator.alloc(parsed.program);
  Some(File::new(text, path, kind, program, allocator))
}

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
