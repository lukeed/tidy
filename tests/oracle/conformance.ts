// Formats Prettier's own fixtures and compares the output with its snapshots. Embedded code is formatted by Prettier
// through the hook, so only the markup printing is under test.
//
//   bun tests/oracle/conformance.ts [language=html] [filter] [--show=n]
import { $, Glob } from "bun";
import { join, relative } from "node:path";
import * as prettier from "prettier";
import { format, type EmbedRequest, type FormatOptions } from "../../crates/tidyrs_napi/index.js";

let args = Bun.argv.slice(2).filter((arg) => !arg.startsWith("--"));
let show = Number(Bun.argv.find((arg) => arg.startsWith("--show="))?.slice(7) ?? 5);
let language = args[0] ?? "html";
let filter = args[1] ?? "";
let root = join(process.env.HOME!, "repos/oss/prettier/tests/format", language);

type Case = { name: string; file: string; options: Record<string, unknown>; input: string; output: string };

function unescape(text: string) {
	return text.replace(/\\([\\`$])/g, "$1");
}

function parseOptions(text: string) {
	let options: Record<string, unknown> = {};
	for (let line of text.split("\n")) {
		let match = line.match(/^(\w+): (.+)$/);
		if (match) options[match[1]] = match[2] === "Infinity" ? 2 ** 31 - 1 : JSON.parse(match[2]);
	}
	return options;
}

function cases(snapshot: string, directory: string): Case[] {
	let list: Case[] = [];
	let pattern = /exports\[`(.+?) format 1`\] = `\n([\s\S]*?)\n`;\n/g;
	for (let match of snapshot.matchAll(pattern)) {
		let body = unescape(match[2]);
		let parts = body.split(/^=+(options|input|output)=+$/m);
		let options = parseOptions(parts[2] ?? "");
		let input = (parts[4] ?? "").replace(/^\n/, "").replace(/\n$/, "");
		let output = (parts[6] ?? "").replace(/^\n/, "").replace(/\n=+$/, "");
		let name = match[1].replace(/ - \{.*$/, "");
		let file = name.split(" ")[0];
		list.push({ name: `${directory}/${match[1]}`, file, options, input, output });
	}
	return list;
}

function settings(options: Record<string, unknown>, file: string): FormatOptions | undefined {
	let parsers = options.parsers as string[];
	let parser = parsers?.[0];
	if (parser !== "html" && parser !== "vue") return undefined;
	return {
		parser,
		width: (options.printWidth as number) ?? 80,
		tabWidth: (options.tabWidth as number) ?? 2,
		useTabs: (options.useTabs as boolean) ?? false,
		whitespace: (options.htmlWhitespaceSensitivity as "css") ?? "css",
		singleAttributePerLine: (options.singleAttributePerLine as boolean) ?? false,
		bracketSameLine: (options.bracketSameLine as boolean) ?? false,
		vueIndentScriptAndStyle: (options.vueIndentScriptAndStyle as boolean) ?? false,
		filepath: file,
	};
}

function key(request: EmbedRequest) {
	return JSON.stringify([request.code, request.language, request.kind, request.indent, request.width]);
}

/** Parameter lists, which Prettier formats with options that only its HTML printer can pass. */
async function parameters(request: EmbedRequest, extra: Record<string, unknown>, width: number) {
	let ts = request.language.includes("_ts_");
	let wrap = request.language.endsWith("generic")
		? (code: string) => [`type T<${code}> = any`, "type T<", "> = any;"]
		: (code: string) => [`function _(${code}) {}`, "function _(", ") {}"];
	let [code, start, end] = wrap(request.code);
	let formatted = (await prettier.format(code, { ...extra, parser: ts || request.language.endsWith("generic") ? "babel-ts" : "babel", printWidth: 2 ** 31 - 1 })).trim();
	if (!formatted.startsWith(start) || !formatted.endsWith(end)) return undefined;
	let inner = formatted.slice(start.length, -end.length);
	let many = inner.includes(",");
	let text = request.language.includes("for_left") && many ? `(${inner})` : inner;
	return indent(text, request);
}

let tab = "  ";

function indent(text: string, request: EmbedRequest) {
	let prefix = tab.repeat(request.indent);
	return text
		.split("\n")
		.map((line, index) => (index === 0 || line === "" ? line : prefix + line))
		.join("\n");
}

async function embedded(request: EmbedRequest, options: FormatOptions, extra: Record<string, unknown>) {
	if (/__vue(_ts)?_(bindings|for_left|generic)$/.test(request.language)) {
		try {
			return await parameters(request, extra, request.width);
		} catch {
			return undefined;
		}
	}
	let tab = options.useTabs ? "\t" : " ".repeat(options.tabWidth ?? 2);
	let width = (request.width ?? 80) - request.indent * (options.tabWidth ?? 2);
	let handler = request.kind === "attribute" && request.language === "babel";
	try {
		let formatted = await prettier.format(request.code, {
			...extra,
			...(request.kind === "attribute" ? { singleQuote: true } : {}),
			parser: request.language,
			printWidth: Math.max(width, 0),
			tabWidth: options.tabWidth,
			useTabs: options.useTabs,
		});
		formatted = formatted.replace(/\n+$/, "");
		// Prettier's `__isHtmlInlineEventHandler`, which only the HTML printer can pass: no semicolon after one statement.
		if (handler) {
			let { ast } = await prettier.__debug.parse(request.code, { parser: "babel" });
			let body = (ast as { program: { body: { type: string }[] } }).program.body;
			if (body.length === 1 && body[0].type === "ExpressionStatement") formatted = formatted.replace(/;$/, "");
		}
		let indent = tab.repeat(request.indent);
		return formatted
			.split("\n")
			.map((line, index) => (index === 0 || line === "" ? line : indent + line))
			.join("\n");
	} catch {
		return undefined;
	}
}

let all: Case[] = [];
for await (let path of new Glob("**/__snapshots__/format.test.js.snap").scan(root)) {
	let directory = relative(root, join(root, path, "../.."));
	let snapshot = await Bun.file(join(root, path)).text();
	all.push(...cases(snapshot, directory || "."));
}

let passed = 0;
let failed: { name: string; expected: string; actual: string }[] = [];
let skipped = 0;
for (let test of all) {
	if (filter && !test.name.includes(filter)) continue;
	let options = settings(test.options, test.file);
	if (!options || "cursorOffset" in test.options || test.input.includes("<|>") || test.name.startsWith("with-plugins/")) {
		skipped++;
		continue;
	}
	let extra = Object.fromEntries(
		Object.entries(test.options).filter(([name]) =>
			["semi", "singleQuote", "trailingComma", "bracketSpacing", "arrowParens", "quoteProps"].includes(name),
		),
	);
	tab = options.useTabs ? "\t" : " ".repeat(options.tabWidth ?? 2);
	let answers = new Map<string, string | undefined>();
	let actual = "";
	try {
		let off = test.options.embeddedLanguageFormatting === "off";
		// Some requests depend on earlier answers, so this runs until nothing new is asked.
		for (let round = 0; round < 8; round++) {
			let missing = new Map<string, EmbedRequest>();
			actual = format(test.input, options, (request) => {
				if (off) return undefined;
				let id = key(request);
				if (!answers.has(id)) missing.set(id, request);
				return answers.get(id);
			});
			if (missing.size === 0) break;
			for (let [id, request] of missing) answers.set(id, await embedded(request, options, extra));
		}
	} catch (error) {
		actual = `!error ${(error as Error).message}`;
	}
	if (actual === test.output) passed++;
	else failed.push({ name: test.name, expected: test.output, actual });
}

for (let failure of failed.slice(0, show)) {
	console.log(`\n✗ ${failure.name}`);
	await Bun.write("/tmp/tidyrs-expected", failure.expected);
	await Bun.write("/tmp/tidyrs-actual", failure.actual);
	let diff = await $`diff -u /tmp/tidyrs-expected /tmp/tidyrs-actual`.nothrow().text();
	console.log(diff.split("\n").slice(2).join("\n"));
}
let total = passed + failed.length;
console.log(`\n${language}: ${passed}/${total} same (${((passed / total) * 100).toFixed(1)}%), ${skipped} skipped`);
await Bun.write(
	join(import.meta.dir, `../../target/conformance-${language}.txt`),
	failed.map((failure) => failure.name).join("\n"),
);
