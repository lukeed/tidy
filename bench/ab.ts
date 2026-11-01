import { $ } from "bun";
import { copyFile, mkdir } from "node:fs/promises";
import { join } from "node:path";

// Usage: mise run bench:ab -- [--rev <rev>] [--runs <n>] <dir>...
// Times the CLI built from the working tree against the one built from <rev> (HEAD by default),
// both checking the given folders. Each pair runs in both orders, since the first command in a
// pair is slower, and medians are compared, since means swing with machine load.

type Result = { command: string; median: number; min: number };

let args = Bun.argv.slice(2);
let option = (name: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
let rev = option("--rev") ?? "HEAD";
let runs = Number(option("--runs")) || 60;
let dirs = args.filter((arg, index) => !arg.startsWith("--") && !args[index - 1]?.startsWith("--"));
if (dirs.length === 0) throw new Error("usage: mise run bench:ab -- [--rev <rev>] [--runs <n>] <dir>...");

let root = new URL("..", import.meta.url).pathname;
let { target_directory } = await $`cargo metadata --format-version 1 --no-deps`.cwd(root).json();
let bench = join(target_directory, "ab");
await mkdir(bench, { recursive: true });

await $`cargo build --release -p mdtidy_cli`.cwd(root).quiet();
await copyFile(join(target_directory, "release", "mdtidy"), join(bench, "new"));

let tree = join(bench, "tree");
await $`git worktree add --force --detach ${tree} ${rev}`.cwd(root).quiet();
try {
	let env = { ...process.env, CARGO_TARGET_DIR: join(bench, "target") };
	await $`cargo build --release -p mdtidy_cli`.cwd(tree).env(env).quiet();
	await copyFile(join(bench, "target", "release", "mdtidy"), join(bench, "old"));
} finally {
	await $`git worktree remove --force ${tree}`.cwd(root).quiet();
}

let file = join(bench, "run.json");
let medians: Record<string, number[]> = { old: [], new: [] };
for (let order of [["old", "new"], ["new", "old"]]) {
	let commands = order.flatMap((name) => ["--command-name", name, `${join(bench, name)} ${dirs.join(" ")} --check`]);
	await $`hyperfine -N --warmup 20 --runs ${runs} --ignore-failure --style none --export-json ${file} ${commands}`.quiet();
	let { results } = (await Bun.file(file).json()) as { results: Result[] };
	for (let { command, median, min } of results) {
		medians[command].push(median);
		console.log(`${command.padEnd(4)} median ${(median * 1000).toFixed(2)} ms, min ${(min * 1000).toFixed(2)} ms`);
	}
}

let average = (values: number[]) => values.reduce((sum, value) => sum + value, 0) / values.length;
console.log(`new / old median: ${(average(medians.new) / average(medians.old)).toFixed(3)} (${rev})`);
