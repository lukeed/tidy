import { join } from "node:path";

/**
 * Writes a tidy.json in `dir` for options given as flags, `--wrap always
 * --width 40`, and returns its path: `width` and `ignore` at the top, and the
 * rest in its `markdown` section.
 */
export async function config(flags: string[], dir: string): Promise<string> {
  let top: Record<string, unknown> = {};
  let markdown: Record<string, unknown> = {};
  for (let index = 0; index < flags.length; index += 2) {
    let name = flags[index].replace(/^--/, "");
    let value = flags[index + 1];
    if (name === "width") top.width = Number(value);
    else if (name === "ignore") top.ignore = value;
    else markdown[name] = value;
  }
  let path = join(dir, "tidy.json");
  await Bun.write(path, JSON.stringify({ ...top, markdown }));
  return path;
}
