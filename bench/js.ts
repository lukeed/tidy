// Times the JavaScript and TypeScript formatters against each other, and counts the files that each prints as Prettier
// does. Each corpus has the files that Prettier parses. Every tool runs with Prettier's defaults: oxfmt with a width of
// 80, Biome with spaces, tidy without JSDoc formatting. Each runs with embedded code left alone, and the tools that can
// format it run again with it formatted.
//
// The tools: tidy, Prettier 3.9.9, oxfmt, Biome and bun fmt. Each set runs in both orders; Prettier runs fewer times.
// The results go to bench/results/format/js.md and ts.md.
//
//   bun bench/js.ts [--runs <n>] [--oss <dir>] [--only js,ts]
import { $ } from "bun";
import { cpSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import * as prettierApi from "prettier";
import { record, version } from "./results";

let args = Bun.argv.slice(2);
let option = (name: string) =>
  args.includes(name) ? args[args.indexOf(name) + 1] : undefined;
let runs = Number(option("--runs") ?? 5);
let oss = option("--oss") ?? join(homedir(), "repos", "oss");
let only = option("--only")?.split(",");

let root = new URL("..", import.meta.url).pathname;
let bin = (name: string) => join(root, "node_modules/.bin", name);
let target = join(root, "target");
await $`cargo build --release -p tidy`
  .cwd(root)
  .env({ ...process.env, CARGO_TARGET_DIR: target })
  .quiet();
let repos = ["vite", "storybook", "rolldown", "BrowserBox"];
let extensions: Record<string, string[]> = {
  js: ["js", "jsx", "mjs", "cjs"],
  ts: ["ts", "tsx", "mts", "cts"],
};

let base = join(tmpdir(), "tidy-bench-js");
rmSync(base, { recursive: true, force: true });
/**
 * The config files of tidy and oxfmt, with embedded code left alone or
 * formatted.
 */
let configs = async (embedded: boolean) => {
  let mode = embedded ? "auto" : "off";
  let oxfmt = join(base, `oxfmtrc-${mode}.json`);
  await Bun.write(
    oxfmt,
    JSON.stringify({ printWidth: 80, embeddedLanguageFormatting: mode }),
  );
  let tidy = join(base, `tidy-${mode}.json`);
  await Bun.write(tidy, JSON.stringify({ embedded, js: { jsdoc: false } }));
  return { oxfmt, tidy };
};
let config = { off: await configs(false), on: await configs(true) };

/**
 * The commands that check a folder, or format it in place. With `embedded`,
 * only the tools that format embedded code.
 */
let commands = (
  dir: string,
  write: boolean,
  embedded = false,
): Record<string, string> => {
  let check = write ? "" : "--check";
  let { oxfmt, tidy } = embedded ? config.on : config.off;
  let off = embedded ? "" : "--embedded-language-formatting off";
  return {
    tidy: `${target}/release/tidy ${write ? "up" : "check"} ${dir} --config ${tidy}`,
    "tidy (1 thread)": `${target}/release/tidy ${write ? "up" : "check"} ${dir} --config ${tidy} --threads 1`,
    prettier: `${bin("prettier")} ${write ? "--write" : "--check"} ${dir} --no-config ${off} --log-level silent`,
    oxfmt: `${bin("oxfmt")} ${check} -c ${oxfmt} --disable-nested-config ${dir}`,
    "oxfmt (1 thread)": `${bin("oxfmt")} ${check} -c ${oxfmt} --disable-nested-config --threads=1 ${dir}`,
    ...(embedded
      ? {}
      : {
          biome: `${bin("biome")} format ${write ? "--write" : ""} --indent-style=space --max-diagnostics=0 ${dir}`,
        }),
    bun: `bun fmt --cwd=${dir} ${check} --no-editorconfig --no-script --no-config ${off}`,
  };
};

let versions: Record<string, string> = {
  tidy: "tidy at the commit above",
  prettier: `Prettier ${prettierApi.version}`,
  oxfmt: `oxfmt ${await version([bin("oxfmt"), "--version"])}`,
  biome: `Biome ${await version([bin("biome"), "--version"])}`,
  bun: `bun fmt ${await version(["bun", "--version"])}`,
};

for (let name of Object.keys(extensions).filter(
  (name) => !only || only.includes(name),
)) {
  let corpus = join(base, name);
  await gather(corpus, extensions[name]);
  let [files, bytes] = size(corpus);
  let measure = async (embedded: boolean) => {
    let check = commands(corpus, false, embedded);
    let fast = Object.keys(check).filter((tool) => tool !== "prettier");
    let means = {
      ...(await time(fast, check, runs)),
      ...(await time(["prettier"], check, Math.min(runs, 2))),
    };
    let tools = Object.keys(versions).filter((tool) => tool in check);
    return { means, matches: await conformance(corpus, tools, embedded) };
  };
  let off = await measure(false);
  let on = await measure(true);
  let cells = ({ means, matches }: typeof off, tool: string) => {
    let values = means[tool];
    if (!values) return ["", "", ""];
    let fastest = Math.min(...Object.values(means).map(average));
    let match = matches[tool.replace(" (1 thread)", "")];
    let share =
      tool === "prettier"
        ? "reference"
        : `${match} (${((match / files) * 100).toFixed(1)}%)`;
    return [
      `${average(values).toFixed(0)} ms`,
      `${(average(values) / fastest).toFixed(2)}x`,
      share,
    ];
  };
  let sorted = Object.keys(off.means).sort(
    (left, right) => average(off.means[left]) - average(off.means[right]),
  );
  let rows = sorted.map((tool) => [
    tool,
    ...cells(off, tool),
    ...cells(on, tool),
  ]);
  console.log(`\n${name}: ${files} files`);
  for (let row of rows) console.log(`  ${row.join("  ")}`);
  let summary = `${files} files, ${(bytes / 1024 / 1024).toFixed(1)} MB, from ${repos.join(", ")}: the files that Prettier parses. Each tool checks every file; a time is the mean of ${runs} runs in each order, or ${Math.min(runs, 2)} for Prettier. "Same as Prettier" counts the files that a tool formats to Prettier's output. With one thread, tidy reads and formats the files in turn, while oxfmt still reads them on another thread, so its time doesn't include all of its reading.`;
  summary +=
    " With embedded code formatted, Biome doesn't run, as it can't format it, and Prettier formats it too.";
  let head = [
    "Tool",
    "Embedded off",
    "Against fastest",
    "Same as Prettier",
    "Embedded on",
    "Against fastest",
    "Same as Prettier",
  ];
  let title = `Formatters: ${name === "js" ? "JavaScript" : "TypeScript"}`;
  await record(
    `format/${name}.md`,
    title,
    [{ summary, head, rows }],
    Object.values(versions),
  );
}

/**
 * Copies the files of the repos with `list` extensions that Prettier parses
 * into `corpus`.
 */
async function gather(corpus: string, list: string[]) {
  for (let repo of repos) {
    let patterns = list.map((extension) => `*.${extension}`);
    let listed = await $`git -C ${join(oss, repo)} ls-files -z -- ${patterns}`
      .nothrow()
      .text();
    for (let file of listed.split("\0").filter(Boolean)) {
      if (
        /node_modules|\/dist\/|\/build\/|(^|\/)\.|\.min\.js$|fixtures?\/|__snapshots__|\/vendor\//.test(
          file,
        )
      )
        continue;
      let from = join(oss, repo, file);
      let source = await Bun.file(from)
        .text()
        .catch(() => undefined);
      if (source === undefined) continue;
      let parsed = await prettierApi.format(source, { filepath: from }).then(
        () => true,
        () => false,
      );
      if (!parsed) continue;
      let to = join(corpus, repo, file);
      mkdirSync(dirname(to), { recursive: true });
      cpSync(from, to);
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

function list(path: string): string[] {
  if (statSync(path).isFile()) return [path];
  return readdirSync(path).flatMap((name) => list(join(path, name)));
}

type Result = { command: string; mean: number };

async function time(
  names: string[],
  all: Record<string, string>,
  count: number,
) {
  let means: Record<string, number[]> = {};
  for (let order of [names, names.toReversed()]) {
    let file = join(base, "hyperfine.json");
    let flags = order.flatMap((tool) => ["--command-name", tool, all[tool]]);
    await $`hyperfine --warmup 1 --runs ${count} --ignore-failure --style none --export-json ${file} ${flags}`.quiet();
    let { results } = (await Bun.file(file).json()) as { results: Result[] };
    for (let result of results)
      (means[result.command] ??= []).push(result.mean);
  }
  return means;
}

/**
 * Formats a copy of the corpus with each tool, and counts the files equal to
 * Prettier's copy.
 */
async function conformance(corpus: string, tools: string[], embedded: boolean) {
  let outputs: Record<string, Map<string, string>> = {};
  for (let tool of tools) {
    let copy = embedded ? `${corpus}-embedded-${tool}` : `${corpus}-${tool}`;
    rmSync(copy, { recursive: true, force: true });
    cpSync(corpus, copy, { recursive: true });
    await $`sh -c ${commands(copy, true, embedded)[tool]}`.nothrow().quiet();
    let texts = await Promise.all(
      list(copy).map(async (path) => [
        relative(copy, path),
        await Bun.file(path).text(),
      ]),
    );
    outputs[tool] = new Map(texts as [string, string][]);
  }
  let reference = outputs.prettier;
  let matches: Record<string, number> = {};
  for (let tool of tools) {
    matches[tool] = [...reference].filter(
      ([path, text]) => outputs[tool].get(path) === text,
    ).length;
  }
  return matches;
}

function average(values: number[]) {
  return (values.reduce((sum, value) => sum + value, 0) / values.length) * 1000;
}
