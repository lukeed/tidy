// Compares tidy's CSS formatter with oxc's (oxc_formatter_css), Malva (the engine of dprint-plugin-malva), bun fmt
// and Prettier, with lightningcss as a floor: it parses and prints CSS back, but doesn't format. Each corpus only
// has the files that Prettier can parse. For each tool it shows the time, with all cores and with one, and how many
// files it prints exactly as Prettier does.
//
// The results go to bench/results/css.md.
//
//   bun bench/css.ts [--runs <n>] [--oss <dir>]
import { $ } from "bun";
import { cpSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, extname, join, relative } from "node:path";
import * as prettier from "prettier";
import { crates, record, type Section, version } from "./results";

let args = Bun.argv.slice(2);
let option = (name: string) =>
  args.includes(name) ? args[args.indexOf(name) + 1] : undefined;
let runs = Number(option("--runs") ?? 10);
let oss = option("--oss") ?? join(homedir(), "repos", "oss");

let root = new URL("..", import.meta.url).pathname;
let target = join(root, "target", "css");
await $`cargo build --release`
  .cwd(join(root, "bench/css"))
  .env({ ...process.env, CARGO_TARGET_DIR: target })
  .quiet();
await $`cargo build --release -p tidy`
  .cwd(root)
  .env({ ...process.env, CARGO_TARGET_DIR: join(root, "target") })
  .quiet();
let tidy = join(root, "target/release/tidy");
let harness = join(target, "release/css-bench");

let parsers: Record<string, string> = {
  ".css": "css",
  ".scss": "scss",
  ".less": "less",
};
let corpus = join(tmpdir(), "tidy-css-bench");
rmSync(corpus, { recursive: true, force: true });

/**
 * Where each corpus comes from: repositories and the extensions to take from
 * them.
 */
let sources: Record<
  string,
  { repo: string; extensions: string[]; path?: string }[]
> = {
  frameworks: [
    { repo: "bun", extensions: ["css"], path: "test/js/bun/css/files" },
  ],
  css: ["vite", "svelte", "rolldown", "BrowserBox"].map((repo) => ({
    repo,
    extensions: ["css"],
  })),
  scss: ["snowpack", "tastejs", "vite", "storybook"].map((repo) => ({
    repo,
    extensions: ["scss"],
  })),
  less: ["bootstrap3", "antd4", "pwa", "vite", "taskr", "freshie"].map(
    (repo) => ({ repo, extensions: ["less"] }),
  ),
};

/** Prettier's output for each file of the corpus, by its path in the corpus. */
let expected = new Map<string, string>();
for (let [name, list] of Object.entries(sources)) {
  for (let { repo, extensions, path } of list) {
    let patterns = extensions.map(
      (extension) => `${path ? `${path}/` : ""}*.${extension}`,
    );
    let files = (
      await $`git -C ${join(oss, repo)} ls-files -z -- ${patterns}`
        .nothrow()
        .text()
    )
      .split("\0")
      .filter(Boolean);
    for (let file of files) {
      if (/node_modules|\/dist\/|\.min\.css$|(^|\/)\./.test(file)) continue;
      let source = await Bun.file(join(oss, repo, file))
        .text()
        .catch(() => undefined);
      if (source === undefined) continue;
      let formatted = await prettier
        .format(source, { parser: parsers[extname(file)] })
        .catch(() => undefined);
      if (formatted === undefined) continue;
      let to = join(corpus, name, repo, file);
      mkdirSync(dirname(to), { recursive: true });
      cpSync(join(oss, repo, file), to);
      expected.set(relative(corpus, to), formatted);
    }
  }
}

function size(path: string): [number, number] {
  if (statSync(path).isFile()) return [1, statSync(path).size];
  return readdirSync(path)
    .map((name) => size(join(path, name)))
    .reduce(
      ([files, bytes], [more, extra]) => [files + more, bytes + extra],
      [0, 0],
    );
}

let prettierBin = join(root, "node_modules/.bin/prettier");
let tools = (dir: string, name: string): Record<string, string> => ({
  tidy: `${tidy} check ${dir} --no-config`,
  "tidy (1 thread)": `${tidy} check ${dir} --no-config --threads 1`,
  oxc: `${harness} oxc ${dir}`,
  "oxc (1 thread)": `${harness} oxc ${dir} --threads 1`,
  malva: `${harness} malva ${dir}`,
  "malva (1 thread)": `${harness} malva ${dir} --threads 1`,
  ...(name === "scss" || name === "less"
    ? {}
    : {
        lightningcss: `${harness} lightningcss ${dir}`,
        "lightningcss (1 thread)": `${harness} lightningcss ${dir} --threads 1`,
      }),
  bun: `bun fmt --cwd=${dir} --check --no-config --no-editorconfig`,
  "bun (1 thread)": `bun fmt --cwd=${dir} --check --no-config --no-editorconfig --threads 1`,
  prettier: `${prettierBin} --check ${dir} --no-config --log-level silent`,
});

