//! Prints docs given as JSON on stdin, for `tests/oracle/doc.ts`.

use std::io::Read;

use bumpalo::Bump;
use serde_json::Value;
use tidyrs::doc::{Align, Doc, Docs, GroupId, Options, print};

struct Builder<'a, 'b> {
  docs: &'b mut Docs<'a>,
  bump: &'a Bump,
  ids: Vec<Option<GroupId>>,
  defs: Vec<Doc>,
}

impl<'a> Builder<'a, '_> {
  fn id(&mut self, value: &Value) -> Option<GroupId> {
    let index = value.as_u64()? as usize;
    if self.ids.len() <= index {
      self.ids.resize(index + 1, None);
    }
    Some(*self.ids[index].get_or_insert_with(|| self.docs.group_id()))
  }

  fn list(&mut self, value: &Value) -> Vec<Doc> {
    value.as_array().unwrap().iter().map(|part| self.build(part)).collect()
  }

  fn build(&mut self, value: &Value) -> Doc {
    if let Some(text) = value.as_str() {
      let text = self.bump.alloc_str(text);
      return self.docs.text(text);
    }
    if value.is_array() {
      let parts = self.list(value);
      return self.docs.concat(&parts);
    }
    let field = |name: &str| &value[name];
    match field("t").as_str().unwrap() {
      "ref" => self.defs[field("i").as_u64().unwrap() as usize],
      "group" => {
        let id = self.id(field("id"));
        let broken = field("b").as_bool().unwrap_or(false);
        if field("s").is_array() {
          let states = self.list(field("s"));
          let doc = self.docs.conditional(&states);
          assert!(id.is_none() && !broken);
          return doc;
        }
        let contents = self.build(field("c"));
        self.docs.group_with(contents, id, broken)
      }
      "fill" => {
        let parts = self.list(field("p"));
        self.docs.fill(&parts)
      }
      "indent" => {
        let contents = self.build(field("c"));
        self.docs.indent(contents)
      }
      "align" => {
        let contents = self.build(field("c"));
        let align = match field("n") {
          Value::String(text) if text == "root" => Align::Mark,
          Value::String(text) if text == "dedentToRoot" => Align::Root,
          Value::String(text) => Align::Text(self.bump.alloc_str(text)),
          Value::Number(n) if n.as_i64() == Some(-1) => Align::Dedent,
          Value::Number(n) => Align::Width(n.as_u64().unwrap() as usize),
          other => panic!("align {other}"),
        };
        self.docs.align(align, contents)
      }
      "ifBreak" => {
        let broken = self.build(field("b"));
        let flat = self.build(field("f"));
        let id = self.id(field("id"));
        self.docs.if_break(broken, flat, id)
      }
      "indentIfBreak" => {
        let contents = self.build(field("c"));
        let id = self.id(field("id")).unwrap();
        self.docs.indent_if_break(contents, id, field("neg").as_bool().unwrap_or(false))
      }
      "lineSuffix" => {
        let contents = self.build(field("c"));
        self.docs.line_suffix(contents)
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
    let mut builder = Builder { docs: &mut docs, bump: &bump, ids: Vec::new(), defs: Vec::new() };
    for def in case["defs"].as_array().unwrap() {
      let doc = builder.build(def);
      builder.defs.push(doc);
    }
    let doc = builder.build(&case["doc"]);
    let options = Options {
      width: case["width"].as_u64().unwrap() as usize,
      tab_width: case["tabWidth"].as_u64().unwrap() as usize,
      tabs: case["useTabs"].as_bool().unwrap(),
      eol: Default::default(),
    };
    outputs.push(Value::String(print(&mut docs, doc, &options)));
  }
  println!("{}", Value::Array(outputs));
}
