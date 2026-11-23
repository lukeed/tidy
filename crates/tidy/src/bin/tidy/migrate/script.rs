//! The object a JavaScript or TypeScript config exports, read without running it: `export default {...}`,
//! `module.exports = {...}`, a `const` that one of them names, and a `defineConfig({...})` or `satisfies` around it.

use std::cell::RefCell;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
  ArrayExpressionElement, AssignmentTarget, BindingPattern, Expression, ObjectPropertyKind,
  Program, Statement, UnaryOperator,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use serde_json::{Map, Number, Value};

/// The exported object of the config at `path`, whose text is `text`, without the properties that only running it
/// would tell, and a note for each of those.
pub(crate) fn value(text: &str, path: &Path) -> Result<(Value, Vec<String>), String> {
  let allocator = Allocator::default();
  let source = SourceType::from_path(path).unwrap_or_default();
  let parsed = Parser::new(&allocator, text, source).parse();
  if parsed.diagnostics.has_errors() {
    let error = parsed.diagnostics.iter().map(ToString::to_string).next().unwrap_or_default();
    return Err(format!("it doesn't parse: {error}"));
  }
  let reader = Reader { text, program: &parsed.program, notes: RefCell::new(Vec::new()) };
  let exported = reader
    .exported()
    .ok_or("it exports no object: `export default {...}` or `module.exports = {...}`")?;
  let value = reader.value(exported)?;
  Ok((value, reader.notes.into_inner()))
}

struct Reader<'a, 'p> {
  notes: RefCell<Vec<String>>,
  text: &'a str,
  program: &'p Program<'a>,
}

impl<'a, 'p> Reader<'a, 'p> {
  fn exported(&self) -> Option<&'p Expression<'a>> {
    self.program.body.iter().find_map(|statement| match statement {
      Statement::ExportDefaultDeclaration(it) => it.declaration.as_expression(),
      Statement::ExpressionStatement(it) => match &it.expression {
        Expression::AssignmentExpression(it) => match &it.left {
          AssignmentTarget::StaticMemberExpression(member)
            if member.property.name == "exports"
              && matches!(&member.object, Expression::Identifier(it) if it.name == "module") =>
          {
            Some(&it.right)
          }
          _ => None,
        },
        _ => None,
      },
      _ => None,
    })
  }

  /// The value of the top-level `const`, `let` or `var` named `name`.
  fn variable(&self, name: &str) -> Option<&'p Expression<'a>> {
    self.program.body.iter().find_map(|statement| {
      let declaration = match statement {
        Statement::VariableDeclaration(it) => it,
        Statement::ExportDeclaration(it) => match &it.declaration {
          oxc_ast::ast::Declaration::VariableDeclaration(it) => it,
          _ => return None,
        },
        _ => return None,
      };
      declaration.declarations.iter().find_map(|it| match &it.id {
        BindingPattern::BindingIdentifier(id) if id.name == name => it.init.as_ref(),
        _ => None,
      })
    })
  }

  fn line(&self, at: u32) -> usize {
    self.text[..at as usize].matches('\n').count() + 1
  }

  fn fail<T>(&self, expression: &Expression<'a>, what: &str) -> Result<T, String> {
    Err(self.unread(expression.span().start, what))
  }

  fn unread(&self, at: u32, what: &str) -> String {
    format!(
      "line {}: tidy can't read {what} without running the file; set it by hand",
      self.line(at)
    )
  }

  fn value(&self, expression: &'p Expression<'a>) -> Result<Value, String> {
    match expression {
      Expression::ObjectExpression(object) => {
        let mut map = Map::new();
        for property in &object.properties {
          let ObjectPropertyKind::ObjectProperty(property) = property else {
            self.notes.borrow_mut().push(self.unread(property.span().start, "a spread"));
            continue;
          };
          let key = property.key.static_name();
          let Some(key) = key.as_ref().and_then(|it| it.as_str()) else {
            self.notes.borrow_mut().push(self.unread(property.span.start, "a computed key"));
            continue;
          };
          match self.value(&property.value) {
            Ok(value) => drop(map.insert(key.to_owned(), value)),
            Err(error) => self.notes.borrow_mut().push(format!("`{key}`: {error}")),
          }
        }
        Ok(Value::Object(map))
      }
      Expression::ArrayExpression(array) => array
        .elements
        .iter()
        .map(|element| match element {
          ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => {
            self.fail(expression, "a spread or a hole in an array")
          }
          element => self.value(element.to_expression()),
        })
        .collect::<Result<_, _>>()
        .map(Value::Array),
      Expression::StringLiteral(it) => match it.value.as_str() {
        Some(text) => Ok(Value::String(text.to_owned())),
        None => self.fail(expression, "a string has a lone surrogate"),
      },
      Expression::TemplateLiteral(it) if it.expressions.is_empty() => {
        let cooked = it.quasis.iter().map(|it| it.value.cooked.and_then(|it| it.as_str()));
        match cooked.collect::<Option<String>>() {
          Some(text) => Ok(Value::String(text)),
          None => self.fail(expression, "a template has an invalid escape"),
        }
      }
      Expression::NumericLiteral(it) => Ok(number(it.value)),
      Expression::UnaryExpression(it) if it.operator == UnaryOperator::UnaryNegation => {
        match &it.argument {
          Expression::NumericLiteral(it) => Ok(number(-it.value)),
          _ => self.fail(expression, "a computed number"),
        }
      }
      Expression::BooleanLiteral(it) => Ok(Value::Bool(it.value)),
      Expression::NullLiteral(_) => Ok(Value::Null),
      Expression::ParenthesizedExpression(it) => self.value(&it.expression),
      Expression::TSAsExpression(it) => self.value(&it.expression),
      Expression::TSSatisfiesExpression(it) => self.value(&it.expression),
      Expression::CallExpression(call)
        if matches!(&call.callee, Expression::Identifier(it) if it.name == "defineConfig")
          && call.arguments.len() == 1 =>
      {
        match call.arguments[0].as_expression() {
          Some(argument) => self.value(argument),
          None => self.fail(expression, "a spread"),
        }
      }
      Expression::Identifier(it) => match self.variable(&it.name) {
        Some(init) => self.value(init),
        None => self.fail(expression, &format!("`{}` is not a top-level variable", it.name)),
      },
      _ => self.fail(expression, "a computed value"),
    }
  }
}

fn number(value: f64) -> Value {
  match value.fract() == 0.0 && value.abs() < 9e15 {
    true => Value::from(value as i64),
    false => Number::from_f64(value).map_or(Value::Null, Value::Number),
  }
}
