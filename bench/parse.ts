// Times tidy's parsers against the Rust crates that parse the same languages, with bench/parse, on the corpora of
// bench/run.ts. Each language's results go to bench/results/parse/<language>.md.
//
//   mise run bench:parse -- [--runs <n>] [--only <language,...>] [--rejects] [<corpus>]
import { $ } from "bun";
import { join } from "node:path";
import { crates, language as named, record } from "./results";

let root = new URL("..", import.meta.url).pathname;
let target = join(root, "target", "parse");
await $`cargo build --release`.cwd(join(root, "bench/parse")).env({ ...process.env, CARGO_TARGET_DIR: target }).quiet();

type Language = {
	language: string;
	files: number;
	total: number;
	bytes: number;
	runs: number;
	tools: { name: string; builds: string; ms: number; rejects: number }[];
};

/** The crate behind each tool that isn't tidy. */
let sources: Record<string, string> = {
	"jsonc-parser": "jsonc-parser",
	serde_json: "serde_json",
	"sonic-rs": "sonic-rs",
	taplo: "taplo",
	toml_edit: "toml_edit",
	toml: "toml",
	yaml_parser: "yaml_parser",
	saphyr: "saphyr",
	"yaml-rust2": "yaml-rust2",
	serde_yaml: "serde_yaml",
	raffia: "raffia",
	lightningcss: "lightningcss",
	scraper: "scraper",
	tl: "tl",
	roxmltree: "roxmltree",
	"quick-xml": "quick-xml",
	handlebars: "handlebars",
	"dprint-plugin-dockerfile": "dprint-plugin-dockerfile",
	"dockerfile-parser": "dockerfile-parser",
	"parse-dockerfile": "parse-dockerfile",
};

let output = await $`${join(target, "release/parse-bench")} --json ${Bun.argv.slice(2)}`.text();
for (let line of output.split("\n").filter(Boolean)) {
	let { language, files, total, bytes, runs, tools } = JSON.parse(line) as Language;
	let fastest = Math.min(...tools.map((tool) => tool.ms));
	let megabytes = bytes / 1e6;
	console.log(`\n${language}: ${files} of ${total} files, ${megabytes.toFixed(2)} MB`);
	for (let tool of tools) {
		let speed = `${(megabytes / (tool.ms / 1000)).toFixed(0)} MB/s`;
		let columns = [tool.name.padEnd(24), tool.builds.padEnd(22), `${tool.ms.toFixed(2)} ms`.padStart(10), speed.padStart(10)];
		console.log(`  ${columns.join(" ")}  ${(tool.ms / fastest).toFixed(2).padStart(6)}x  rejects ${tool.rejects}`);
	}
	let summary = `${files} of ${total} files, ${megabytes.toFixed(2)} MB: the files that every tool accepts. Each tool parses them on one thread, from memory, and drops what it builds. A time is the median of ${runs} passes. The tools build different things: a tree for a formatter keeps comments and positions, and a value keeps only the data.`;
	let rows = tools.map((tool) => [
		tool.name,
		tool.builds,
		`${tool.ms.toFixed(2)} ms`,
		`${(megabytes / (tool.ms / 1000)).toFixed(0)} MB/s`,
		`${(tool.ms / fastest).toFixed(2)}x`,
		`${tool.rejects}`,
	]);
	let head = ["Tool", "Builds", "Time", "Speed", "Against fastest", "Rejects"];
	let names = [...new Set(tools.map((tool) => sources[tool.name.split(" ")[0]]).filter(Boolean))];
	let versions = ["tidy at the commit above", ...(await crates("bench/parse/Cargo.lock", names))];
	await record(`parse/${language}.md`, `Parsers: ${named(language)}`, [{ summary, head, rows }], versions);
}
