//! Prints docs given as JSON on stdin, for `tests/oracle/doc.ts`.

use std::io::Read;

use bumpalo::Bump;
use serde_json::Value;
use tidy::doc::{Align, Branch, Docs, GroupId, Options, print};

struct Builder<'a, 'b> {
  docs: &'b mut Docs<'a>,
  bump: &'a Bump,
  ids: Vec<Option<GroupId>>,
  defs: Vec<Value>,
}

impl<'a> Builder<'a, '_> {
  fn id(&mut self, value: &Value) -> Option<GroupId> {
    let index = value.as_u64()? as usize;
    if self.ids.len() <= index {
      self.ids.resize(index + 1, None);
    }
    Some(*self.ids[index].get_or_insert_with(|| self.docs.group_id()))
  }

  fn emit(&mut self, value: &Value) {
    if let Some(text) = value.as_str() {
      let text = self.bump.alloc_str(text);
      return self.docs.text(text);
    }
    if let Some(parts) = value.as_array() {
      for part in parts {
        self.emit(part);
      }
      return;
    }
    let field = |name: &str| &value[name];
    match field("t").as_str().unwrap() {
      "ref" => {
        let def = self.defs[field("i").as_u64().unwrap() as usize].clone();
        self.emit(&def);
      }
      "group" => {
        let id = self.id(field("id"));
        let broken = field("b").as_bool().unwrap_or(false);
        if let Some(states) = field("s").as_array() {
          assert!(id.is_none() && !broken);
          let mark = self.docs.conditional();
          for state in states {
            let part = self.docs.part();
            self.emit(state);
            self.docs.close(part);
          }
          return self.docs.close(mark);
        }
        let mark = self.docs.group_with(id, broken);
        self.emit(field("c"));
        self.docs.close(mark);
      }
      "fill" => {
        let mark = self.docs.fill();
        for part in field("p").as_array().unwrap() {
          let inner = self.docs.part();
          self.emit(part);
          self.docs.close(inner);
        }
        self.docs.close(mark);
      }
      "indent" => {
        let mark = self.docs.indent();
        self.emit(field("c"));
        self.docs.close(mark);
      }
      "align" => {
        let align = match field("n") {
          Value::String(text) if text == "root" => Align::Mark,
          Value::String(text) if text == "dedentToRoot" => Align::Root,
          Value::String(text) => Align::Text(self.bump.alloc_str(text)),
          Value::Number(n) if n.as_i64() == Some(-1) => Align::Dedent,
          Value::Number(n) => Align::Width(n.as_u64().unwrap() as usize),
          other => panic!("align {other}"),
        };
        let mark = self.docs.align(align);
        self.emit(field("c"));
        self.docs.close(mark);
      }
      "ifBreak" => {
        let id = self.id(field("id"));
        let mark = self.docs.if_break(Branch::Break, id);
        self.emit(field("b"));
        self.docs.close(mark);
        let mark = self.docs.if_break(Branch::Flat, id);
        self.emit(field("f"));
        self.docs.close(mark);
      }
      "indentIfBreak" => {
        let id = self.id(field("id")).unwrap();
        let mark = self.docs.indent_if_break(id, field("neg").as_bool().unwrap_or(false));
        self.emit(field("c"));
        self.docs.close(mark);
      }
      "lineSuffix" => {
        let mark = self.docs.line_suffix();
        self.emit(field("c"));
        self.docs.close(mark);
      }
      "lsb" => self.docs.line_suffix_boundary(),
      "line" => self.docs.line(),
      "softline" => self.docs.softline(),
      "hardline" => self.docs.hardline(),
      "literalline" => self.docs.literalline(),
      "bp" => self.docs.break_parent(),
      "trim" => self.docs.trim(),
      other => panic!("unknown doc {other}"),
    }
  }
}

fn main() {
  let mut input = String::new();
  std::io::stdin().read_to_string(&mut input).unwrap();
  let cases: Value = serde_json::from_str(&input).unwrap();
  let mut outputs = Vec::new();
  for case in cases.as_array().unwrap() {
    let bump = Bump::new();
    let mut docs = Docs::new(&bump);
    let defs = case["defs"].as_array().unwrap().clone();
    let mut builder = Builder { docs: &mut docs, bump: &bump, ids: Vec::new(), defs };
    builder.emit(&case["doc"]);
    let options = Options {
      width: case["width"].as_u64().unwrap() as usize,
      tab_width: case["tabWidth"].as_u64().unwrap() as usize,
      tabs: case["useTabs"].as_bool().unwrap(),
      eol: Default::default(),
    };
    outputs.push(Value::String(print(&docs, &options, String::new())));
  }
  println!("{}", Value::Array(outputs));
}
