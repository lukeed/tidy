// Formats JavaScript and TypeScript files with Prettier and with tidy, and compares the outputs. Both rejecting a file
// counts as the same. Where Prettier's output changes when formatted again, tidy prints Prettier's fixed point instead,
// so that a formatted file passes `tidy check`. Every output of tidy must read back the same. Embedded code isn't
// formatted by either, unless `--embedded` is given: then both format the CSS, GraphQL, Markdown and HTML in templates.
//
//   bun tests/oracle/js.ts [dir...] [--show=n] [--list=file] [--embedded]
//
// Without folders, it reads Prettier's fixtures of JavaScript, JSX and TypeScript, and the bench corpus of bench/js.ts.
import { readdirSync, statSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import * as prettier from "prettier";
import { format } from "../../crates/tidy_napi/index.js";

let args = Bun.argv.slice(2).filter((arg) => !arg.startsWith("--"));
let option = (name: string) =>
  Bun.argv.find((arg) => arg.startsWith(`--${name}=`))?.slice(name.length + 3);
let show = Number(option("show") ?? 3);
let list = option("list");
let embedded = Bun.argv.includes("--embedded");
let fixtures = join(homedir(), "repos/oss/prettier/tests/format");
let dirs = args.length
  ? args
  : [
      join(fixtures, "js"),
      join(fixtures, "jsx"),
      join(fixtures, "typescript"),
      join(tmpdir(), "tidy-bench-js"),
    ];
let script = /\.(js|jsx|mjs|cjs|ts|tsx|mts|cts)$/i;

function files(path: string): string[] {
  try {
    if (statSync(path).isFile())
      return script.test(path) && !path.endsWith(".js.flow") ? [path] : [];
    return readdirSync(path).flatMap((name) =>
      name === "node_modules" ||
      name === "__snapshots__" ||
      /-(prettier|tidy|oxfmt|biome|bun)$/.test(name)
        ? []
        : files(join(path, name)),
    );
  } catch {
    return [];
  }
}

let counts = {
  same: 0,
  "same as Prettier's fixed point": 0,
  different: 0,
  rejected: 0,
  "tidy rejects": 0,
  "tidy accepts": 0,
  unstable: 0,
};
let differing: string[] = [];
let shown = 0;
let report = (title: string, ...texts: string[]) => {
  if (shown++ >= show) return;
  console.log(`--- ${title}`);
  for (let text of texts)
    console.log(text.length > 3000 ? `${text.slice(0, 3000)}…` : text);
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
  let parser = /\.(ts|tsx|mts|cts)$/i.test(path) ? "typescript" : "babel";
  let expected = await prettier
    .format(text, {
      parser,
      filepath: path,
      embeddedLanguageFormatting: embedded ? "auto" : "off",
    })
    .catch(() => undefined);
  let tidy = attempt(() =>
    format(text, { filepath: path, embedded, js: { jsdoc: false } }),
  );
  if (expected === undefined && tidy === undefined) {
    counts.rejected++;
  } else if (tidy === undefined) {
    counts["tidy rejects"]++;
    differing.push(`rejects ${path}`);
  } else if (expected === undefined) {
    counts["tidy accepts"]++;
    differing.push(`accepts ${path}`);
  } else if (tidy === expected) {
    counts.same++;
  } else if (tidy === (await fixedPoint(expected, parser, path))) {
    counts["same as Prettier's fixed point"]++;
  } else {
    counts.different++;
    differing.push(path);
    report(`different: ${path}`, expected, tidy);
  }
  if (tidy !== undefined) {
    let again = attempt(() =>
      format(tidy, { filepath: path, embedded, js: { jsdoc: false } }),
    );
    if (again !== tidy) {
      counts.unstable++;
      differing.push(`unstable ${path}`);
    }
  }
}
/**
 * What Prettier formats `output`, its own, to when formatted again until it
 * stays the same, if it does.
 */
async function fixedPoint(output: string, parser: string, path: string) {
  let options = {
    parser,
    filepath: path,
    embeddedLanguageFormatting: embedded ? "auto" : "off",
  } as const;
  for (let round = 0; round < 5; round++) {
    let again = await prettier.format(output, options).catch(() => undefined);
    if (again === undefined || again === output) return again;
    output = again;
  }
  return undefined;
}

if (list) await Bun.write(list, `${differing.join("\n")}\n`);
console.log(counts);
