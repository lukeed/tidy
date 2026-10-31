// A naive formatter on @lukeed/html, as a floor for what a JavaScript formatter costs: it parses each file, puts
// block elements on their own lines with two-space indents, collapses whitespace in text, and keeps the rest as written.
//
//   bun bench/naive.ts <dir>
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { parse, stringify } from "@lukeed/html";

type Node = ReturnType<typeof parse>["children"][number];

const BLOCKS = new Set(
	"address article aside blockquote body dd details dialog div dl dt fieldset figcaption figure footer form h1 h2 h3 h4 h5 h6 head header hr html li link main meta nav ol p pre script section style table tbody td tfoot th thead title tr ul".split(
		" ",
	),
);
const RAW = new Set(["pre", "script", "style", "textarea"]);

function format(source: string) {
	let out: string[] = [];
	let print = (node: Node, depth: number) => {
		let indent = "  ".repeat(depth);
		if ("name" in node) {
			let source = stringify(node);
			if (RAW.has(node.name) || !BLOCKS.has(node.name)) {
				out.push(indent + source.replace(/\s+/g, " ").trim());
				return;
			}
			let open = source.slice(0, source.indexOf(">") + 1).replace(/\s+/g, " ");
			out.push(indent + open);
			for (let child of node.children) print(child, depth + 1);
			out.push(`${indent}</${node.name}>`);
			return;
		}
		let text = stringify(node).replace(/\s+/g, " ").trim();
		if (text) out.push(indent + text);
	};
	for (let node of parse(source).children) print(node, 0);
	return `${out.join("\n")}\n`;
}

function walk(path: string, files: string[]) {
	if (statSync(path).isFile()) return files.push(path);
	for (let name of readdirSync(path)) walk(join(path, name), files);
}

let files: string[] = [];
for (let arg of Bun.argv.slice(2)) walk(arg, files);
let changed = 0;
for (let file of files) {
	let source = readFileSync(file, "utf8");
	if (format(source) !== source) changed++;
}
process.exit(changed > 0 ? 1 : 0);
