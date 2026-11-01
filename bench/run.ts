// Times tidy against the incumbent formatters of each language, and on all of them at once. Every tool checks the
// files of a corpus and leaves embedded code alone, except tidy, which always formats style sheets in markup and in
// Markdown code blocks, so it does more work than the others. Each corpus only has the files that Prettier parses.
// Each set runs in both orders; Prettier runs fewer times, since it is two orders of magnitude slower.
//
// The incumbents, where they format a language:
// - Prettier 3.9.9, with @prettier/plugin-xml for XML;
// - bun fmt (not XML);
// - dprint's plugins: markup_fmt for HTML, Vue and XML, malva for style sheets, and dprint-plugin-markdown;
// - oxc's CSS formatter and lightningcss (CSS only, which reprints rather than formats) for style sheets;
// - a naive formatter on @lukeed/html, as a floor for HTML in JavaScript.
//
// "all" is every corpus in one tree, and "all-but-xml" leaves out XML, which bun fmt doesn't format.
//
//   mise run bench -- [--runs <n>] [--oss <dir>] [--only <corpus,...>]
import { $ } from "bun";
import { cpSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, extname, join } from "node:path";
import * as prettierApi from "prettier";

let args = Bun.argv.slice(2);
let option = (name: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
let runs = Number(option("--runs") ?? 10);
let oss = option("--oss") ?? join(homedir(), "repos", "oss");
let only = option("--only")?.split(",");

let root = new URL("..", import.meta.url).pathname;
let target = join(root, "target");
let build = (dir: string, out: string) =>
	$`cargo build --release`.cwd(join(root, dir)).env({ ...process.env, CARGO_TARGET_DIR: join(target, out) }).quiet();
await $`cargo build --release -p tidy_cli`.cwd(root).env({ ...process.env, CARGO_TARGET_DIR: target }).quiet();
await build("bench/dprint", "dprint");
await build("bench/css", "css");

/** Where each corpus comes from: repositories and the extensions to take from them. */
let sources: Record<string, { repo: string; extensions: string[]; path?: string }[]> = {
	html: ["vite", "storybook"].map((repo) => ({ repo, extensions: ["html"] })),
	vue: ["vite", "storybook", "npmx.dev"].map((repo) => ({ repo, extensions: ["vue"] })),
	handlebars: [{ repo: "ghost", extensions: ["hbs"] }],
	css: ["vite", "svelte", "rolldown", "BrowserBox"].map((repo) => ({ repo, extensions: ["css"] })),
	scss: ["snowpack", "tastejs", "vite", "storybook"].map((repo) => ({ repo, extensions: ["scss"] })),
	less: ["bootstrap3", "antd4", "pwa", "vite", "taskr", "freshie"].map((repo) => ({ repo, extensions: ["less"] })),
	xml: [
		{ repo: "maven", extensions: ["xml"] },
		{ repo: "lucide", extensions: ["svg"] },
		{ repo: "docbook-xsl", extensions: ["xsl", "xml"] },
	],
	markdown: ["vite", "storybook", "shadcn-svelte", "bun"].map((repo) => ({ repo, extensions: ["md"] })),
};

/** Prettier's options for each extension. */
let parsers: Record<string, Record<string, unknown>> = {
	".html": { parser: "html" },
	".vue": { parser: "vue" },
	".hbs": { parser: "glimmer" },
	".css": { parser: "css" },
	".scss": { parser: "scss" },
	".less": { parser: "less" },
	".xml": { parser: "xml", plugins: ["@prettier/plugin-xml"] },
	".svg": { parser: "xml", plugins: ["@prettier/plugin-xml"] },
	".xsl": { parser: "xml", plugins: ["@prettier/plugin-xml"] },
	".md": { parser: "markdown" },
};

let corpus = join(tmpdir(), "tidy-bench");
rmSync(corpus, { recursive: true, force: true });
for (let [name, list] of Object.entries(sources)) {
	for (let { repo, extensions } of list) {
		let patterns = extensions.map((extension) => `*.${extension}`);
		let listed = await $`git -C ${join(oss, repo)} ls-files -z -- ${patterns}`.nothrow().text();
		for (let file of listed.split("\0").filter(Boolean)) {
			if (/node_modules|\/dist\/|\/build\/|\.min\.css$|(^|\/)\./.test(file)) continue;
			let source = await Bun.file(join(oss, repo, file)).text().catch(() => undefined);
			if (source === undefined) continue;
			if (!(await prettierApi.format(source, parsers[extname(file)]).then(() => true, () => false))) continue;
			let to = join(corpus, name, repo, file);
			mkdirSync(dirname(to), { recursive: true });
			cpSync(join(oss, repo, file), to);
		}
	}
}
cpSync(join(oss, "html/bench/corpus"), join(corpus, "pages"), { recursive: true });
for (let name of [...Object.keys(sources), "pages"]) {
	cpSync(join(corpus, name), join(corpus, "all", name), { recursive: true });
	if (name !== "xml") cpSync(join(corpus, name), join(corpus, "all-but-xml", name), { recursive: true });
}

function size(path: string): [number, number] {
	if (statSync(path).isFile()) return [1, statSync(path).size];
	return readdirSync(path)
		.map((name) => size(join(path, name)))
		.reduce(([files, bytes], [more, extra]) => [files + more, bytes + extra], [0, 0]);
}

let prettier = join(root, "node_modules/.bin/prettier");
let off = "--no-config --embedded-language-formatting off";
let commands = (dir: string): Record<string, string> => ({
	tidy: `${target}/release/tidy ${dir} --check --no-config`,
	"tidy (1 thread)": `${target}/release/tidy ${dir} --check --no-config --threads 1`,
	bun: `bun fmt --cwd=${dir} --check --no-editorconfig --no-script ${off}`,
	dprint: `${target}/dprint/release/dprint-bench ${dir}`,
	oxc: `${target}/css/release/css-bench oxc ${dir}`,
	lightningcss: `${target}/css/release/css-bench lightningcss ${dir}`,
	naive: `bun ${root}/bench/naive.ts ${dir}`,
	prettier: `${prettier} --check ${dir} ${off} --log-level silent --plugin @prettier/plugin-xml`,
});

/** The corpora that each tool formats; the others leave them out. */
let markup = ["html", "vue", "pages"];
let sheets = ["css", "scss", "less"];
let formats: Record<string, string[]> = {
	bun: [...markup, "handlebars", ...sheets, "markdown", "all-but-xml"],
	dprint: [...markup, ...sheets, "xml", "markdown", "all", "all-but-xml"],
	oxc: sheets,
	lightningcss: ["css"],
	naive: markup,
};

type Result = { command: string; mean: number };

async function time(names: string[], all: Record<string, string>, count: number) {
	let means: Record<string, number[]> = {};
	for (let order of [names, names.toReversed()]) {
		let file = join(target, "bench.json");
		let list = order.flatMap((tool) => ["--command-name", tool, all[tool]]);
		await $`hyperfine --warmup ${Math.min(2, count)} --runs ${count} --ignore-failure --style none --export-json ${file} ${list}`.quiet();
		let { results } = (await Bun.file(file).json()) as { results: Result[] };
		for (let result of results) (means[result.command] ??= []).push(result.mean);
	}
	return means;
}

let average = (values: number[]) => (values.reduce((sum, value) => sum + value, 0) / values.length) * 1000;
let names = ["all", "all-but-xml", "html", "vue", "pages", "handlebars", "css", "scss", "less", "xml", "markdown"];
let rows: string[] = [];
for (let name of names.filter((name) => !only || only.includes(name))) {
	let dir = join(corpus, name);
	let [files, bytes] = size(dir);
	let all = commands(dir);
	let fast = Object.keys(all).filter((tool) => tool !== "prettier" && (!formats[tool] || formats[tool].includes(name)));
	let means = { ...(await time(fast, all, runs)), ...(await time(["prettier"], all, Math.min(runs, 3))) };
	let ours = average(means.tidy);
	rows.push(`\n${name}: ${files} files, ${(bytes / 1024 / 1024).toFixed(1)} MB`);
	let sorted = Object.entries(means).sort(([, left], [, right]) => average(left) - average(right));
	for (let [tool, values] of sorted) {
		let mean = average(values);
		rows.push(`  ${tool.padEnd(16)} ${mean.toFixed(1).padStart(9)} ms  ${(mean / ours).toFixed(2).padStart(7)}x`);
	}
	console.log(rows.slice(-sorted.length - 1).join("\n"));
}
await Bun.write(join(target, "bench.txt"), rows.join("\n"));
