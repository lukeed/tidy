// A naive formatter on @lukeed/html, as a floor for what a JavaScript formatter costs: it parses each file, puts
// block elements on their own lines with two-space indents, collapses whitespace in text, and keeps the rest as written.
//
//   bun bench/naive.ts <dir> [--threads <n>]
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

function check(files: string[]) {
	let changed = 0;
	for (let file of files) {
		let source = readFileSync(file, "utf8");
		if (format(source) !== source) changed++;
	}
	return changed;
}

if (Bun.isMainThread) {
	let args = Bun.argv.slice(2);
	let at = args.indexOf("--threads");
	let threads = at >= 0 ? Number(args.splice(at, 2)[1]) : navigator.hardwareConcurrency;
	let files: string[] = [];
	for (let arg of args) walk(arg, files);
	threads = Math.max(1, Math.min(threads, files.length));
	let changed = 0;
	if (threads === 1) {
		changed = check(files);
	} else {
		// Each worker takes every nth file, which spreads large files out as well as a queue would here.
		let counts = await Promise.all(
			Array.from({ length: threads }, (_, index) => {
				let worker = new Worker(import.meta.url);
				let mine = files.filter((_, at) => at % threads === index);
				return new Promise<number>((resolve) => {
					worker.onmessage = (event) => {
						resolve(event.data as number);
						worker.terminate();
					};
					worker.postMessage(mine);
				});
			}),
		);
		changed = counts.reduce((sum, count) => sum + count, 0);
	}
	process.exit(changed > 0 ? 1 : 0);
} else {
	self.onmessage = (event: MessageEvent<string[]>) => postMessage(check(event.data));
}
