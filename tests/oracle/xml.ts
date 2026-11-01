// Formats XML files with @prettier/plugin-xml and with tidy, under several option sets, and compares the outputs.
// Embedded code is formatted by Prettier through the hook. Both rejecting a file counts as the same.
//
//   bun tests/oracle/xml.ts [filter] [--show=n] [--builtin-css] [--oss=<dir>]
//
// With `--builtin-css`, the hook declines style sheets, so tidy formats them itself. The plugin's embedding is limited
// to `<style>` and `<script>`, as tidy's is (`xml-embed-by-name` in deviations.md), unless `--plugin-embed` is set.
// The other XML deviations show up as differences: `xml-hex-character-reference` and `xml-ignore-trims-xml-whitespace`,
// in 76 files of the corpus that deviations.md names, with `ignore` and `preserve` only.
import { $ } from "bun";
import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { join } from "node:path";
import plugin from "@prettier/plugin-xml";
import * as prettier from "prettier";
import { format, type EmbedRequest, type FormatOptions } from "../../crates/tidy_napi/index.js";

let args = Bun.argv.slice(2).filter((arg) => !arg.startsWith("--"));
let option = (name: string) => Bun.argv.find((arg) => arg.startsWith(`--${name}=`))?.slice(name.length + 3);
let show = Number(option("show") ?? 5);
let oss = option("oss") ?? join(homedir(), "repos", "oss");
let builtinCss = Bun.argv.includes("--builtin-css");
let filter = args[0] ?? "";

if (!Bun.argv.includes("--plugin-embed")) {
	let printer = plugin.printers.xml;
	let embed = printer.embed!;
	printer.embed = (path, options) => {
		let node = path.getValue();
		let name = node.name === "element" ? (node.Name as string).toLowerCase() : "";
		return name === "style" || name === "script" ? embed(path, options) : undefined;
	};
}

/** The extensions of tidy's walk, which leaves out those that other languages mostly use. */
let skipped = new Set([".rs", ".mm", ".mojo", ".typ", ".res", ".mod", ".sw", ".gml", ".ncl", ".mjml"]);
let extensions = plugin.languages.flatMap((language) => language.extensions ?? []).filter((it) => !skipped.has(it));

/** Option sets: the plugin's names, and tidy's for the same. */
let sets: { name: string; prettier: Record<string, unknown>; tidy: FormatOptions }[] = [
	{ name: "default", prettier: {}, tidy: {} },
	{ name: "ignore", prettier: { xmlWhitespaceSensitivity: "ignore" }, tidy: { whitespace: "ignore" } },
	{ name: "preserve", prettier: { xmlWhitespaceSensitivity: "preserve" }, tidy: { whitespace: "auto" } },
	{
		name: "ignore, bracketSameLine",
		prettier: { xmlWhitespaceSensitivity: "ignore", bracketSameLine: true },
		tidy: { whitespace: "ignore", bracketSameLine: true },
	},
	{
		name: "ignore, no self-closing space",
		prettier: { xmlWhitespaceSensitivity: "ignore", xmlSelfClosingSpace: false },
		tidy: { whitespace: "ignore", xml: { selfClosingSpace: false } },
	},
	{
		name: "ignore, sorted",
		prettier: { xmlWhitespaceSensitivity: "ignore", xmlSortAttributesByKey: true },
		tidy: { whitespace: "ignore", xml: { sortAttributes: true } },
	},
	{ name: "single quotes", prettier: { xmlQuoteAttributes: "single" }, tidy: { xml: { attributeQuotes: "single" } } },
	{ name: "double quotes", prettier: { xmlQuoteAttributes: "double" }, tidy: { xml: { attributeQuotes: "double" } } },
	{
		name: "ignore, one attribute per line",
		prettier: { xmlWhitespaceSensitivity: "ignore", singleAttributePerLine: true },
		tidy: { whitespace: "ignore", singleAttributePerLine: true },
	},
	{
		name: "ignore, width 40, tabs",
		prettier: { xmlWhitespaceSensitivity: "ignore", printWidth: 40, useTabs: true },
		tidy: { whitespace: "ignore", width: 40, tabs: true },
	},
	{
		name: "embedded off",
		prettier: { embeddedLanguageFormatting: "off" },
		tidy: { embedded: false },
	},
];

