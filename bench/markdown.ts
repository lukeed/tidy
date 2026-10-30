import { $ } from "bun";

// Usage: mise run bench -- <dir> [--runs <n>]
// Compares mdtidy with bun fmt, Bun's Prettier-compatible formatter, both in check mode with code
// blocks left alone. Each pair runs in both orders, since the first command in a pair is slower.

let args = Bun.argv.slice(2);
let dir = args.find((arg) => !arg.startsWith("--"));
if (!dir) throw new Error("usage: mise run bench -- <dir> [--runs <n>]");
let runs = Number(args[args.indexOf("--runs") + 1]) || 15;

let root = new URL("..", import.meta.url).pathname;
await $`cargo build --release -p mdtidy_cli`.cwd(root).quiet();
let { target_directory } = await $`cargo metadata --format-version 1 --no-deps`.cwd(root).json();

let tools = {
	mdtidy: `${target_directory}/release/mdtidy ${dir} --check`,
	bun: `bun fmt --cwd=${dir} --check --no-config --no-editorconfig --no-script --embedded-language-formatting off`,
};

type Result = { command: string; mean: number; stddev: number };

let means: Record<string, number[]> = { mdtidy: [], bun: [] };
for (let order of [Object.keys(tools), Object.keys(tools).reverse()]) {
	let file = `${root}/target/bench.json`;
	let commands = order.flatMap((name) => ["--command-name", name, tools[name as keyof typeof tools]]);
	await $`hyperfine --warmup 3 --runs ${runs} --ignore-failure --style none --export-json ${file} ${commands}`.quiet();
	let { results } = (await Bun.file(file).json()) as { results: Result[] };
	for (let result of results) means[result.command].push(result.mean);
}

let average = (values: number[]) => values.reduce((sum, value) => sum + value, 0) / values.length;
for (let [name, values] of Object.entries(means)) {
	console.log(`${name.padEnd(8)} ${(average(values) * 1000).toFixed(1)} ms`);
}
console.log(`mdtidy is ${(average(means.bun) / average(means.mdtidy)).toFixed(2)}x faster`);
