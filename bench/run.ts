// Compares tidyrs with Prettier, bun fmt, markup_fmt (the engine of dprint-plugin-markup), and a naive formatter on
// @lukeed/html. Every tool checks the files and leaves embedded code alone, except that tidyrs always formats style
// sheets, so it does more work than the others. Each set runs in both orders. The
// Handlebars corpus is Ghost's templates that Prettier can parse; only tidyrs, bun and Prettier format Handlebars. The XML
// corpus is the files of Maven, Lucide's icons and DocBook's style sheets that @prettier/plugin-xml can parse; only tidyrs,
// markup_fmt and Prettier with that plugin format XML.
//
//   mise run bench -- [--runs <n>] [--oss <dir>]
import { $ } from "bun";
import { cpSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join } from "node:path";
import * as prettierApi from "prettier";

let args = Bun.argv.slice(2);
let option = (name: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
let runs = Number(option("--runs") ?? 10);
let oss = option("--oss") ?? join(homedir(), "repos", "oss");

let root = new URL("..", import.meta.url).pathname;
let target = join(root, "target");
await $`cargo build --release -p tidyrs_cli`.cwd(root).env({ ...process.env, CARGO_TARGET_DIR: target }).quiet();
await $`cargo build --release`.cwd(join(root, "bench/dprint")).env({ ...process.env, CARGO_TARGET_DIR: join(target, "dprint") }).quiet();

/** The corpora: markup files from local clones, and real pages from @lukeed/html's benchmark. */
let corpus = join(tmpdir(), "tidyrs-bench");
rmSync(corpus, { recursive: true, force: true });
let sources: Record<string, { repo: string; extensions: string[] }[]> = {
	html: [
		{ repo: "vite", extensions: ["html"] },
		{ repo: "storybook", extensions: ["html"] },
	],
	vue: [
		{ repo: "vite", extensions: ["vue"] },
		{ repo: "storybook", extensions: ["vue"] },
		{ repo: "npmx.dev", extensions: ["vue"] },
	],
	handlebars: [{ repo: "ghost", extensions: ["hbs"] }],
	xml: [
		{ repo: "maven", extensions: ["xml"] },
		{ repo: "lucide", extensions: ["svg"] },
		{ repo: "docbook-xsl", extensions: ["xsl", "xml"] },
	],
};
let parsers: Record<string, Record<string, unknown>> = {
	handlebars: { parser: "glimmer" },
	xml: { parser: "xml", plugins: ["@prettier/plugin-xml"] },
};
for (let [name, list] of Object.entries(sources)) {
	for (let { repo, extensions } of list) {
		let patterns = extensions.map((extension) => `*.${extension}`);
		let files = (await $`git -C ${join(oss, repo)} ls-files -z -- ${patterns}`.text()).split("\0").filter(Boolean);
		for (let file of files) {
			if (/node_modules|\/dist\/|\/build\/|(^|\/)\./.test(file)) continue;
			let to = join(corpus, name, repo, file);
			if (parsers[name]) {
				let source = await Bun.file(join(oss, repo, file)).text().catch(() => undefined);
				if (source === undefined) continue;
				let parses = await prettierApi.format(source, parsers[name]).then(() => true, () => false);
				if (!parses) continue;
			}
			mkdirSync(dirname(to), { recursive: true });
			try {
				cpSync(join(oss, repo, file), to);
			} catch {}
		}
	}
}
cpSync(join(oss, "html/bench/corpus"), join(corpus, "pages"), { recursive: true });

function size(path: string): [number, number] {
	if (statSync(path).isFile()) return [1, statSync(path).size];
	return readdirSync(path)
		.map((name) => size(join(path, name)))
		.reduce(([files, bytes], [more, extra]) => [files + more, bytes + extra], [0, 0]);
}

let prettier = join(root, "node_modules/.bin/prettier");
let tools = (dir: string, name: string): Record<string, string> => ({
	tidyrs: `${target}/release/tidyrs ${dir} --check --no-config`,
	"tidyrs (1 thread)": `${target}/release/tidyrs ${dir} --check --no-config --threads 1`,
	bun: `bun fmt --cwd=${dir} --check --no-config --no-editorconfig --no-script --embedded-language-formatting off`,
	"bun (1 thread)": `bun fmt --cwd=${dir} --check --no-config --no-editorconfig --no-script --embedded-language-formatting off --threads 1`,
	markup_fmt: `${target}/dprint/release/dprint-bench ${dir}`,
	prettier: `${prettier} --check ${dir} --no-config --embedded-language-formatting off --log-level silent${name === "xml" ? " --plugin @prettier/plugin-xml" : ""}`,
	naive: `bun ${root}/bench/naive.ts ${dir}`,
	"naive (1 thread)": `bun ${root}/bench/naive.ts ${dir} --threads 1`,
});

type Result = { command: string; mean: number };

let rows: string[] = [];
/** The tools that don't format a corpus. */
let unsupported: Record<string, string[]> = {
	handlebars: ["markup_fmt", "naive", "naive (1 thread)"],
	xml: ["bun", "bun (1 thread)", "naive", "naive (1 thread)"],
};
for (let name of ["html", "vue", "pages", "handlebars", "xml"]) {
	let dir = join(corpus, name);
	let [files, bytes] = size(dir);
	let all = tools(dir, name);
	let commands = Object.fromEntries(Object.entries(all).filter(([tool]) => !unsupported[name]?.includes(tool)));
	let means: Record<string, number[]> = {};
	for (let order of [Object.keys(commands), Object.keys(commands).reverse()]) {
		let file = join(target, "bench.json");
		let list = order.flatMap((tool) => ["--command-name", tool, commands[tool]]);
		await $`hyperfine --warmup 2 --runs ${runs} --ignore-failure --style none --export-json ${file} ${list}`.quiet();
		let { results } = (await Bun.file(file).json()) as { results: Result[] };
		for (let result of results) (means[result.command] ??= []).push(result.mean);
	}
	let average = (values: number[]) => (values.reduce((sum, value) => sum + value, 0) / values.length) * 1000;
	let ours = average(means.tidyrs);
	rows.push(`\n${name}: ${files} files, ${(bytes / 1024).toFixed(0)} KB`);
	for (let [tool, values] of Object.entries(means)) {
		let mean = average(values);
		rows.push(`  ${tool.padEnd(18)} ${mean.toFixed(1).padStart(8)} ms  ${(mean / ours).toFixed(2).padStart(6)}x`);
	}
}
console.log(rows.join("\n"));
