# JavaScript and TypeScript

tidy formats JavaScript and TypeScript with a port of bun's formatter (`bun fmt`), whose output is Prettier 3.9.9's byte for byte, but where Prettier's output changes what the code means. bun's printer reads bun's own syntax tree. tidy reads oxc's, through a layer that gives the printer the same handles. `PROVENANCE.md` maps each file to bun's.

```text
text ─ oxc_parser ─→ oxc AST ─ ast::File::new ─→ node tables, handles
                                  └─ comments: decorate ─→ attach ─→ owners
handles + comments ─ format ─→ ir document ─ ir::printer ─→ String
```

## Why handles over oxc

bun's printer is bun's port of oxc's formatter, corrected until it matches Prettier. Its functions take handles (`Expr`, `Stmt`, `Func`, ..) and call `kind()`, `parent()` and accessors such as `call.callee()`. `js::ast` gives the same API over oxc's tree, so the printer ports with few changes, and later fixes from bun apply as diffs.

## The node tables

oxc's parser numbers every node densely: `Program` is 0, the rest count up as they are built. `ast::File::new` (with the text, its path, oxc's source type, the program and the arena) walks the tree once with `oxc_ast_visit::Visit` and fills three tables, indexed by node id:

- `nodes`: the `AstKind` of each node.
- `parents`: the id of each node's parent.
- `children`: each node's children in the order of their fields, grouped by parent. That is source order but in a template literal, whose parts come before its expressions, and in a function with a `this` parameter, which comes after the others; Prettier's `getSortedChildNodes` sorts those. The elements of one list field are a run of siblings, so a `List` is a range of this table, as in bun.

A handle is the file and a node id, `Copy` and two words wide, and compares by identity, as bun's are.

## How bun's tree maps onto oxc's

bun's tree is built for its type checker, not ESTree. Where they differ, `js::ast` presents bun's shape:

- Parentheses aren't nodes. The parser keeps `ParenthesizedExpression`; the tables skip it, a handle to `(a)` is `a`, and `Expr::is_parenthesized` and `Expr::outer_span` tell about the parentheses.
- `ChainExpression` isn't a node. `a?.b.c` is `Dot`s and `Call`s with a `Chain` of `Start` or `Continue`.
- `Missing` is an array hole (`Elision`), the `{}` of JSX (`JSXEmptyExpression`), and a missing argument.
- JSX text, a string value of a JSX attribute, and a tag name such as `a-b` are `String` expressions.
- `Func` is a function declaration or expression, an arrow function, and the function of a method, accessor or constructor.
- `Member` is an element of a class, an interface or a type literal. `Prop` is a property of an object literal.
- `Pat` is a binding pattern, `Param` a formal parameter, `TypeNode` a TypeScript type.

| bun handle | oxc nodes |
| --- | --- |
| `Expr` | every `Expression` but `ParenthesizedExpression` and `ChainExpression`, `Elision`, `JSXText`, `JSXEmptyExpression`, JSX names and string values |
| `Stmt` | every `Statement`, and `VariableDeclaration` in a `for` head |
| `TypeNode` | every `TSType` |
| `Func` | `Function`, `ArrowFunctionExpression` |
| `Class` | `Class` |
| `Member` | `ClassElement`, `TSSignature` |
| `Prop` | `ObjectProperty`, a `SpreadElement` in an object |
| `Pat`, `PatProp`, `PatElem` | `BindingPattern`, `BindingProperty`, an element of `ArrayPattern` |
| `Param`, `TypeParam` | `FormalParameter`, `FormalParameterRest`, `TSThisParameter`, `TSIndexSignatureName`; `TSTypeParameter` and a mapped type's key |
| `VarDecl`, `Case`, `EnumMember` | `VariableDeclarator`, `SwitchCase`, `TSEnumMember` |
| `ImportSpec`, `ExportSpec`, `TupleElem` | `ImportDeclarationSpecifier`, `ExportSpecifier`, `TSTupleElement` |

## Comments

bun's parser records, for each comment, Prettier's `decorateComment`: the innermost node that encloses it and that node's children right before and after it, with the end of the token before it and the start of the token after it. `comments::decorate` computes the same from the node tables and the text. `attach/` then places each comment with Prettier's rules, as in bun.

## Entry points

- A file: `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`.
- Code in markup and Markdown, by Prettier's parser names: `babel` and `typescript` for scripts and code blocks, and `__js_expression`, `__ts_expression`, `__vue_expression`, `__vue_ts_expression`, `__vue_event_binding`, `__vue_ts_event_binding`, `__vue_bindings`, `__vue_ts_bindings`, `__vue_for_left`, `__vue_ts_for_left` and `__vue_generic` for template code (`template/`). The embed hook can still answer first.

In JavaScript templates, CSS, Markdown, HTML and GraphQL format with tidy's formatters (`embed/`).

## Choices

- JSDoc comments are formatted by default, as prettier-plugin-jsdoc does (Luke, 2026-11-02). Import sorting is an option.
- Flow isn't formatted: oxc doesn't parse it.
- No cursor, no range, no pragmas: tidy has none of these options.
- The `js` feature, on by default, builds this module. Without it, scripts go to the embed hook.
