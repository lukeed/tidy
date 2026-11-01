// Writes what Prettier prints for the YAML Test Suite and the YAML files under a folder, under several option sets,
// for `cargo run -- format` to compare with tidy.
//
//   bun tests/oracle/yaml/format.ts [<folder>] > target/yaml-format.json
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

/** Option sets, by the names the harness knows. */
let sets: Record<string, Record<string, unknown>> = {
	default: {},
	"single quotes, width 40": { singleQuote: true, printWidth: 40 },
	"prose always, width 40": { proseWrap: "always", printWidth: 40 },
	"prose never": { proseWrap: "never" },
	"no bracket spacing, no trailing commas, indent 4": { bracketSpacing: false, trailingComma: "none", tabWidth: 4 },
};

let out = [];
for (let { name, text } of inputs) {
	for (let [set, options] of Object.entries(sets)) {
		let expected: string;
		try {
			expected = await prettier.format(text, { parser: "yaml", ...options });
		} catch (error) {
			if (error instanceof RangeError) continue;
			expected = "!error";
		}
		out.push({ name, set, text, expected });
	}
}
console.log(JSON.stringify(out));
