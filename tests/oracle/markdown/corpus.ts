import { $ } from "bun";
import { copyFile, mkdir } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";

// Usage: mise run oracle:corpus -- <out> <repo>...
// Copies the Markdown files each git repo tracks into <out>/<repo name>. Files missing from a
// partial checkout and anything under node_modules are skipped, as Prettier skips them.

let [out, ...repos] = Bun.argv.slice(2);
if (!out || repos.length === 0) throw new Error("usage: mise run oracle:corpus -- <out> <repo>...");

for (let repo of repos.map((repo) => resolve(repo))) {
	let listed = await $`git -C ${repo} ls-files -z -- ${"*.md"}`.text();
	let files = listed.split("\0").filter((file) => file && !file.split("/").includes("node_modules"));
	let copied = 0;
	for (let file of files) {
		let from = join(repo, file);
		if (!(await Bun.file(from).exists())) continue;
		let to = join(out, basename(repo), file);
		await mkdir(dirname(to), { recursive: true });
		await copyFile(from, to);
		copied++;
	}
	console.log(`${basename(repo).padEnd(24)} ${copied} files`);
}
