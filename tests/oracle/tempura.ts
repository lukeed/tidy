// Formats tempura templates and checks that the output is stable, still compiles with tempura, and compiles to the
// same code apart from whitespace. Embedded JavaScript is formatted by Prettier through the hook.
//
//   bun tests/oracle/tempura.ts [dirs...] (default: ~/repos/oss/bongo and ~/repos/oss/tempura)
import { $ } from "bun";
import { join } from "node:path";
import * as prettier from "prettier";
import { format, type EmbedRequest } from "../../crates/tidyrs_napi/index.js";
import { transform } from "tempura";

let home = process.env.HOME!;
let dirs = Bun.argv.slice(2);
if (!dirs.length) dirs = [join(home, "repos/oss/bongo"), join(home, "repos/oss/tempura")];
let files = (await $`fd -e hbs . ${dirs} -E node_modules`.text()).trim().split("\n").filter(Boolean);
// Custom blocks print nothing, so any name is one.
let blocks = new Proxy({}, { has: () => true, get: () => () => "" });

async function run(source: string, filepath: string) {
	let answers = new Map<string, string | undefined>();
	let output = "";
	// Some requests depend on earlier answers, so this runs until nothing new is asked.
	for (let round = 0; round < 8; round++) {
		let missing: EmbedRequest[] = [];
		output = format(source, { parser: "handlebars", filepath }, (request) => {
			let id = JSON.stringify(request);
			if (!answers.has(id)) missing.push(request);
			return answers.get(id);
		});
		if (!missing.length) break;
		for (let request of missing) {
			let width = request.width - request.indent * 2;
			let options = { parser: request.language, printWidth: width, singleQuote: request.kind === "attribute" };
			let text = await prettier.format(request.code, options).catch(() => undefined);
			let lines = text?.replace(/\n+$/, "").split("\n");
			answers.set(JSON.stringify(request), lines?.map((line, index) => (index === 0 || !line ? line : "  ".repeat(request.indent) + line)).join("\n"));
		}
	}
	return output;
}

function compiled(text: string) {
	try {
		return transform(text, { blocks, loose: true }).replace(/\s+/g, "");
	} catch {
		return undefined;
	}
}

let problems = 0;
for (let file of files) {
	let name = file.replace(`${home}/repos/oss/`, "");
	let source = await Bun.file(file).text();
	try {
		let once = await run(source, file);
		if ((await run(once, file)) !== once) throw new Error("not stable");
		// Only the template is compared: style sheets are formatted, which changes more than whitespace.
		let verbatim = format(source, { parser: "handlebars", filepath: file, embeddedLanguageFormatting: "off" });
		let before = compiled(source);
		if (before !== undefined && compiled(verbatim) !== before) throw new Error("compiles differently");
		if (before !== undefined && compiled(once) === undefined) throw new Error("doesn't compile");
	} catch (error) {
		problems++;
		console.log(`✗ ${name}: ${(error as Error).message}`);
	}
}
console.log(`\ntempura: ${files.length - problems}/${files.length} good`);
