import { $ } from "bun";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import { config } from "./config";

// Usage: mise run oracle:markdown:shrink -- <file> [--<option> <value>...]
// Shrinks a Markdown file that tidy breaks, removing lines while formatting it still changes
// the events the semantics oracle compares, or changes it again on a second pass. Prints what is
// left, usually a few lines to turn into a test.

let [path, ...options] = Bun.argv.slice(2);
if (!path)
  throw new Error(
    "usage: mise run oracle:markdown:shrink -- <file> [--<option> <value>...]",
  );

let root = new URL("../../..", import.meta.url).pathname;
await $`cargo build --release -p tidy`.cwd(root).quiet();
await $`cargo build --release -p tidy --example markdown_events`
  .cwd(root)
  .quiet();
let { target_directory } = await $`cargo metadata --format-version 1 --no-deps`
  .cwd(root)
  .json();
let tidy = join(target_directory, "release", "tidy");
let scratch = join(target_directory, "oracle", "shrink");
await mkdir(scratch, { recursive: true });
let settings = await config(options, scratch);

let lines = (await Bun.file(path).text()).split("\n");
if (!(await broken(lines)))
  throw new Error(`${path}: formatting keeps its meaning and settles`);

for (let size of [64, 32, 16, 8, 4, 2, 1]) {
  let index = 0;
  while (index < lines.length) {
    let trial = lines.toSpliced(index, size);
    if (trial.length > 0 && (await broken(trial))) lines = trial;
    else index += size;
  }
}
console.log(lines.join("\n"));

/** Whether formatting `lines` changes their events or doesn't settle. */
async function broken(lines: string[]): Promise<boolean> {
  let source = lines.join("\n").trimEnd() + "\n";
  let once = await format(source);
  if ((await format(once)) !== once) return true;
  return (await events(source)) !== (await events(once));
}

async function format(source: string): Promise<string> {
  return $`${tidy} --parser markdown --config ${settings} < ${new Response(source)}`.text();
}

async function events(source: string): Promise<string> {
  let file = join(scratch, "events.md");
  await Bun.write(file, source);
  return $`${join(target_directory, "release", "examples", "markdown_events")} ${file}`.text();
}
