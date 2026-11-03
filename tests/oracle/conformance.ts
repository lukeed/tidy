// Formats Prettier's own fixtures and compares the output with its snapshots. Embedded code is formatted by Prettier
// through the hook, so only the markup printing is under test.
//
//   bun tests/oracle/conformance.ts [language=html] [filter] [--show=n] [--builtin-css] [--builtin-js]
//
// With `--builtin-css`, the hook declines style sheets, so tidy formats them itself. With `--builtin-js`, it declines
// JavaScript and TypeScript, and the expressions and bindings of templates.
import { $, Glob } from "bun";
import { join, relative } from "node:path";
import * as prettier from "prettier";
import { format, type EmbedRequest, type FormatOptions } from "../../crates/tidy_napi/index.js";

let args = Bun.argv.slice(2).filter((arg) => !arg.startsWith("--"));
let show = Number(Bun.argv.find((arg) => arg.startsWith("--show="))?.slice(7) ?? 5);
let builtinCss = Bun.argv.includes("--builtin-css");
let builtinJs = Bun.argv.includes("--builtin-js");
let javascript = /^(babel|typescript|babel-ts|__(js|ts|vue|vue_ts)_[a-z_]+)$/;
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

/** Prettier's parsers of JavaScript and TypeScript that tidy stands in for. Flow isn't one. */
let scriptParsers: Record<string, string> = {
	babel: "babel",
	espree: "babel",
	meriyah: "babel",
	acorn: "babel",
	typescript: "typescript",
	"babel-ts": "babel-ts",
};

function settings(options: Record<string, unknown>, file: string): FormatOptions | undefined {
	let parsers = options.parsers as string[];
	let script = parsers?.map((name) => scriptParsers[name]).find(Boolean);
	if (script) {
		if ("rangeStart" in options || "rangeEnd" in options || "requirePragma" in options || "insertPragma" in options) {
			return undefined;
		}
		return {
			parser: script,
			quote: options.singleQuote ? "single" : "double",
			js: {
				...scripts(options),
				...Object.fromEntries(
					["objectWrap", "experimentalTernaries", "experimentalOperatorPosition", "jsxSingleQuote", "bracketSameLine"]
						.filter((name) => name in options)
						.map((name) => [name, options[name]]),
				),
			},
			embedded: options.embeddedLanguageFormatting !== "off",
			width: (options.printWidth as number) ?? 80,
			indent: (options.tabWidth as number) ?? 2,
			tabs: (options.useTabs as boolean) ?? false,
			filepath: file,
			...(options.endOfLine && options.endOfLine !== "auto" ? { eol: options.endOfLine } : {}),
			...(options.singleAttributePerLine ? { singleAttributePerLine: true } : {}),
			// `jsxBracketSameLine` is the deprecated name of `bracketSameLine`.
			...((options.bracketSameLine ?? options.jsxBracketSameLine) ? { bracketSameLine: true } : {}),
			...(options.proseWrap ? { markdown: { wrap: options.proseWrap } } : {}),
			...(options.htmlWhitespaceSensitivity
				? { whitespace: { css: "auto", strict: "strict", ignore: "ignore" }[options.htmlWhitespaceSensitivity as string] }
				: {}),
		} as FormatOptions;
	}
	let parser: string = parsers?.[0] === "glimmer" ? "handlebars" : parsers?.[0];
	if (!["html", "vue", "mjml", "handlebars", "css", "scss", "less"].includes(parser)) return undefined;
	let whitespace = { css: "auto", strict: "strict", ignore: "ignore" }[(options.htmlWhitespaceSensitivity as string) ?? "css"];
	return {
		parser,
		quote: options.singleQuote ? "single" : "double",
		css: { trailingComma: options.trailingComma !== "none" },
		js: scripts(options),
		embedded: options.embeddedLanguageFormatting !== "off",
		width: (options.printWidth as number) ?? 80,
		indent: (options.tabWidth as number) ?? 2,
		tabs: (options.useTabs as boolean) ?? false,
		whitespace: whitespace as "auto",
		singleAttributePerLine: (options.singleAttributePerLine as boolean) ?? false,
		bracketSameLine: (options.bracketSameLine as boolean) ?? false,
		vue: { indentBlocks: (options.vueIndentScriptAndStyle as boolean) ?? false },
		filepath: file,
	};
}

/** tidy's options for the scripts it formats itself: Prettier's for the fixture, without prettier-plugin-jsdoc. */
function scripts(options: Record<string, unknown>) {
	let names = ["semi", "trailingComma", "bracketSpacing", "arrowParens", "quoteProps"];
	let set = names.filter((name) => name in options).map((name) => [name, options[name]]);
	return { jsdoc: false, ...Object.fromEntries(set) };
}

function key(request: EmbedRequest) {
	return JSON.stringify([request.code, request.language, request.kind, request.indent, request.prefix, request.suffix, request.width]);
}