let pattern = `(${extensions.map((extension) => extension.replace(/[.-]/g, "\\$&")).join("|")})$`;
let listed = await $`fd --type f --no-ignore --exclude node_modules --exclude target ${pattern} ${oss}`
	.nothrow()
	.quiet()
	.text();
let seen = new Set<string>();
let files: { path: string; source: string }[] = [];
for (let path of listed.split("\n").filter(Boolean)) {
	if (!extensions.some((extension) => path.endsWith(extension))) continue;
	let source = await Bun.file(path).text().catch(() => undefined);
	if (source === undefined || source.length > 1_000_000) continue;
	let hash = createHash("sha1").update(source).digest("hex");
	if (seen.has(hash)) continue;
	seen.add(hash);
	files.push({ path, source });
}

async function embedded(request: EmbedRequest, options: Record<string, unknown>) {
	if (builtinCss && ["css", "scss", "less"].includes(request.language)) return undefined;
	try {
		let formatted = await prettier.format(request.code, {
			...options,
			parser: request.language,
			printWidth: request.width,
		});
		return formatted.replace(/\n+$/, "");
	} catch {
		return undefined;
	}
}

function tidy(source: string, options: FormatOptions, answers: Map<string, string | undefined>, missing: Map<string, EmbedRequest>) {
	return format(source, { parser: "xml", ...options }, (request) => {
		let id = JSON.stringify([request.code, request.language, request.width]);
		if (!answers.has(id)) missing.set(id, request);
		return answers.get(id);
	});
}

let passed = 0;
let total = 0;
let failed: { name: string; expected: string; actual: string }[] = [];
let perSet = new Map<string, number>();
for (let set of sets) {
	for (let file of files) {
		let name = `${set.name}: ${file.path.slice(oss.length + 1)}`;
		if (filter && !name.includes(filter)) continue;
		total++;
		let expected: string;
		try {
			expected = await prettier.format(file.source, { parser: "xml", plugins: [plugin], ...set.prettier });
		} catch (error) {
			expected = "!error";
		}
		let actual = "";
		try {
			let answers = new Map<string, string | undefined>();
			for (let round = 0; round < 4; round++) {
				let missing = new Map<string, EmbedRequest>();
				actual = tidy(file.source, set.tidy, answers, missing);
				if (missing.size === 0) break;
				for (let [id, request] of missing) answers.set(id, await embedded(request, set.prettier));
			}
		} catch (error) {
			actual = `!error ${(error as Error).message}`;
		}
		let same = expected === actual || (expected === "!error" && actual.startsWith("!error"));
		if (same) passed++;
		else {
			failed.push({ name, expected, actual });
			perSet.set(set.name, (perSet.get(set.name) ?? 0) + 1);
		}
	}
}

for (let failure of failed.slice(0, show)) {
	console.log(`\n✗ ${failure.name}`);
	await Bun.write("/tmp/tidy-expected", failure.expected);
	await Bun.write("/tmp/tidy-actual", failure.actual);
	let diff = await $`diff -u /tmp/tidy-expected /tmp/tidy-actual`.nothrow().text();
	console.log(diff.split("\n").slice(2, 40).join("\n"));
}
for (let [name, count] of perSet) console.log(`  ${name}: ${count} different`);
console.log(`\nxml: ${passed}/${total} same (${((passed / total) * 100).toFixed(1)}%), ${files.length} files`);
await Bun.write(join(import.meta.dir, "../../target/conformance-xml.txt"), failed.map((failure) => failure.name).join("\n"));
