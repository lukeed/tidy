// Formats HTML, Vue and Markdown files with Prettier, embedded code included, and with tidy twice: with a hook that
// formats the JavaScript and TypeScript with Prettier, as a host of the Node binding can, and without one, where tidy
// formats them itself. Every other embedded language is tidy's both times. Counts the files that match Prettier each
// way, and shows the ones that only match through the hook.
//
//   bun tests/oracle/embedded.ts [dir...] [--show=n] [--list=file]
//
// Without folders, it reads the HTML and Vue files of the bench corpora (see bench/run.ts), and the Markdown files of
// target/corpus/markdown that have a JavaScript or TypeScript code block.
import { $ } from "bun";
import { readdirSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { extname, join } from "node:path";
import * as prettier from "prettier";
import { format, type EmbedRequest } from "../../crates/tidy_napi/index.js";

let args = Bun.argv.slice(2).filter((arg) => !arg.startsWith("--"));
let option = (name: string) => Bun.argv.find((arg) => arg.startsWith(`--${name}=`))?.slice(name.length + 3);
let show = Number(option("show") ?? 3);
let list = option("list");
let root = new URL("../..", import.meta.url).pathname;
let markup = /\.(html|vue|md)$/;
let script = /^\s*(`{3,}|~{3,})\s*(js|jsx|ts|tsx|javascript|typescript|mjs|cjs|mts|cts)\b/m;

async function corpus(): Promise<string[]> {
	let oss = join(homedir(), "repos/oss");
	let sources = [
		...["vite", "storybook"].map((repo) => [repo, "*.html"]),
		...["vite", "storybook", "npmx.dev"].map((repo) => [repo, "*.vue"]),
	];
	let paths: string[] = [];
	for (let [repo, pattern] of sources) {
		let listed = await $`git -C ${join(oss, repo)} ls-files -z -- ${pattern}`.nothrow().text();
		for (let file of listed.split("\0").filter(Boolean)) {
			if (/node_modules|\/dist\/|\/build\/|(^|\/)\./.test(file)) continue;
			paths.push(join(oss, repo, file));
		}
	}
	return [...paths, ...files(join(root, "target/corpus/markdown"))];
}

function files(path: string): string[] {
	try {
		if (statSync(path).isFile()) return markup.test(path) ? [path] : [];
		return readdirSync(path).flatMap((name) => (name === "node_modules" ? [] : files(join(path, name))));
	} catch {
		return [];
	}
}

let javascript = /^(babel|typescript|babel-ts|__(js|ts|vue|vue_ts)_[a-z_]+)$/;

/** What Prettier prints for `request`, inside its indents. */
async function prettierHook(request: EmbedRequest): Promise<string | undefined> {
	let tab = "  ";
	let indent = (text: string) =>
		text
			.split("\n")
			.map((line, index) => (index === 0 || line === "" ? line : tab.repeat(request.indent) + line))
			.join("\n");
	try {
		// Prettier's HTML printer passes options for these that no caller can, so the declaration is formatted.
		let binding = /^__vue(_ts)?_(bindings|for_left|generic)$/.exec(request.language);
		if (binding) {
			let generic = binding[2] === "generic";
			let [code, start, end] = generic
				? [`type T<${request.code}> = any`, "type T<", "> = any;"]
				: [`function _(${request.code}) {}`, "function _(", ") {}"];
			let parser = binding[1] || generic ? "babel-ts" : "babel";
			let formatted = (await prettier.format(code, { parser, printWidth: 2 ** 31 - 1 })).trim();
			if (!formatted.startsWith(start) || !formatted.endsWith(end)) return undefined;
			let inner = formatted.slice(start.length, -end.length);
			return indent(binding[2] === "for_left" && inner.includes(",") ? `(${inner})` : inner);
		}
		let settings = {
			parser: request.language,
			printWidth: request.width,
			tabWidth: 2,
			useTabs: false,
			...(request.kind === "attribute" ? { singleQuote: true } : {}),
			...(request.module ? {} : { filepath: "script.cjs" }),
		};
		let { builders, printer } = prettier.doc;
		let doc: prettier.Doc = [builders.hardline, await prettier.__debug.printToDoc(request.code, settings)];
		for (let level = 0; level < request.indent; level++) doc = builders.indent(doc);
		let formatted = printer.printDocToString(doc, settings as prettier.doc.printer.Options).formatted;
		formatted = formatted.replace(/^\n[\t ]*/, "").replace(/\s+$/, "");
		// `__isHtmlInlineEventHandler`: no semicolon after a single expression statement.
		if (request.kind === "attribute" && request.language === "babel") {
			let { ast } = await prettier.__debug.parse(request.code, { parser: "babel" });
			let program = (ast as { program: { body: { type: string }[]; directives: unknown[] } }).program;
			let single = program.body.length === 1 && program.body[0].type === "ExpressionStatement";
			if (single && program.directives.length === 0) formatted = formatted.replace(/;$/, "");
		}
		return formatted;
	} catch {
		return undefined;
	}
}

/** tidy's output with the hook, which some requests only make after an earlier answer. */
async function withHook(text: string, filepath: string) {
	let answers = new Map<string, string | undefined>();
	let output = "";
	for (let round = 0; round < 8; round++) {
		let missing = new Map<string, EmbedRequest>();
		output = format(text, { filepath }, (request) => {
			if (!javascript.test(request.language)) return undefined;
			let id = JSON.stringify(request);
			if (!answers.has(id)) missing.set(id, request);
			return answers.get(id);
		});
		if (missing.size === 0) break;
		for (let [id, request] of missing) answers.set(id, await prettierHook(request));
	}
	return output;
}

let attempt = async (run: () => string | Promise<string>) => {
	try {
		return await run();
	} catch {
		return undefined;
	}
};

let paths = args.length ? args.flatMap(files) : await corpus();
let counts: Record<string, { files: number; hook: number; builtin: number }> = {};
let onlyHook: string[] = [];
let shown = 0;
for (let path of paths) {
	let text = await Bun.file(path).text();
	if (path.endsWith(".md") && !script.test(text)) continue;
	let expected = await attempt(() => prettier.format(text, { filepath: path }));
	if (expected === undefined) continue;
	let hooked = await attempt(() => withHook(text, path));
	// Prettier runs without prettier-plugin-jsdoc.
	let builtin = await attempt(() => format(text, { filepath: path, js: { jsdoc: false } }));
	let count = (counts[extname(path)] ??= { files: 0, hook: 0, builtin: 0 });
	count.files++;
	if (hooked === expected) count.hook++;
	if (builtin === expected) count.builtin++;
	if (builtin === expected || hooked !== expected) continue;
	onlyHook.push(path);
	if (shown++ >= show) continue;
	console.log(`\n✗ ${path}`);
	await Bun.write("/tmp/tidy-expected", expected);
	await Bun.write("/tmp/tidy-actual", builtin ?? "!error");
	console.log((await $`diff -u /tmp/tidy-expected /tmp/tidy-actual`.nothrow().text()).split("\n").slice(2).join("\n"));
}
if (list) await Bun.write(list, `${onlyHook.join("\n")}\n`);
console.log();
for (let [extension, { files, hook, builtin }] of Object.entries(counts)) {
	console.log(`${extension}: ${files} files, ${hook} match Prettier through the hook, ${builtin} without it`);
}
console.log(`${onlyHook.length} only match through the hook`);
