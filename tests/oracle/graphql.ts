// Formats GraphQL files with Prettier and with tidy, under several option sets, and compares the outputs. Both rejecting
// a file counts as the same. Every output of tidy must read back the same.
//
//   bun tests/oracle/graphql.ts [dir...] [--show=n]
//
// Without folders, it reads Prettier's fixtures, the tests of pretty_graphql, and target/corpus/graphql.
import { readdirSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import * as prettier from "prettier";
import { format, type FormatOptions } from "../../crates/tidy_napi/index.js";

let args = Bun.argv.slice(2).filter((arg) => !arg.startsWith("--"));
let option = (name: string) =>
  Bun.argv.find((arg) => arg.startsWith(`--${name}=`))?.slice(name.length + 3);
let show = Number(option("show") ?? 5);
let oss = join(homedir(), "repos", "oss");
let dirs = args.length
  ? args
  : [
      join(oss, "prettier/tests/format/graphql"),
      join(oss, "pretty_graphql/tests"),
      join(import.meta.dir, "../../target/corpus/graphql"),
    ];

/** Option sets: Prettier's names, and tidy's for the same. */
let sets: {
  name: string;
  prettier: Record<string, unknown>;
  tidy: FormatOptions;
}[] = [
  { name: "default", prettier: {}, tidy: {} },
  {
    name: "no bracket spacing",
    prettier: { bracketSpacing: false },
    tidy: { bracketSpacing: false },
  },
  { name: "width 40", prettier: { printWidth: 40 }, tidy: { width: 40 } },
  { name: "tabs", prettier: { useTabs: true }, tidy: { tabs: true } },
];

function files(path: string): string[] {
  try {
    if (statSync(path).isFile())
      return /\.(graphql|gql|graphqls)$/i.test(path) ? [path] : [];
    return readdirSync(path).flatMap((name) =>
      name === "node_modules" ? [] : files(join(path, name)),
    );
  } catch {
    return [];
  }
}

let counts = {
  same: 0,
  different: 0,
  rejected: 0,
  "tidy rejects": 0,
  "tidy accepts": 0,
  unstable: 0,
};
let shown = 0;
let report = (title: string, ...texts: string[]) => {
  if (shown++ >= show) return;
  console.log(`--- ${title}`);
  for (let text of texts) console.log(JSON.stringify(text));
};
let attempt = (run: () => string) => {
  try {
    return run();
  } catch {
    return undefined;
  }
};

for (let path of dirs.flatMap(files)) {
  let text = await Bun.file(path).text();
  for (let set of sets) {
    let expected = await prettier
      .format(text, { parser: "graphql", ...set.prettier })
      .catch(() => undefined);
    let tidy = attempt(() => format(text, { parser: "graphql", ...set.tidy }));
    if (expected === undefined && tidy === undefined) {
      counts.rejected++;
    } else if (tidy === undefined) {
      counts["tidy rejects"]++;
      report(`tidy rejects, ${set.name}: ${path}`, text);
    } else if (expected === undefined) {
      counts["tidy accepts"]++;
      report(`tidy accepts, ${set.name}: ${path}`, text, tidy);
    } else if (tidy === expected) {
      counts.same++;
    } else {
      counts.different++;
      report(`different, ${set.name}: ${path}`, text, expected, tidy);
    }
    if (tidy !== undefined) {
      let again = attempt(() =>
        format(tidy, { parser: "graphql", ...set.tidy }),
      );
      if (again !== tidy) {
        counts.unstable++;
        report(`unstable, ${set.name}: ${path}`, tidy, again ?? "!error");
      }
    }
  }
}
console.log(counts);
