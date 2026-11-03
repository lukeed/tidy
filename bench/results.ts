// Writes the results of a bench to a markdown file in bench/results, with what it ran on: the date, the commit, the
// machine and the version of each tool.
import { $ } from "bun";
import { mkdirSync } from "node:fs";
import { cpus, release } from "node:os";
import { dirname, join } from "node:path";
import { format } from "../crates/tidy_napi/index.js";

let root = new URL("..", import.meta.url).pathname;

/**
 * A part of the results: a heading, a line about its corpus, and a table with a
 * heading row and a row for each tool.
 */
export type Section = {
  title?: string;
  summary: string;
  head: string[];
  rows: string[][];
};

/**
 * Writes `bench/results/<path>`: the title, each section, and how the bench
 * ran.
 */
export async function record(
  path: string,
  title: string,
  sections: Section[],
  tools: string[],
) {
  let line = (cells: string[]) => `| ${cells.join(" | ")} |`;
  let lines = [`# ${title}`, ""];
  for (let { title, summary, head, rows } of sections) {
    if (title) lines.push(`## ${title}`, "");
    let numeric = (index: number) =>
      rows.every((row) => row[index] === "" || /^[\d.]/.test(row[index]));
    lines.push(
      summary,
      "",
      line(head),
      line(head.map((_, index) => (numeric(index) ? "---:" : "---"))),
    );
    lines.push(...rows.map(line), "");
  }
  lines.push(
    "## Setup",
    "",
    ...(await setup()).map((it) => `- ${it}`),
    `- Tools: ${tools.join(", ")}`,
    "",
  );
  let file = join(root, "bench/results", path);
  mkdirSync(dirname(file), { recursive: true });
  await Bun.write(
    file,
    format(lines.join("\n"), { parser: "markdown", embedded: false }),
  );
}

async function setup() {
  let commit = (await $`git rev-parse --short HEAD`.cwd(root).text()).trim();
  let changed =
    (
      await $`git status --porcelain -- crates bench :!bench/results`
        .cwd(root)
        .text()
    ).trim() !== "";
  let model =
    (await $`sysctl -n machdep.cpu.brand_string`.nothrow().text()).trim() ||
    cpus()[0].model;
  let system =
    process.platform === "darwin"
      ? `macOS ${(await $`sw_vers -productVersion`.text()).trim()}`
      : release();
  return [
    `Date: ${new Date().toLocaleDateString("sv")}`,
    `Commit: ${commit}${changed ? ", with changes not committed" : ""}`,
    `Machine: ${model}, ${cpus().length} cores, ${system}`,
  ];
}

/** How a corpus or language is written in prose. */
export function language(name: string) {
  let names: Record<string, string> = {
    less: "Less",
    handlebars: "Handlebars",
    markdown: "Markdown",
    dockerfile: "Dockerfiles",
    graphql: "GraphQL",
    vue: "Vue",
    pages: "HTML pages",
    all: "all corpora",
    "all-but-xml": "all corpora but XML",
  };
  return names[name] ?? name.toUpperCase();
}

/** The versions of `names` in a Cargo.lock, as `name version`. */
export async function crates(lockfile: string, names: string[]) {
  let text = await Bun.file(join(root, lockfile)).text();
  return names.map((name) => {
    let version = text.match(
      new RegExp(`name = "${name}"\\nversion = "([^"]+)"`),
    )?.[1];
    return `${name} ${version ?? "?"}`;
  });
}

/** The first version number that `command` prints. */
export async function version(command: string[]) {
  let text = await $`${command}`.nothrow().quiet().text();
  return text.match(/\d+\.\d+\.\d+/)?.[0] ?? "?";
}
