// Prints random docs with Prettier's printer and with ours, and reports the first that differ.
//
//   bun tests/oracle/doc.ts [cases=2000] [seed=1]
import { $ } from "bun";
import { builders as b, printer } from "prettier/doc";

type Json = string | Json[] | { [key: string]: Json | number | boolean };

let count = Number(Bun.argv[2] ?? 2000);
let seed = Number(Bun.argv[3] ?? 1);

function random() {
	seed = (seed * 1103515245 + 12345) & 0x7fffffff;
	return seed / 0x7fffffff;
}

function pick<T>(list: readonly T[]): T {
	return list[Math.floor(random() * list.length)];
}

const WORDS = ["a", "bb", "ccc", "dddd", "eeeee", "ffffff", " ", "  x", "y  ", "日本", "\t", ""];

function generate(depth: number, defs: number): Json {
	if (depth <= 0 || random() < 0.25) {
		let r = random();
		if (r < 0.5) return pick(WORDS);
		if (r < 0.65) return { t: "line" };
		if (r < 0.78) return { t: "softline" };
		if (r < 0.84) return { t: "hardline" };
		if (r < 0.86) return { t: "literalline" };
		if (r < 0.88) return { t: "bp" };
		if (r < 0.9) return { t: "trim" };
		if (r < 0.92) return { t: "lsb" };
		if (defs > 0) return { t: "ref", i: Math.floor(random() * defs) };
		return pick(WORDS);
	}
	let next = () => generate(depth - 1, defs);
	let r = random();
	if (r < 0.25) return Array.from({ length: 1 + Math.floor(random() * 4) }, next);
	if (r < 0.45) {
		let group: Json = { t: "group", c: next() };
		if (random() < 0.1) group.b = true;
		if (random() < 0.3) group.id = Math.floor(random() * 4);
		return group;
	}
	if (r < 0.5) return { t: "group", s: Array.from({ length: 2 + Math.floor(random() * 2) }, next) };
	if (r < 0.58) {
		let parts: Json[] = [];
		let n = 1 + Math.floor(random() * 6);
		for (let i = 0; i < n; i++) {
			if (i > 0) parts.push(pick<Json>([{ t: "line" }, { t: "softline" }, { t: "hardline" }]));
			parts.push(next());
		}
		return { t: "fill", p: parts };
	}
	if (r < 0.68) return { t: "indent", c: next() };
	if (r < 0.74) return { t: "align", n: pick<Json | number>([1, 2, 3, -1, "root", "dedentToRoot", "> "]) as Json, c: next() };
	if (r < 0.84) {
		let doc: Json = { t: "ifBreak", b: next(), f: next() };
		if (random() < 0.5) doc.id = Math.floor(random() * 4);
		return doc;
	}
	if (r < 0.88) return { t: "indentIfBreak", c: next(), id: Math.floor(random() * 4), neg: random() < 0.3 };
	if (r < 0.94) return { t: "lineSuffix", c: next() };
	return next();
}

function toPrettier(json: Json, ids: Map<number, symbol>, defs: unknown[]): unknown {
	if (typeof json === "string") return json;
	if (Array.isArray(json)) return json.map((part) => toPrettier(part, ids, defs));
	let to = (value: Json) => toPrettier(value, ids, defs);
	let id = (value: unknown) => {
		if (typeof value !== "number") return undefined;
		if (!ids.has(value)) ids.set(value, Symbol(String(value)));
		return ids.get(value);
	};
	switch (json.t) {
		case "ref":
			return defs[json.i as number];
		case "group":
			if (json.s) return b.conditionalGroup((json.s as Json[]).map(to) as never);
			return b.group(to(json.c as Json) as never, { shouldBreak: Boolean(json.b), id: id(json.id) });
		case "fill":
			return b.fill((json.p as Json[]).map(to) as never);
		case "indent":
			return b.indent(to(json.c as Json) as never);
		case "align": {
			let n = json.n;
			let contents = to(json.c as Json) as never;
			if (n === "root") return b.markAsRoot(contents);
			if (n === "dedentToRoot") return b.dedentToRoot(contents);
			return b.align(n as never, contents);
		}
		case "ifBreak":
			return b.ifBreak(to(json.b as Json) as never, to(json.f as Json) as never, { groupId: id(json.id) });
		case "indentIfBreak":
			return b.indentIfBreak(to(json.c as Json) as never, { groupId: id(json.id)!, negate: Boolean(json.neg) });
		case "lineSuffix":
			return b.lineSuffix(to(json.c as Json) as never);
		case "lsb":
			return b.lineSuffixBoundary;
		case "line":
			return b.line;
		case "softline":
			return b.softline;
		case "hardline":
			return b.hardline;
		case "literalline":
			return b.literalline;
		case "bp":
			return b.breakParent;
		case "trim":
			return b.trim;
	}
	throw new Error(`unknown ${json.t}`);
}

let cases = [];
let expected: string[] = [];
for (let i = 0; i < count; i++) {
	let defs: Json[] = [];
	let prettierDefs: unknown[] = [];
	let ids = new Map<number, symbol>();
	let n = Math.floor(random() * 3);
	for (let d = 0; d < n; d++) {
		let def = generate(3, d);
		defs.push(def);
		prettierDefs.push(toPrettier(def, ids, prettierDefs));
	}
	let doc = generate(6, n);
	let width = pick([0, 4, 10, 20, 40, 80]);
	let tabWidth = pick([2, 4]);
	let useTabs = random() < 0.3;
	cases.push({ doc, defs, width, tabWidth, useTabs });
	try {
		let printed = printer.printDocToString(toPrettier(doc, ids, prettierDefs) as never, {
			printWidth: width,
			tabWidth,
			useTabs,
		} as never);
		expected.push(printed.formatted);
	} catch (error) {
		expected.push(`!error ${(error as Error).message}`);
	}
}

let root = new URL("../..", import.meta.url).pathname;
let actual: string[] = await $`cargo run -q -p tidyrs --example doc < ${new Response(JSON.stringify(cases))}`
	.cwd(root)
	.env({ ...process.env, CARGO_TARGET_DIR: `${root}target` })
	.json();

let failed = 0;
for (let i = 0; i < count; i++) {
	if (expected[i].startsWith("!error")) continue;
	if (actual[i] === expected[i]) continue;
	failed++;
	if (failed <= 3) {
		console.log(`case ${i}: ${JSON.stringify(cases[i])}`);
		console.log(`prettier: ${JSON.stringify(expected[i])}`);
		console.log(`tidyrs:   ${JSON.stringify(actual[i])}`);
	}
}
let errors = expected.filter((text) => text.startsWith("!error")).length;
console.log(`${count - failed - errors} same, ${failed} different, ${errors} rejected by prettier`);
process.exit(failed > 0 ? 1 : 0);
