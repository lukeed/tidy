// Writes the concrete syntax trees that `yaml` 2.9.0 parses from the YAML Test Suite and from the YAML files under a
// folder, for `cargo run -- cst` to compare with tidy's.
//
//   bun tests/oracle/yaml/cst.ts [<folder>] > target/yaml-cst.json
import { $ } from "bun";
import { homedir } from "node:os";
import { join } from "node:path";
import { Parser } from "yaml";
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
let out = inputs.map(({ name, text }) => ({ name, text, cst: [...new Parser().parse(text)] }));
console.log(JSON.stringify(out));
