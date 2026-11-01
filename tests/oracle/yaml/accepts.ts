// Writes whether Prettier's YAML parser accepts each input of the YAML Test Suite and each YAML file under a folder,
// for `cargo run -- accepts` to compare with tidy.
//
//   bun tests/oracle/yaml/accepts.ts [<folder>] > target/yaml-accepts.json
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
let out = [];
for (let { name, text } of inputs) {
	let accepted = true;
	let message = "";
	try {
		await prettier.__debug.parse(text, { parser: "yaml" });
	} catch (error) {
		accepted = false;
		message = String((error as Error).message).split("\n")[0];
	}
	out.push({ name, text, accepted, message });
}
console.log(JSON.stringify(out));
