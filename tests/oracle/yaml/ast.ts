// Writes the trees that Prettier's YAML parser builds, with their comments, from the YAML Test Suite and the YAML files
// under a folder, for `cargo run -- ast` to compare with tidy's.
//
//   bun tests/oracle/yaml/ast.ts [<folder>] > target/yaml-ast.json
import { $ } from "bun";
import { homedir } from "node:os";
import { join } from "node:path";
import * as prettier from "prettier";
import suite from "yaml-test-suite";

let folder = Bun.argv[2] ?? join(homedir(), "repos", "oss");
let inputs: { name: string; text: string }[] = [];
for (let test of suite) {
	for (let [index, entry] of test.cases.entries()) inputs.push({ name: `suite/${test.id}/${index}`, text: entry.yaml });
}
let listed = await $`fd --type f --no-ignore --exclude node_modules --exclude target --exclude .git -e yml -e yaml . ${folder}`
	.nothrow()
	.quiet()
	.text();
let seen = new Set<string>();
for (let path of listed.split("\n").filter(Boolean)) {
	let text = await Bun.file(path).text().catch(() => undefined);
	if (text === undefined || text.length > 500_000 || seen.has(text)) continue;
	seen.add(text);
	inputs.push({ name: path.slice(folder.length + 1), text });
}

let point = (p: any) => [p.line, p.column, p.offset];
function dump(node: any): any {
	if (!node) return null;
	let out: any = { type: node.type, start: point(node.position.start), end: point(node.position.end) };
	if (node.type === "directive") {
		out.value = node.name;
		out.parameters = node.parameters;
	} else if (typeof node.value === "string") out.value = node.value;
	if (node.children) out.children = node.children.map(dump);
	for (let key of ["leadingComments", "endComments", "middleComments"]) if (node[key]?.length) out[key] = node[key].map(dump);
	for (let key of ["trailingComment", "indicatorComment", "tag", "anchor"]) if (node[key]) out[key] = dump(node[key]);
	if ("chomping" in node) out.chomping = node.chomping;
	if ("indent" in node && node.type.startsWith("block")) out.indent = node.indent;
	if (node.directivesEndMarker) out.directivesEndMarker = true;
	if (node.documentEndMarker) out.documentEndMarker = true;
	return out;
}

let out = [];
for (let { name, text } of inputs) {
	try {
		let { ast } = await prettier.__debug.parse(text, { parser: "yaml" });
		out.push({ name, text, ast: dump(ast), comments: (ast as any).comments?.map(dump) ?? [] });
	} catch {}
}
console.log(JSON.stringify(out));
