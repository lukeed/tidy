import { describe, expect, test } from "bun:test";

import { check, format, type EmbedRequest } from "./index.js";

let js = { parser: "babel" } as const;

describe("format", () => {
	test("finds the language by the file's path", () => {
		let cases = [
			["a.js", "let a = {b:1}", "let a = { b: 1 };\n"],
			["a.ts", "let a:number=1", "let a: number = 1;\n"],
			["a.jsx", "<div   className='x'/>", '<div className="x" />;\n'],
			["a.json", '{"a":1,"b":[1,2]}', '{ "a": 1, "b": [1, 2] }\n'],
			["a.css", "a{color:red}", "a {\n  color: red;\n}\n"],
			["a.scss", "a{b{c:d}}", "a {\n  b {\n    c: d;\n  }\n}\n"],
			["a.less", "@a:1;a{b:@a}", "@a: 1;\na {\n  b: @a;\n}\n"],
			["a.html", "<div  ><p>hi</p></div>", "<div><p>hi</p></div>\n"],
			["a.vue", "<template><p   >hi</p></template>", "<template><p>hi</p></template>\n"],
			["a.hbs", "{{#if   a}}<p>x</p>{{/if}}", "{{#if a}}<p>x</p>{{/if}}"],
			["a.yaml", "a:   1\nb:\n    - 2", "a: 1\nb:\n  - 2\n"],
			["a.toml", "a=1\n[b]\nc  =  'x'", "a = 1\n[b]\nc = 'x'\n"],
			["a.graphql", "query{a{b}}", "query {\n  a {\n    b\n  }\n}\n"],
			["a.xml", "<a><b>c</b></a>", "<a><b>c</b></a>\n"],
			["Dockerfile", "from node\nrun echo hi", "FROM node\nRUN echo hi\n"],
			["a.ini", "[a]\nb=c", "[a]\nb = c\n"],
		] as const;
		for (let [filepath, source, output] of cases) expect(format(source, { filepath })).toBe(output);
	});

	test("takes the parser over the path, and HTML without either", () => {
		expect(format("a{color:red}", { filepath: "a.txt", parser: "css" })).toBe("a {\n  color: red;\n}\n");
		expect(format("<p   >x</p>")).toBe("<p>x</p>\n");
	});

	test("applies the shared options", () => {
		let list = "let a = [1111111111, 2222222222, 3333333333];";
		expect(format(list, { ...js, width: 30 })).toBe("let a = [\n  1111111111, 2222222222,\n  3333333333,\n];\n");
		expect(format(list, { ...js, width: 30, trailingComma: false })).toBe(
			"let a = [\n  1111111111, 2222222222,\n  3333333333\n];\n",
		);
		expect(format("if (a) { b() }", { ...js, tabs: true })).toBe("if (a) {\n\tb();\n}\n");
		expect(format("if (a) { b() }", { ...js, indent: 4 })).toBe("if (a) {\n    b();\n}\n");
		expect(format("a;\nb;", { ...js, eol: "crlf" })).toBe("a;\r\nb;\r\n");
		expect(format('let a = "b"', { ...js, quote: "single" })).toBe("let a = 'b';\n");
		expect(format("let a = {b: 1}", { ...js, bracketSpacing: false })).toBe("let a = {b: 1};\n");
	});

	test("lets a language's section override the shared options", () => {
		expect(format('let a = "b"', { ...js, quote: "single", js: { semi: false } })).toBe("let a = 'b'\n");
		expect(format('let a = "b"', { ...js, quote: "single", js: { quote: "double" } })).toBe('let a = "b";\n');
		expect(format("a{content:'x'}", { parser: "css", css: { quote: "single" } })).toBe("a {\n  content: 'x';\n}\n");
	});

	test("throws on code that doesn't parse", () => {
		expect(() => format("let = ;", js)).toThrow("Unexpected token");
	});

	test("rejects unknown parsers, keys and values", () => {
		expect(() => format("x", { parser: "nope" as never })).toThrow("unknown parser: nope");
		expect(() => format("x", { ...js, eol: "lfcr" as never })).toThrow('`eol` must be "lf", "crlf" or "cr"');
		expect(() => format("x", { ...js, js: { semicolons: false } })).toThrow("unknown option `js.semicolons`");
		expect(() => format("x", { ...js, js: { semi: "no" } })).toThrow("`js.semi` must be true or false");
	});
});

describe("embed", () => {
	test("asks the hook for each piece of code in another language", () => {
		let requests: EmbedRequest[] = [];
		let html = '<div><script>let  a</script><style>a{b:c}</style><p style="color:red">x</p></div>';
		format(html, { parser: "html" }, (request) => void requests.push(request));
		let common = { prefix: 0, suffix: 0, width: 80 };
		expect(requests).toEqual([
			{ ...common, code: "let  a", language: "babel", kind: "block", indent: 2, module: false },
			{ ...common, code: "a{b:c}", language: "css", kind: "block", indent: 2, module: true },
			{ ...common, code: "color:red", language: "css", kind: "attribute", indent: 0, module: true },
		]);
	});

	test("prints what the hook returns", () => {
		let vue = '<template><p :a="b  +  c">{{ x  +  y }}</p></template>\n<script setup lang="ts">let  a</script>';
		let output = format(vue, { parser: "vue" }, (request) => request.code.replace(/\s+/g, " ").trim());
		expect(output).toBe('<template>\n  <p :a="b + c">{{ x + y }}</p>\n</template>\n<script setup lang="ts">\nlet a\n</script>\n');
	});

	test("formats what the hook declines itself, unless embedded is off", () => {
		expect(format("<style>a{b:c}</style>", { parser: "html" })).toBe("<style>\n  a {\n    b: c;\n  }\n</style>\n");
		expect(format("<style>a{b:c}</style>", { parser: "html", embedded: false })).toBe("<style>\n  a{b:c}\n</style>\n");
	});

	test("throws what the hook throws", () => {
		let hook = () => {
			throw new Error("boom");
		};
		expect(() => format("<style>a{b:c}</style>", { parser: "html" }, hook)).toThrow("boom");
	});
});

describe("check", () => {
	test("tells whether a script is formatted", () => {
		expect(check("let a = 1;\n", js)).toBe(true);
		expect(check("let a=1", js)).toBe(false);
	});
});
