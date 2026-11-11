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
  let bin = process.env.TIDY_BIN || "tidy";
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