/** How many files of the corpus a tool prints exactly as Prettier does. */
async function matches(tool: string, dir: string): Promise<number | undefined> {
  if (tool === "prettier") return undefined;
  let out = join(corpus, "..", `tidy-css-out-${tool}`);
  rmSync(out, { recursive: true, force: true });
  if (tool === "bun") {
    cpSync(dir, out, { recursive: true });
    await $`bun fmt --cwd=${out} --write --no-config --no-editorconfig`
      .nothrow()
      .quiet();
  } else if (tool === "tidy") {
    cpSync(dir, out, { recursive: true });
    await $`${tidy} up ${out} --no-config`.nothrow().quiet();
  } else {
    await $`${harness} ${tool} ${dir} --write ${out}`.nothrow().quiet();
  }
  let same = 0;
  for (let [path, formatted] of expected) {
    if (!path.startsWith(relative(corpus, dir) + "/")) continue;
    let printed = await Bun.file(
      join(out, relative(relative(corpus, dir), path)),
    )
      .text()
      .catch(() => undefined);
    if (printed === formatted) same++;
  }
  return same;
}

type Result = { command: string; mean: number; user: number; system: number };

let rows: string[] = [];
let sections: Section[] = [];
for (let name of Object.keys(sources)) {
  let dir = join(corpus, name);
  let [files, bytes] = size(dir);
  let commands = tools(dir, name);
  let results: Record<string, Result[]> = {};
  for (let order of [Object.keys(commands), Object.keys(commands).reverse()]) {
    let file = join(target, "bench.json");
    let list = order.flatMap((tool) => [
      "--command-name",
      tool,
      commands[tool],
    ]);
    await $`hyperfine --warmup 2 --runs ${runs} --ignore-failure --style none --export-json ${file} ${list}`.quiet();
    let { results: measured } = (await Bun.file(file).json()) as {
      results: Result[];
    };
    for (let result of measured) (results[result.command] ??= []).push(result);
  }
  let average = (values: number[]) =>
    (values.reduce((sum, value) => sum + value, 0) / values.length) * 1000;
  rows.push(`\n${name}: ${files} files, ${(bytes / 1024).toFixed(0)} KB`);
  let summary = `${files} files, ${(bytes / 1024).toFixed(0)} KB. A time is the mean of ${runs} runs in each order. The last column counts the files that a tool prints exactly as Prettier does.`;
  let section: Section = {
    title: name,
    summary,
    head: ["Tool", "Wall", "CPU", "User", "Same as Prettier"],
    rows: [],
  };
  sections.push(section);
  rows.push(
    `  ${"tool".padEnd(24)} ${"wall".padStart(9)} ${"cpu".padStart(9)} ${"user".padStart(9)}  same as Prettier`,
  );
  for (let [tool, list] of Object.entries(results)) {
    let wall = average(list.map((it) => it.mean));
    let cpu = average(list.map((it) => it.user + it.system));
    let user = average(list.map((it) => it.user));
    let same = tool.includes("(1 thread)")
      ? undefined
      : await matches(tool, dir);
    let column =
      same === undefined
        ? ""
        : `${same}/${files} (${((same / files) * 100).toFixed(0)}%)`;
    rows.push(
      `  ${tool.padEnd(24)} ${wall.toFixed(1).padStart(6)} ms ${cpu.toFixed(1).padStart(6)} ms ${user.toFixed(1).padStart(6)} ms  ${column}`,
    );
    let times = [wall, cpu, user].map((time) => `${time.toFixed(1)} ms`);
    section.rows.push([tool, ...times, column || "—"]);
  }
}
console.log(rows.join("\n"));
let versions = [
  "tidy at the commit above",
  ...(await crates("bench/css/Cargo.lock", [
    "oxc_formatter_css",
    "malva",
    "lightningcss",
  ])),
  `bun fmt ${await version(["bun", "--version"])}`,
  `Prettier ${prettier.version}`,
];
await record("css.md", "Formatters: CSS, SCSS and Less", sections, versions);