/** Parameter lists, which Prettier formats with options that only its HTML printer can pass. */
async function parameters(request: EmbedRequest, extra: Record<string, unknown>, width: number) {
	let ts = request.language.includes("_ts_");
	let wrap = request.language.endsWith("generic")
		? (code: string) => [`type T<${code}> = any`, "type T<", "> = any;"]
		: (code: string) => [`function _(${code}) {}`, "function _(", ") {}"];
	let [code, start, end] = wrap(request.code);
	let parser = ts || request.language.endsWith("generic") ? "babel-ts" : "babel";
	let formatted = (await prettier.format(code, { ...extra, parser, printWidth: 2 ** 31 - 1 })).trim();
	if (!formatted.startsWith(start) || !formatted.endsWith(end)) return undefined;
	let inner = formatted.slice(start.length, -end.length);
	let { ast } = await prettier.__debug.parse(code, { parser });
	let many = (ast as { program: { body: { params?: unknown[] }[] } }).program.body[0].params?.length !== 1;
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
	let handler = request.kind === "attribute" && request.language === "babel";
	try {
		let settings = {
			...extra,
			...(request.kind === "attribute" ? { singleQuote: true } : {}),
			parser: request.language,
			// Prettier parses a classic script with `__babelSourceType`, which only its HTML printer can pass.
			...(request.module ? {} : { filepath: "script.cjs" }),
			printWidth: request.width,
			tabWidth: options.indent ?? 2,
			useTabs: options.tabs ?? false,
		};
		// The doc printed inside the indents, as Prettier prints it, so that literal lines stay where they are.
		let { builders, printer } = prettier.doc;
		let code = await prettier.__debug.printToDoc(request.code, settings);
		// The text around the code on its lines, as spaces that the trimming below removes.
		let doc: prettier.Doc = [builders.hardline, " ".repeat(request.prefix), code, " ".repeat(request.suffix)];
		for (let level = 0; level < request.indent; level++) doc = builders.indent(doc);
		let formatted = printer.printDocToString(doc, settings as prettier.doc.printer.Options).formatted;
		formatted = formatted.replace(/^\n[\t ]*/, "").replace(/\s+$/, "");
		// Prettier's `__isHtmlInlineEventHandler`, which only the HTML printer can pass: no semicolon after one statement.
		if (handler) {
			let { ast } = await prettier.__debug.parse(request.code, { parser: "babel" });
			let program = (ast as { program: { body: { type: string }[]; directives: unknown[] } }).program;
			let single = program.body.length === 1 && program.body[0].type === "ExpressionStatement";
			if (single && program.directives.length === 0) formatted = formatted.replace(/;$/, "").replace(/^;/, "");
		}
		return formatted;
	} catch {
		return undefined;
	}
}

let scriptParserSet: Record<string, true> = { babel: true, typescript: true, "babel-ts": true };

/** Prettier's fixed point for a script: where its output changes when formatted again, tidy prints what it settles on. */
async function scriptFixedPoint(output: string, options: Record<string, unknown>, file: string) {
	let parser = (options.parsers as string[]).map((name) => scriptParsers[name]).find(Boolean);
	let { parsers: _, ...rest } = options;
	for (let round = 0; round < 5; round++) {
		let again = await prettier.format(output, { ...rest, parser, filepath: file }).catch(() => undefined);
		if (again === undefined || again === output) return again;
		output = again;
	}
	return undefined;
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
	// The TOML fixtures need a plugin from Prettier's test setup.
	let plugin = test.name.startsWith("with-plugins/") || test.name.startsWith("front-matter/toml/");
	// Jest shows line breaks other than `\n` as `<CR>` and `<LF>`, which aren't the input's.
	let markers = /<(CR|LF)>/.test(test.input) || /<(CR|LF)>/.test(test.output);
	if (!options || "cursorOffset" in test.options || test.input.includes("<|>") || plugin || markers) {
		skipped++;
		continue;
	}
	let extra = Object.fromEntries(
		Object.entries(test.options).filter(([name]) =>
			["semi", "singleQuote", "trailingComma", "bracketSpacing", "arrowParens", "quoteProps"].includes(name),
		),
	);
	tab = options.tabs ? "\t" : " ".repeat(options.indent ?? 2);
	let answers = new Map<string, string | undefined>();
	let actual = "";
	try {
		let off = test.options.embeddedLanguageFormatting === "off";
		// Some requests depend on earlier answers, so this runs until nothing new is asked.
		for (let round = 0; round < 8; round++) {
			let missing = new Map<string, EmbedRequest>();
			actual = format(test.input, options, (request) => {
				if (off || (builtinCss && ["css", "scss", "less"].includes(request.language))) return undefined;
				if (builtinJs && javascript.test(request.language)) return undefined;
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
	if (actual === test.output || (options.parser && options.parser in scriptParserSet && actual === (await scriptFixedPoint(test.output, test.options, test.file)))) passed++;
	else failed.push({ name: test.name, expected: test.output, actual });
}

for (let failure of failed.slice(0, show)) {
	console.log(`\n✗ ${failure.name}`);
	await Bun.write("/tmp/tidy-expected", failure.expected);
	await Bun.write("/tmp/tidy-actual", failure.actual);
	let diff = await $`diff -u /tmp/tidy-expected /tmp/tidy-actual`.nothrow().text();
	console.log(diff.split("\n").slice(2).join("\n"));
}
let total = passed + failed.length;
console.log(`\n${language}: ${passed}/${total} same (${((passed / total) * 100).toFixed(1)}%), ${skipped} skipped`);
await Bun.write(
	join(import.meta.dir, `../../target/conformance-${language}.txt`),
	failed.map((failure) => failure.name).join("\n"),
);
