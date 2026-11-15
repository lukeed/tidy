import { existsSync } from "node:fs";
import * as path from "node:path";

import { expect, test } from "bun:test";

import * as languages from "./languages";
import manifest from "../package.json";

test("activates on each language it formats", () => {
  let events = manifest.activationEvents.map((event) =>
    event.replace(/^onLanguage:/, ""),
  );
  expect(events.toSorted()).toEqual(
    Object.keys(languages.EXTENSIONS).toSorted(),
  );
});

test("tidy knows the name of each untitled document", () => {
  let bin = process.env.TIDY_BIN || built();
  expect(existsSync(bin), `${bin} is missing: run cargo build -p tidy`).toBe(
    true,
  );
  for (let extension of Object.values(languages.EXTENSIONS)) {
    let run = Bun.spawnSync([bin, `--stdin=untitled.${extension}`], {
      stdin: Buffer.from(""),
    });
    expect({ extension, error: run.stderr.toString() }).toEqual({
      extension,
      error: "",
    });
  }
});

/**
 * The repository's debug build, never the `tidy` on the PATH, which on macOS is
 * HTML Tidy.
 */
function built(): string {
  let metadata = Bun.spawnSync(
    ["cargo", "metadata", "--format-version=1", "--no-deps"],
    {
      cwd: import.meta.dir,
    },
  );
  let target: unknown = JSON.parse(metadata.stdout.toString()).target_directory;
  if (typeof target !== "string")
    throw new Error("cargo metadata has no target_directory");
  return path.join(target, "debug", "tidy");
}
