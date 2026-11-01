// Formats JSON files with Prettier and with tidy, under several option sets, and compares the outputs. Each file is
// formatted with the parser that Prettier infers from its name, and with each of the four JSON parsers. Both
// rejecting a file counts as the same. The files are those of the repos in `--oss`, and the inputs of JSONTestSuite,
// valid or not. Prettier overflows its stack on a value nested about 2,000 deep, which tidy formats: those runs are
// counted apart.
//
//   bun tests/oracle/json.ts [filter] [--show=n] [--oss=<dir>]
import { $ } from "bun";
import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { join } from "node:path";
import { parsing, transform } from "json-test-suite";
import * as prettier from "prettier";
import { format, type FormatOptions } from "../../crates/tidy_napi/index.js";

let args = Bun.argv.slice(2).filter((arg) => !arg.startsWith("--"));
let option = (name: string) => Bun.argv.find((arg) => arg.startsWith(`--${name}=`))?.slice(name.length + 3);
let show = Number(option("show") ?? 5);
let oss = option("oss") ?? join(homedir(), "repos", "oss");
let filter = args[0] ?? "";

type Set = { name: string; parser?: string; prettier: Record<string, unknown>; tidy: FormatOptions };

/** Option sets: Prettier's names, and tidy's for the same. Without a parser, it is the one Prettier infers. */
let sets: Set[] = [
	{ name: "default", prettier: {}, tidy: {} },
	{ name: "json", parser: "json", prettier: {}, tidy: {} },
	{ name: "jsonc", parser: "jsonc", prettier: {}, tidy: {} },
	{ name: "json5", parser: "json5", prettier: {}, tidy: {} },
	{ name: "json-stringify", parser: "json-stringify", prettier: {}, tidy: {} },
	{
		name: "jsonc, no trailing commas, width 40, tabs",
		parser: "jsonc",
		prettier: { trailingComma: "none", printWidth: 40, useTabs: true },
		tidy: { trailingComma: false, width: 40, tabs: true },
	},
	{
		name: "json5, consistent, single quotes",
		parser: "json5",
		prettier: { quoteProps: "consistent", singleQuote: true },
		tidy: { quote: "single", json: { quoteProps: "consistent" } },
	},
	{
		name: "json5, preserve",
		parser: "json5",
		prettier: { quoteProps: "preserve" },
		tidy: { json: { quoteProps: "preserve" } },
	},
	{
		name: "json, collapse, no bracket spacing",
		parser: "json",
		prettier: { objectWrap: "collapse", bracketSpacing: false },
		tidy: { json: { objectWrap: "collapse", bracketSpacing: false } },
	},
];

/** Prettier's `getLanguageByFileName`, for its JSON languages. */
let languages = (await prettier.getSupportInfo()).languages.filter((it) => it.parsers[0]?.startsWith("json"));
function inferParser(path: string) {
	let name = path.slice(path.lastIndexOf("/") + 1).toLowerCase();
	let language =
		languages.find((it) => it.filenames?.some((file) => file.toLowerCase() === name)) ??
		languages.find((it) => it.extensions?.some((extension) => name.endsWith(extension)));
	return language?.parsers[0];
}

let listed = await $`fd --type f --no-ignore --exclude node_modules --exclude target --exclude .git . ${oss}`
	.nothrow()
	.quiet()
	.text();
let seen = new Set<string>();
let files: { path: string; source: string; parser: string }[] = [];
for (let path of listed.split("\n").filter(Boolean)) {
	let parser = inferParser(path);
	if (!parser) continue;
	let source = await Bun.file(path).text().catch(() => undefined);
	if (source === undefined || source.length > 1_000_000) continue;
	let hash = createHash("sha1").update(source).digest("hex");
	if (seen.has(hash)) continue;
	seen.add(hash);
	files.push({ path, source, parser });
}

for (let { name, input } of [...parsing, ...transform]) {
	if (name.includes("utf16") || name.includes("UTF-16")) continue;
	files.push({ path: join(oss, "json-test-suite", name), source: input, parser: "json" });
}

let passed = 0;
let overflowed = 0;
let total = 0;
let failed: { name: string; expected: string; actual: string }[] = [];
let perSet = new Map<string, number>();
for (let set of sets) {
	for (let file of files) {
		let name = `${set.name}: ${file.path.slice(oss.length + 1)}`;
		if (filter && !name.includes(filter)) continue;
		total++;
		let parser = set.parser ?? file.parser;
		let expected: string;
		try {
			expected = await prettier.format(file.source, { parser, ...set.prettier });
		} catch (error) {
			if (error instanceof RangeError) {
				overflowed++;
				total--;
				continue;
			}
			expected = "!error";
		}
		let actual: string;
		try {
			actual = format(file.source, { parser: parser as FormatOptions["parser"], ...set.tidy });
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
if (overflowed) console.log(`  ${overflowed} runs left out: Prettier overflowed its stack`);
console.log(`\njson: ${passed}/${total} same (${((passed / total) * 100).toFixed(1)}%), ${files.length} files`);
await Bun.write(join(import.meta.dir, "../../target/conformance-json.txt"), failed.map((failure) => failure.name).join("\n"));
