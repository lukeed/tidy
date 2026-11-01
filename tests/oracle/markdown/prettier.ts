import { $ } from "bun";
import { existsSync } from "node:fs";
import { cp, rm } from "node:fs/promises";
import { basename, join, resolve } from "node:path";

// Usage: mise run oracle:prettier -- <corpus>... [--show <kind>]
// Formats one copy of each corpus with Prettier, the reference, and one with mdtidy's Prettier
// preset, then counts identical files and sorts the differences by kind. --show prints the hunks
// of one kind. Prettier's copy is cached in target/oracle: delete it after changing a corpus.

const PRETTIER = "prettier@3.9.10";

type Hunk = { file: string; removed: string[]; added: string[] };

let args = Bun.argv.slice(2);
let show = args.includes("--show") ? args[args.indexOf("--show") + 1] : undefined;
let corpora = args.filter((arg, index) => !arg.startsWith("--") && args[index - 1] !== "--show");
if (corpora.length === 0) throw new Error("usage: mise run oracle:prettier -- <corpus>... [--show <kind>]");

let root = new URL("..", import.meta.url).pathname;
await $`cargo build --release -p mdtidy_cli`.cwd(root).quiet();
let { target_directory } = await $`cargo metadata --format-version 1 --no-deps`.cwd(root).json();
let mdtidy = join(target_directory, "release", "mdtidy");

for (let corpus of corpora.map((corpus) => resolve(corpus))) {
	let cache = join(target_directory, "oracle", `${basename(corpus)}-${Bun.hash(corpus).toString(36)}`);
	let expected = join(cache, "prettier");
	if (!existsSync(expected)) {
		await cp(corpus, expected, { recursive: true });
		let flags = ["--no-config", "--no-editorconfig", "--ignore-path=", "--embedded-language-formatting=off"];
		await $`bunx ${PRETTIER} ${flags} --log-level=silent --write .`.cwd(expected).nothrow();
	}
	let actual = join(cache, "mdtidy");
	await rm(actual, { recursive: true, force: true });
	await cp(corpus, actual, { recursive: true });

	let files = (await Array.fromAsync(new Bun.Glob("**/*.md").scan({ cwd: actual, dot: true })))
		.filter((file) => !file.split("/").includes("node_modules"))
		.sort();
	for (let index = 0; index < files.length; index += 500) {
		await $`${mdtidy} --write ${files.slice(index, index + 500)}`.cwd(actual).quiet().nothrow();
	}

	let hunks: Hunk[] = [];
	let same = 0;
	for (let file of files) {
		let [before, after] = [join(expected, file), join(actual, file)];
		if ((await Bun.file(before).text()) === (await Bun.file(after).text())) {
			same++;
			continue;
		}
		let diff = await $`diff -U0 ${before} ${after}`.nothrow().text();
		for (let line of diff.split("\n").slice(2)) {
			if (line.startsWith("@@")) hunks.push({ file, removed: [], added: [] });
			else if (line.startsWith("-")) hunks.at(-1)?.removed.push(line.slice(1));
			else if (line.startsWith("+")) hunks.at(-1)?.added.push(line.slice(1));
		}
	}

	console.log(`${basename(corpus)}: ${same}/${files.length} files match Prettier`);
	let kinds = Map.groupBy(hunks, (hunk) => kind(hunk.removed, hunk.added));
	for (let [name, group] of [...kinds].sort((a, b) => b[1].length - a[1].length)) {
		let count = new Set(group.map((hunk) => hunk.file)).size;
		console.log(`${String(group.length).padStart(6)} hunks ${String(count).padStart(5)} files  ${name}`);
	}
	for (let hunk of kinds.get(show ?? "") ?? []) {
		console.log(`## ${hunk.file}`);
		for (let line of hunk.removed) console.log(`-${line.slice(0, 160)}`);
		for (let line of hunk.added) console.log(`+${line.slice(0, 160)}`);
	}
}

/** The kind of a difference: `-` lines are Prettier's, `+` lines are mdtidy's. */
function kind(removed: string[], added: string[]): string {
	let blank = (line: string) => line.trim() === "";
	if ([...removed, ...added].every(blank)) {
		return removed.length > 0 ? "blank lines (prettier adds)" : "blank lines (mdtidy adds)";
	}
	if (removed.length !== added.length) {
		let text = (lines: string[]) => lines.filter((line) => !blank(line)).join("\n");
		return text(removed) === text(added) ? "blank lines mixed" : "other";
	}
	let kinds = new Set<string>();
	removed.forEach((before, index) => {
		let after = added[index];
		let words = (line: string) => line.split(/\s+/).filter(Boolean).join(" ");
		let quotes = (line: string) => line.replaceAll('"', "'");
		let marks = (line: string) => line.replaceAll(/[`<>\\]/g, "");
		if (before.trim() === after.trim()) kinds.add(before.trimStart().startsWith("<") ? "html indent" : "indent");
		else if (quotes(before) === quotes(after)) kinds.add("title quotes");
		else if (words(before) === words(after)) kinds.add("spaces");
		else if (before.trimEnd() === after.trimEnd()) kinds.add("trailing space");
		else if (marks(before) === marks(after)) kinds.add("backticks, escapes, or <>");
		else kinds.add("other");
	});
	return [...kinds].sort().join(" + ");
}
