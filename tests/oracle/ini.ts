// Formats INI files with prettier-plugin-ini 1.3.0 and with tidy, under several option sets, and compares the outputs
// where the plugin accepts the file. Where it rejects the file, tidy formats it in kept mode, and only its stability
// is checked. Git config files are always in kept mode.
//
//   bun tests/oracle/ini.ts files [dir] [--show=n]
//   bun tests/oracle/ini.ts fuzz [count] [--seed=n] [--show=n]
import { readdirSync } from "node:fs";
import { join } from "node:path";
import plugin from "prettier-plugin-ini";
import * as prettier from "prettier";
import { format, type FormatOptions } from "../../crates/tidy_napi/index.js";

// tidy's deviations from the plugin's output, applied to the plugin's tree: an empty section has one blank line after
// it, and a list without values is a key with an empty value.
type Node = {
  type: string;
  value: Node[] | string;
  startLine: number;
  endLine: number;
};
let parse = plugin.parsers.ini.parse;
plugin.parsers.ini.parse = (text: string, options: unknown) => {
  let root: Node = parse(text, options);
  let nodes = root.value as Node[];
  for (let [index, node] of nodes.entries()) {
    if (
      node.type === "section" &&
      node.value.length === 0 &&
      nodes[index + 1]
    ) {
      node.endLine = nodes[index + 1].startLine;
    }
  }
  for (let node of [root, ...nodes].flatMap((node) =>
    Array.isArray(node.value) ? node.value : [],
  )) {
    if (node.type === "list" && node.value.length === 0)
      Object.assign(node, { type: "param", value: "" });
  }
  return root;
};

let args = Bun.argv.slice(2).filter((arg) => !arg.startsWith("--"));
let option = (name: string) =>
  Bun.argv.find((arg) => arg.startsWith(`--${name}=`))?.slice(name.length + 3);
let show = Number(option("show") ?? 5);

/** Option sets: the plugin's names, and tidy's for the same. */
let sets: {
  name: string;
  prettier: Record<string, unknown>;
  tidy: FormatOptions;
}[] = [
  {
    name: "default",
    prettier: {},
    tidy: { ini: { spaceAroundEquals: false } },
  },
  { name: "spaces", prettier: { iniSpaceAroundEquals: true }, tidy: {} },
  {
    name: "indent 4",
    prettier: { tabWidth: 4 },
    tidy: { indent: 4, ini: { spaceAroundEquals: false } },
  },
  {
    name: "tabs",
    prettier: { useTabs: true },
    tidy: { tabs: true, ini: { spaceAroundEquals: false } },
  },
];

type Result = {
  same: number;
  different: number;
  rejected: number;
  unstable: number;
  shown: number;
};

async function compare(
  name: string,
  text: string,
  git: boolean,
  result: Result,
) {
  for (let set of sets) {
    let parser = git ? "gitconfig" : "ini";
    let tidy = format(text, { parser, ...set.tidy });
    let again = format(tidy, { parser, ...set.tidy });
    if (again !== tidy) {
      result.unstable++;
      report(result, `unstable, ${set.name}: ${name}`, text, tidy, again);
    }
    if (git) continue;
    let expected: string;
    try {
      expected = await prettier.format(text, {
        parser: "ini",
        plugins: [plugin],
        ...set.prettier,
      });
    } catch {
      result.rejected++;
      continue;
    }
    if (tidy === expected) {
      result.same++;
    } else {
      result.different++;
      report(result, `different, ${set.name}: ${name}`, text, expected, tidy);
    }
  }
}

function report(
  result: Result,
  title: string,
  input: string,
  a: string,
  b: string,
) {
  if (result.shown++ >= show) return;
  console.log(`--- ${title}`);
  console.log(JSON.stringify(input));
  console.log(JSON.stringify(a));
  console.log(JSON.stringify(b));
}

async function files(dir: string) {
  let result: Result = {
    same: 0,
    different: 0,
    rejected: 0,
    unstable: 0,
    shown: 0,
  };
  for (let name of readdirSync(dir).sort()) {
    let text = await Bun.file(join(dir, name)).text();
    await compare(
      name,
      text,
      /(^|_)\.?git(config|modules)$/.test(name),
      result,
    );
  }
  return result;
}

/** A pool of lines around the edges of the plugin's patterns. */
let pool = [
  "",
  " ",
  "\t",
  "   ",
  "[a]",
  " [ b ] ",
  "[c]x]",
  "[d] ; note",
  "[e]#",
  "[]",
  "k=v",
  " k = v ",
  "key =",
  "key=",
  "=v",
  " =v",
  "  = v",
  "a=b=c",
  "x　=　y",
  "k = v \\",
  "k=  ",
  "# c",
  "  ; c",
  "#",
  ";",
  "k: v",
  "bare",
  "  cont",
  "\tcont",
  "    deep = x",
  "  [f]",
  "  # in",
  " ",
  " ",
  "a= b",
  "a=b ",
  "[ ]",
  "﻿k=v",
];

/** Mulberry32 */
function random(seed: number) {
  return () => {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

async function fuzz(count: number, seed: number) {
  let result: Result = {
    same: 0,
    different: 0,
    rejected: 0,
    unstable: 0,
    shown: 0,
  };
  let next = random(seed);
  let pick = <T>(list: T[]) => list[Math.floor(next() * list.length)];
  for (let index = 0; index < count; index++) {
    let lines = Array.from({ length: 1 + Math.floor(next() * 10) }, () =>
      pick(pool),
    );
    let text =
      lines.join(pick(["\n", "\n", "\n", "\r\n", "\r"])) + pick(["", "\n"]);
    await compare(`#${index}`, text, next() < 0.1, result);
  }
  return result;
}

let result =
  args[0] === "fuzz"
    ? await fuzz(Number(args[1] ?? 20000), Number(option("seed") ?? 1))
    : await files(args[1] ?? join(import.meta.dir, "../../target/corpus/ini"));
let { shown, ...counts } = result;
console.log(counts);
