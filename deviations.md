# Deviations from Prettier

tidyrs prints what Prettier 3.9.9 prints, except where this file says otherwise. Each entry below says what tidyrs prints instead, why, where the rule is, and whether Luke has signed it off. `crates/tidyrs/tests/deviations.json` holds every input that was checked against both tools, and `cargo test --test deviations` fails if tidyrs stops printing what an entry says.

## Process

1. A change that makes tidyrs print something other than Prettier is a deviation. It is never shipped silently: add an entry here with status `proposed`, add its rows to `tests/deviations.json`, and raise it with Luke for sign-off.
2. Proposed rule, which itself needs sign-off: keep a deviation only where Prettier's output changes what the code means (it loses text, changes a value that a program reads, or no longer parses). Where the difference is only layout, print what Prettier prints.
3. Each entry is `proposed` (in the code, awaiting sign-off) or `approved` (with the date). A rejected entry is removed from the code and from this file, and its rows from the test. A deviation that is raised but not yet in the code is listed under [Raised, not in the code](#raised-not-in-the-code).

Most entries come from bun's ports, which correct the places where Prettier's output changes the meaning of the code. When HTML was ported, the corrections that are only layout (an SVG `<text>`, `<script>` and `<dialog>` on their own lines, classes split at ASCII whitespace only) were removed, so HTML prints what Prettier prints there.

## Summary

| Entry | Area | Status |
| --- | --- | --- |
| [hbs-doctype](#hbs-doctype) | handlebars | proposed |
| [hbs-dropped-markup-kept-as-written](#hbs-dropped-markup-kept-as-written) | handlebars | proposed |
| [hbs-html-comment-start-and-end](#hbs-html-comment-start-and-end) | handlebars | proposed |
| [hbs-escaped-mustache](#hbs-escaped-mustache) | handlebars | proposed |
| [hbs-style-with-mustache](#hbs-style-with-mustache) | handlebars | proposed |
| [hbs-no-break-space](#hbs-no-break-space) | handlebars | proposed |
| [hbs-no-break-space-in-class](#hbs-no-break-space-in-class) | handlebars | proposed |
| [hbs-backslash-before-mustache](#hbs-backslash-before-mustache) | handlebars | proposed |
| [hbs-both-quotes-in-value](#hbs-both-quotes-in-value) | handlebars | proposed |
| [hbs-script-textarea-lines](#hbs-script-textarea-lines) | handlebars | proposed |
| [hbs-pre-descendant-whitespace](#hbs-pre-descendant-whitespace) | handlebars | proposed |
| [hbs-whitespace-block-in-pre-textarea](#hbs-whitespace-block-in-pre-textarea) | handlebars | proposed |
| [hbs-void-with-children](#hbs-void-with-children) | handlebars | proposed |
| [hbs-path-round-trip](#hbs-path-round-trip) | handlebars | proposed |
| [hbs-bracketed-segment](#hbs-bracketed-segment) | handlebars | proposed |
| [hbs-hash-key-in-brackets](#hbs-hash-key-in-brackets) | handlebars | proposed |
| [hbs-dropped-arguments](#hbs-dropped-arguments) | handlebars | proposed |
| [hbs-raw-block](#hbs-raw-block) | handlebars | proposed |
| [hbs-number-exponent](#hbs-number-exponent) | handlebars | proposed |
| [hbs-triple-stash-strip](#hbs-triple-stash-strip) | handlebars | proposed |
| [hbs-comment-dashes](#hbs-comment-dashes) | handlebars | proposed |
| [hbs-else-chain](#hbs-else-chain) | handlebars | proposed |
| [hbs-line-separator-columns](#hbs-line-separator-columns) | handlebars | proposed |
| [hbs-ws-ignore-escaped-mustache](#hbs-ws-ignore-escaped-mustache) | handlebars | proposed |
| [css-escaped-space-before-semicolon](#css-escaped-space-before-semicolon) | css/scss/less | proposed |
| [css-escaped-space-in-value](#css-escaped-space-in-value) | css/scss/less | proposed |
| [css-selector-without-namespace](#css-selector-without-namespace) | css/scss/less | proposed |
| [css-non-ascii-lowercase](#css-non-ascii-lowercase) | css/scss/less | proposed |
| [less-variable-colon-without-space](#less-variable-colon-without-space) | less | proposed |
| [less-variable-comment](#less-variable-comment) | less | proposed |
| [css-comment-only-at-rule-params](#css-comment-only-at-rule-params) | scss | proposed |
| [css-line-comment-before-brace-or-semicolon](#css-line-comment-before-brace-or-semicolon) | scss/less | proposed |
| [css-nth-and-keyframe-sign](#css-nth-and-keyframe-sign) | css/scss/less | proposed |
| [css-hex-escape-space-in-selector](#css-hex-escape-space-in-selector) | css/scss/less | proposed |
| [css-attribute-value-escape](#css-attribute-value-escape) | css/scss/less | proposed |
| [css-hex-escape-space-in-value](#css-hex-escape-space-in-value) | css/scss/less | proposed |
| [css-author-defined-names](#css-author-defined-names) | css/scss/less | proposed |
| [css-custom-property-values](#css-custom-property-values) | css/scss/less | proposed |
| [css-custom-property-line-breaks](#css-custom-property-line-breaks) | css/scss/less | proposed |
| [css-custom-property-json](#css-custom-property-json) | css/scss/less | proposed |
| [css-custom-property-url](#css-custom-property-url) | css/less | proposed |
| [html-pre-without-end-tag](#html-pre-without-end-tag) | html | proposed |
| [html-unquoted-attribute-value](#html-unquoted-attribute-value) | html/vue | proposed |
| [html-whitespace-only-pre-textarea](#html-whitespace-only-pre-textarea) | html | proposed |
| [html-ignored-void-without-slash](#html-ignored-void-without-slash) | html | proposed |
| [html-text-after-nul](#html-text-after-nul) | html/vue | proposed |
| [vue-v-pre-interpolation](#vue-v-pre-interpolation) | vue | proposed |
| [the rule in step 2](#process) | all | proposed |
| [tempura and text mode](#beyond-prettier) | handlebars | approved 2026-10-31 |

## Handlebars

### hbs-doctype

- Status: proposed
- Why: Prettier deletes the doctype, because Glimmer has no node for it. Without it the browser renders the page in quirks mode.
- Where: crates/tidyrs/src/handlebars/tokenizer.rs:987 (adds a `Doctype` node), printer.rs:364 (prints it as written)
- Parser: handlebars

```
input:    "<!DOCTYPE html>\n<p   ></p>\n"
Prettier: "<p></p>"
tidyrs:   "<!DOCTYPE html>\n<p></p>"
```

### hbs-dropped-markup-kept-as-written

- Status: proposed
- Why: The HTML tokenizer drops a `<` that does not start a tag, or an unfinished tag at the end, so Prettier loses that text. tidyrs then prints the whole template as written.
- Where: crates/tidyrs/src/handlebars/tokenizer.rs:608, tokenizer.rs:1192 (set `has_lost_text`), mod.rs:282 (prints the template as written)
- Parser: handlebars

```
input:    "a <3 b   ></b>\n"
Prettier: "a <b></b>"
tidyrs:   "a <3 b   ></b>\n"
```

### hbs-html-comment-start-and-end

- Status: proposed
- Why: Prettier loses a character after `<!--`, or moves a mustache out of the comment, so the comment text changes or the mustache renders.
- Where: crates/tidyrs/src/handlebars/tokenizer.rs:720 (dropped character), tokenizer.rs:1052 (mustache at the start or end of a comment); the comment is then printed as written
- Parser: handlebars

```
input:    "<!---a--><p   ></p>\n"
Prettier: "<!-----><p></p>"
tidyrs:   "<!---a--><p></p>"
```

### hbs-escaped-mustache

- Status: proposed
- Why: Prettier drops the backslash of `\{{`, so the text becomes a mustache that Handlebars evaluates.
- Where: crates/tidyrs/src/handlebars/tokenizer.rs:950-969 (comment or tag kept as written), printer.rs:717-720 (`<pre>`), printer.rs:313-335 (ignored text keeps its escapes)
- Parser: handlebars

```
input:    "<!-- \\{{a}} --><p   ></p>\n"
Prettier: "<!-- {{a}} --><p></p>"
tidyrs:   "<!-- \\{{a}} --><p></p>"
```

### hbs-style-with-mustache

- Status: proposed
- Why: In `<style>`, Prettier drops the backslash of `\{{` and the spaces next to a mustache, which changes the CSS that renders (`0{{b}}0` is one token).
- Where: crates/tidyrs/src/handlebars/printer.rs:557-570 (such a `<style>` stays raw text), printer.rs:514
- Parser: handlebars

```
input:    "<style   >a{b:\"\\{{c}}\"}</style>\n"
Prettier: "<style>a{b:\"{{c}}\"}</style>"
tidyrs:   "<style>a{b:\"\\{{c}}\"}</style>"
```

### hbs-no-break-space

- Status: proposed
- Why: Prettier counts U+00A0 as whitespace and deletes it, but it renders as a space. In the component case Prettier also makes `<A>` self-closing, which drops its block content.
- Where: crates/tidyrs/src/handlebars/printer.rs:265-268 (`is_white_space_node` uses ASCII whitespace), mod.rs:244-245 (empty template)
- Parser: handlebars

```
input:    "{{#a   }} {{/a}}\n"
Prettier: "{{#a}}{{/a}}"
tidyrs:   "{{#a}} {{/a}}"
```

### hbs-no-break-space-in-class

- Status: proposed
- Why: HTML splits classes only at ASCII whitespace, so `b\u00a0c` is one class. Prettier splits it into two.
- Where: crates/tidyrs/src/handlebars/printer.rs:686-701
- Parser: handlebars

```
input:    "<a   class=\"b c\"></a>\n"
Prettier: "<a class=\"b c\"></a>"
tidyrs:   "<a class=\"b c\"></a>"
```

### hbs-backslash-before-mustache

- Status: proposed
- Why: `\\{{` is a literal backslash followed by a live mustache. Prettier prints one backslash, which then escapes the mustache.
- Where: crates/tidyrs/src/handlebars/printer.rs:318-330 (text before a comment or block), printer.rs:677-679 (attribute value)
- Parser: handlebars

```
input:    "<a   b=\"c\\\\{{d}}\"></a>\n"
Prettier: "<a b=\"c\\{{d}}\"></a>"
tidyrs:   "<a b=\"c\\\\{{d}}\"></a>"
```

### hbs-both-quotes-in-value

- Status: proposed
- Why: Prettier writes the quote unescaped inside the quoted value, so the value ends early and the rest becomes another attribute.
- Where: crates/tidyrs/src/handlebars/printer.rs:203-210 (writes `&quot;` or `&apos;`)
- Parser: handlebars

```
input:    "<a   b=c\"d'e></a>\n"
Prettier: "<a b=\"c\"d'e\"></a>"
tidyrs:   "<a b=\"c&quot;d'e\"></a>"
```

### hbs-script-textarea-lines

- Status: proposed
- Why: Prettier fills the text of `<script>` and `<textarea>` like prose. In the script that puts code after a `//` comment; in the textarea it changes the value.
- Where: crates/tidyrs/src/handlebars/printer.rs:514-520
- Parser: handlebars

```
input:    "<script   >\n  // a\n  b()\n</script>\n"
Prettier: "<script>\n  // a b()\n</script>"
tidyrs:   "<script>\n  // a\n  b()\n</script>"
```

### hbs-pre-descendant-whitespace

- Status: proposed
- Why: Prettier keeps whitespace only in the direct text of `<pre>`. Text in a nested element is reflowed, which changes what renders.
- Where: crates/tidyrs/src/handlebars/printer.rs:717-720 (`pre_depth`)
- Parser: handlebars

```
input:    "<pre   ><b>a\n  b</b></pre>\n"
Prettier: "<pre><b>a b</b></pre>"
tidyrs:   "<pre><b>a\n  b</b></pre>"
```

### hbs-whitespace-block-in-pre-textarea

- Status: proposed
- Why: Prettier drops a whitespace-only block body even in `<pre>` or `<textarea>`, where the space renders or is the value.
- Where: crates/tidyrs/src/handlebars/printer.rs:966-967
- Parser: handlebars

```
input:    "<textarea   >{{#a}} {{/a}}</textarea>\n"
Prettier: "<textarea>{{#a}}{{/a}}</textarea>"
tidyrs:   "<textarea>{{#a}} {{/a}}</textarea>"
```

### hbs-void-with-children

- Status: proposed
- Why: Prettier treats `<imG>` as the void `<img>` and drops its children. Glimmer treats a name with an upper-case letter as a component.
- Where: crates/tidyrs/src/handlebars/printer.rs:275-281
- Parser: handlebars

```
input:    "<imG   >a</imG>\n"
Prettier: "<imG />"
tidyrs:   "<imG>a</imG>"
```

### hbs-path-round-trip

- Status: proposed
- Why: Prettier prints the path from the parsed parts, which drops `this/`, `@this`, empty brackets and dots, and adds or moves brackets. The result names a different value, or does not parse.
- Where: crates/tidyrs/src/handlebars/parser.rs:544-549 and parser.rs:610-611 (path kept as written), printer.rs:1375-1393 (`path_round_trips`)
- Parser: handlebars

```
input:    "{{this/a   }}\n"
Prettier: "{{a}}"
tidyrs:   "{{this/a}}"
```

### hbs-bracketed-segment

- Status: proposed
- Why: Prettier drops the brackets around a segment that needs them (`[true]`, `[-1]`, `[else]`, `[@b]`, `[b c]`), so it reads as a literal, a keyword, an argument or two names.
- Where: crates/tidyrs/src/handlebars/printer.rs:1375-1393, printer.rs:1497-1512
- Parser: handlebars

```
input:    "{{a   [true]}}\n"
Prettier: "{{a true}}"
tidyrs:   "{{a [true]}}"
```

### hbs-hash-key-in-brackets

- Status: proposed
- Why: Prettier prints `[b c]=d` as `b c=d`, which is a parameter and a different key.
- Where: crates/tidyrs/src/handlebars/parser.rs:378-384
- Parser: handlebars

```
input:    "{{a   [b c]=d}}\n"
Prettier: "{{a b c=d}}"
tidyrs:   "{{a [b c]=d}}"
```

### hbs-dropped-arguments

- Status: proposed
- Why: The parsers silently drop arguments after a literal, and a hash used as a callee, so Prettier deletes code.
- Where: crates/tidyrs/src/handlebars/parser.rs:842-845, parser.rs:416-419 (`has_dropped`, so the mustache is printed as written)
- Parser: handlebars

```
input:    "{{\"a\"   b}}\n"
Prettier: "{{\"a\"}}"
tidyrs:   "{{\"a\"   b}}"
```

### hbs-raw-block

- Status: proposed
- Why: Glimmer turns `{{{{a}}}}` into an ordinary block, so Prettier prints `{{#a}}` and escapes the body, which changes what renders.
- Where: crates/tidyrs/src/handlebars/parser.rs:796-799
- Parser: handlebars

```
input:    "{{{{a}}}}   {{b}} {{{{/a}}}}\n"
Prettier: "{{#a}} \\{{b}} {{/a}}"
tidyrs:   "{{{{a}}}}   {{b}} {{{{/a}}}}"
```

### hbs-number-exponent

- Status: proposed
- Why: Prettier prints `String(Number(x))`, which can be `1e+21` or `1e-7`. The Handlebars lexer does not accept exponents.
- Where: crates/tidyrs/src/handlebars/printer.rs:1356-1371
- Parser: handlebars

```
input:    "{{a   1000000000000000000000}}\n"
Prettier: "{{a 1e+21}}"
tidyrs:   "{{a 1000000000000000000000}}"
```

### hbs-triple-stash-strip

- Status: proposed
- Why: Prettier prints `{{{a~}}}`, which does not parse, and drops the third brace of a modifier.
- Where: crates/tidyrs/src/handlebars/printer.rs:1249-1261, tokenizer.rs:1006-1009
- Parser: handlebars

```
input:    "{{{a   }~}}\n"
Prettier: "{{{a~}}}"
tidyrs:   "{{{a}~}}"
```

### hbs-comment-dashes

- Status: proposed
- Why: Without the `--` form, `{{! a ~}}` reads as a strip mark and `{{!-- a }}` ends at a later `--}}`. Prettier drops the dashes in both cases.
- Where: crates/tidyrs/src/handlebars/printer.rs:380-383
- Parser: handlebars

```
input:    "{{!--   a ~--}}\n"
Prettier: "{{!   a ~}}"
tidyrs:   "{{!--   a ~--}}"
```

### hbs-else-chain

- Status: proposed
- Why: Prettier prints only the head of the name after `else` (`if.b` becomes `if`), and folds `{{else}}{{~#if}}` into `{{else if}}`, which loses the `~` and the inner `{{/if}}`.
- Where: crates/tidyrs/src/handlebars/printer.rs:860-871 (`is_else_if`), printer.rs:921-922 (whole path)
- Parser: handlebars

```
input:    "{{#a   }}{{else if.b c}}{{/a}}\n"
Prettier: "{{#a}}{{else if c}}{{/a}}"
tidyrs:   "{{#a}}{{else if.b c}}{{/a}}"
```

### hbs-line-separator-columns

- Status: proposed
- Why: Prettier slices source text by line and column, and counts U+2028 and U+2029 as line breaks there, so it cuts comments and ignored nodes at the wrong place.
- Where: No single rule: tidyrs slices by byte offset, for example crates/tidyrs/src/handlebars/printer.rs:290-294 (`ignored`)
- Parser: handlebars

```
input:    "a\n {{~! b ~}} <p {{c}} d=\"e\" {{f}} g></p> <!-- {{h}} -->\n {{! prettier-ignore }} {{  i  }} {{!-- j --~}}\n{{k\n  l}} {{~! m }}\n"
Prettier: "a  {{! b }}\n<p {{c}} d=\"e\" {{f}} g></p>\n<!--  {{h} -->\n  {{! prettier-ignore }\n {{  i  }\n{{! j }}\n{{k l}}\n{{! m }}"
tidyrs:   "a  {{~! b ~}}\n<p {{c}} d=\"e\" {{f}} g></p>\n<!-- {{h}} -->\n {{! prettier-ignore }}\n{{  i  }}\n{{! j ~}}\n{{k l}}\n{{~! m }}"
```

### hbs-ws-ignore-escaped-mustache

- Status: proposed
- Why: With `--html-whitespace-sensitivity ignore`, Prettier joins a word and the next escaped mustache (`p\{{a}}`), which changes the text.
- Where: crates/tidyrs/src/handlebars/printer.rs:760 (a text next to a text, from an escaped mustache)
- Parser: handlebars

```
input:    "p \\{{a}} {{q   }}\n"
Prettier: "p\\{{a}} {{q}}"
tidyrs:   "p \\{{a}} {{q}}"
```

## CSS, SCSS and Less

### css-escaped-space-before-semicolon

- Status: proposed
- Why: `\ ` at the end of at-rule params is an escaped space. Prettier trims the space, so the backslash escapes the `;` or `)`.
- Where: crates/tidyrs/src/css/printer.rs:1091-1094 (at-rule), printer.rs:1180-1183 (media feature), printer.rs:766-768 (declaration)
- Parser: scss

```
input:    "a{@extend .a\\ ;}\n"
Prettier: "a {\n  @extend .a\\;\n}\n"
tidyrs:   "a {\n  @extend .a\\ ;\n}\n"
```

### css-escaped-space-in-value

- Status: proposed
- Why: Prettier prints nothing after a word with a backslash, so `Font\ Awesome\ 5` becomes one word, and `\b c` loses the space that ends the hex escape.
- Where: crates/tidyrs/src/css/value_groups.rs:491-505, value_groups.rs:1013-1015, value_groups.rs:1098, value_groups.rs:398
- Parser: css

```
input:    "a{b:Font\\ Awesome\\ 5}\n"
Prettier: "a {\n  b: Font\\Awesome\\5;\n}\n"
tidyrs:   "a {\n  b: Font\\ Awesome\\ 5;\n}\n"
```

### css-selector-without-namespace

- Status: proposed
- Why: `x |b` is a descendant `|b` (no namespace). Prettier prints `x|b` (namespace x), and turns `x,|b` into `x, ,|b`, which does not parse.
- Where: crates/tidyrs/src/css/selector_parser.rs:375-379, selector_parser.rs:397
- Parser: css

```
input:    "x |b{c:d}\n"
Prettier: "x|b {\n  c: d;\n}\n"
tidyrs:   "x |b {\n  c: d;\n}\n"
```

### css-non-ascii-lowercase

- Status: proposed
- Why: Prettier lowercases a media query with `toLowerCase`, which turns U+212A KELVIN SIGN into `k`. CSS is case-insensitive only for ASCII.
- Where: crates/tidyrs/src/css/misc.rs:240-247
- Parser: css

```
input:    "@media (KA){a{b:c}}\n"
Prettier: "@media (ka) {\n  a {\n    b: c;\n  }\n}\n"
tidyrs:   "@media (Ka) {\n  a {\n    b: c;\n  }\n}\n"
```

### less-variable-colon-without-space

- Status: proposed
- Why: postcss reads `@a:1px` as an at-rule name, and Prettier joins the value without the space after it: `1px solid` becomes `1pxsolid`.
- Where: crates/tidyrs/src/css/parse.rs:448-452
- Parser: less

```
input:    "@a:1px solid #ccc;\n"
Prettier: "@a: 1pxsolid #ccc;\n"
tidyrs:   "@a: 1px solid #ccc;\n"
```

### less-variable-comment

- Status: proposed
- Why: Prettier parses the value after postcss has removed comments, then reads comment text at offsets that assume nothing was removed. It deletes comments or prints garbled text such as `b d  c */ d f`.
- Where: crates/tidyrs/src/css/parse.rs:397-400
- Parser: less

```
input:    "@a: 1px /* why */ solid;\n"
Prettier: "@a: 1px solid;\n"
tidyrs:   "@a: 1px /* why */ solid;\n"
```

### css-comment-only-at-rule-params

- Status: proposed
- Why: If the only thing after an at-rule name is a `/* */` comment, Prettier cuts it one character short (`/* why *`), so the comment does not close.
- Where: crates/tidyrs/src/css/parse.rs:753-758
- Parser: scss

```
input:    "@if $a {\n  b: c;\n} @else /* why */ {\n  d: e;\n}\n"
Prettier: "@if $a {\n  b: c;\n} @else  /* why * {\n  d: e;\n}\n"
tidyrs:   "@if $a {\n  b: c;\n} @else /* why */ {\n  d: e;\n}\n"
```

### css-line-comment-before-brace-or-semicolon

- Status: proposed
- Why: When `{` or `;` is on the line after a `//` comment, Prettier moves it onto the comment line, where it becomes part of the comment. Less also blanks quotes and `*` in such comments.
- Where: crates/tidyrs/src/css/parse.rs:787-795 (at-rules), parse.rs:853-858 (`@media` query), parse.rs:424-430 (Less mixin call), parse.rs:666-672 (declaration)
- Parser: scss

```
input:    "@media (min-width: 1px) // Why\n{\n  a {\n    b: c;\n  }\n}\n"
Prettier: "@media (min-width: 1px) // Why {\n  a {\n    b: c;\n  }\n}\n"
tidyrs:   "@media (min-width: 1px) // Why\n{\n  a {\n    b: c;\n  }\n}\n"
```

### css-nth-and-keyframe-sign

- Status: proposed
- Why: Prettier puts a space after a leading `+` in `:nth-*()` and in a keyframe selector (`+ 50%`). Both are then invalid and the browser drops the rule.
- Where: crates/tidyrs/src/css/printer.rs:1371-1387
- Parser: css

```
input:    ":nth-child(+n+3){c:d}\n"
Prettier: ":nth-child(+ n + 3) {\n  c: d;\n}\n"
tidyrs:   ":nth-child(+n + 3) {\n  c: d;\n}\n"
```

### css-hex-escape-space-in-selector

- Status: proposed
- Why: The first space after a hex escape (`\31 `) ends the escape. Prettier collapses `\31  .b` to `\31 .b`, which turns a descendant selector into a compound one.
- Where: crates/tidyrs/src/css/printer.rs:1392-1402
- Parser: css

```
input:    ".\\31  .b{c:d}\n"
Prettier: ".\\31 .b {\n  c: d;\n}\n"
tidyrs:   ".\\31  .b {\n  c: d;\n}\n"
```

### css-attribute-value-escape

- Status: proposed
- Why: Prettier reads `\ ` or the space after a hex escape as the end of the value and the start of a flag: `[a=b\ c]` becomes `[a="b\" c]`, an unterminated string, and `[a=x\31 i]` gets a flag `i`.
- Where: crates/tidyrs/src/css/misc.rs:212-216
- Parser: css

```
input:    "[a=x\\31 i]{c:d}\n"
Prettier: "[a=\"x\\31\" i] {\n  c: d;\n}\n"
tidyrs:   "[a=\"x\\31 i\"] {\n  c: d;\n}\n"
```

### css-hex-escape-space-in-value

- Status: proposed
- Why: `\31 x` is one word and `\31  x` is two. Prettier drops the second space, or breaks the line after `\31`, which splits one word into two.
- Where: crates/tidyrs/src/css/value_groups.rs:472-484
- Parser: css

```
input:    "a{animation-name:\\31  x}\n"
Prettier: "a {\n  animation-name: \\31 x;\n}\n"
tidyrs:   "a {\n  animation-name: \\31  x;\n}\n"
```

### css-author-defined-names

- Status: proposed
- Why: Names inside `@plugin` (Tailwind options) and `@font-feature-values` blocks are case-sensitive. Prettier lowercases them (`className` becomes `classname`).
- Where: crates/tidyrs/src/css/printer.rs:808-815
- Parser: css

```
input:    "@plugin \"@tailwindcss/typography\" {className:wysiwyg}\n"
Prettier: "@plugin \"@tailwindcss/typography\" {\n  classname: wysiwyg;\n}\n"
tidyrs:   "@plugin \"@tailwindcss/typography\" {\n  className: wysiwyg;\n}\n"
```

### css-custom-property-values

- Status: proposed
- Why: Scripts read a custom property's value as written (`getPropertyValue`), and `style()` queries compare it as written. Prettier lowercases colors and units, rewrites numbers, requotes strings and adds spaces (`1,000` becomes `1, 000`).
- Where: crates/tidyrs/src/css/printer.rs:66-72 (`is_custom_property`), applied at printer.rs:1543, 1569, 1581, 1596 and value_groups.rs:243, 963
- Parser: css

```
input:    "a{--x:#FFF}\n"
Prettier: "a {\n  --x: #fff;\n}\n"
tidyrs:   "a {\n  --x: #FFF;\n}\n"
```

### css-custom-property-line-breaks

- Status: proposed
- Why: In a long custom property value, Prettier breaks lines and adds spaces after commas that had none (`1,000` becomes `1, 000`), which changes the value as written.
- Where: crates/tidyrs/src/css/value_groups.rs:1077-1080 (no line after a comma without space), printer.rs:1581
- Parser: css

```
input:    "a {\n  --x: 1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000;\n}\n"
Prettier: "a {\n  --x:\n    1, 000, 1, 000, 1, 000, 1, 000, 1, 000, 1, 000, 1, 000, 1, 000, 1, 000, 1,\n    000, 1, 000, 1, 000, 1, 000, 1, 000, 1, 000, 1, 000;\n}\n"
tidyrs:   "a {\n  --x:\n    1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000,1,000;\n}\n"
```

### css-custom-property-json

- Status: proposed
- Why: `--x: {"a":1}` is JSON for a script. Prettier parses it as a declaration block and adds a `;`, so the JSON no longer parses.
- Where: crates/tidyrs/src/css/parse.rs:624-628
- Parser: css

```
input:    "a{--x:{ \"a\": 1 }}\n"
Prettier: "a {\n  --x: {\n    \"a\":1;\n  };\n}\n"
tidyrs:   "a {\n  --x: { \"a\": 1 };\n}\n"
```

### css-custom-property-url

- Status: proposed
- Why: Prettier treats every `//` as a comment start: `--x: https://a` becomes `--x: https: ; //a`. CSS has no `//` comments, and Less only after a space.
- Where: crates/tidyrs/src/css/parse.rs:702-706
- Parser: css

```
input:    "a{--x:https://Example.com/A;c:d}\n"
Prettier: "a {\n  --x: https: ; //Example.com/A\n  c: d;\n}\n"
tidyrs:   "a {\n  --x: https://Example.com/A;\n  c: d;\n}\n"
```

## HTML and Vue

### html-pre-without-end-tag

- Status: proposed
- Why: With an element inside and no end tag, Prettier prints the contents of `<pre>` or `<listing>` as empty plus a stray `>`. Everything after the start tag is lost.
- Where: crates/tidyrs/src/html/tag.rs:128-136 (`is_verbatim_without_end_tag`), tag.rs:304
- Parser: html

```
input:    "<pre   ><b>a</b>\n"
Prettier: "<pre>>\n"
tidyrs:   "<pre><b>a</b>\n"
```

### html-unquoted-attribute-value

- Status: proposed
- Why: In HTML an unquoted value ends at whitespace or `>`. Prettier (Angular's lexer) ends it at the first `=`, `/` or no-break space, so `src=a.png?v=1` becomes `src="a.png?v" ="1"`. For some valid inputs Prettier fails with a syntax error; tidyrs formats them.
- Where: crates/tidyrs/src/html/lexer.rs:1098-1104 (`End::UnquotedValue` outside Angular)
- Parser: html

```
input:    "<img src=a.png?v=1>\n"
Prettier: "<img src=\"a.png?v\" =\"1\" />\n"
tidyrs:   "<img src=\"a.png?v=1\" />\n"
```

### html-whitespace-only-pre-textarea

- Status: proposed
- Why: Whitespace that is all of a `<pre>` renders, and in a `<textarea>` it is the value. Prettier collapses it to one space or deletes it.
- Where: crates/tidyrs/src/html/utilities.rs:254-258
- Parser: html

```
input:    "<textarea>\n\n</textarea>\n"
Prettier: "<textarea> </textarea>\n"
tidyrs:   "<textarea>\n\n</textarea>\n"
```

### html-ignored-void-without-slash

- Status: proposed
- Why: A void element under `prettier-ignore` without `/>` lends `>` to its neighbor. Prettier assumes `/>` and cuts that many characters from the source, so `src=b>` becomes `src=/>` and `<br>` becomes `<b/>`.
- Where: crates/tidyrs/src/html/tag.rs:120-126 (`is_verbatim_void_without_slash`), tag.rs:112
- Parser: html

```
input:    "<!-- prettier-ignore --><img   src=b><i>c</i>\n"
Prettier: "<!-- prettier-ignore --><img   src=/><i>c</i>\n"
tidyrs:   "<!-- prettier-ignore --><img   src=b><i>c</i>\n"
```

### html-text-after-nul

- Status: proposed
- Why: The parser's end-of-file is code 0, so Prettier silently drops everything after a NUL between tags. tidyrs refuses the file with "Unexpected NUL character" and leaves it unchanged.
- Where: crates/tidyrs/src/html/parser.rs:671, error.rs:21-23, error.rs:113
- Parser: html

```
input:    "<p   a>b</p>\n<p>c\u0000d</p>\n<p>e</p>\n"
Prettier: "<p a>b</p>\n<p>c</p>\n"
tidyrs:   "ERROR(2): tidyrs: Unexpected NUL character | stdout=\"\""
```

### vue-v-pre-interpolation

- Status: proposed
- Why: Vue does not compile anything under `v-pre`, so `{{name}}` renders as that text. Prettier formats it as an expression (`{{ name }}`), which changes the rendered text.
- Where: crates/tidyrs/src/html/preprocess.rs:205-208
- Parser: vue

```
input:    "<template>\n  <p v-pre>Use {{msg}} and {{a+b}} here</p>\n</template>\n"
Prettier: "<template>\n  <p v-pre>Use {{ msg }} and {{ a + b }} here</p>\n</template>\n"
tidyrs:   "<template>\n  <p v-pre>Use {{msg}} and {{a+b}} here</p>\n</template>\n"
```

Only visible when the embed hook formats JavaScript, so `tests/deviations.json` doesn't hold it. It was checked through the napi binding with a hook backed by Prettier, as `tests/oracle/conformance.ts` does.

## Beyond Prettier

Prettier rejects tempura templates, and formats a template of another language as HTML. These decisions are Luke's, from 2026-10-31.

- Status: approved 2026-10-31
- Every `.hbs` file goes through one formatter: Glimmer's parser and printer, with tempura's syntax added. The dialect is tempura when a template uses syntax that only tempura has, or when its tags aren't Handlebars at all.
- Tempura values print with a space inside the braces (`{{ value }}`, `{{{ value }}}`). Handlebars prints `{{value}}`, as Prettier does.
- Tempura's JavaScript (values, conditions, `#each` lists, `#var` values) goes to the embed hook as an expression. The arguments of a custom block stay `key=value` as written.
- `#each` names print as `as item, index`, without parentheses, with a space after the comma.
- Tempura comments print as written. Handlebars' rule for `{{!-- --}}` applies to Handlebars only.
- A template of another language than HTML (`robots.txt.hbs`, `sitemap.xml.hbs`) keeps its text as written. Only the block tags that are alone on their lines are indented by depth, which doesn't change what renders.

## Raised, not in the code

### html-borrowed-markers-once

- Status: raised 2026-10-31, awaiting sign-off
- Why: Where an element's contents print as written, Prettier prints the markers that its tags borrow from the elements next to them twice: once on their own, and once in the tag. The extra `>` or `/>` is text on the page.
- Where it would go: `crates/tidyrs/src/html/print.rs`, `print_element`, which prints them once only for a whitespace-only `<pre>` or `<textarea>` now. bun prints them once always (`src/format/html/printer.rs:524`, and `:335` for `<!-- prettier-ignore -->`).
- Parser: html, with `--whitespace strict`

```
input:    "<b>a</b><pre   ><i>x</i>\n"
Prettier: "<b>a</b>><pre><i>x</i>\n"
tidyrs:   "<b>a</b>><pre><i>x</i>\n"
bun:      "<b>a</b><pre><i>x</i>\n"
```
