// Records what Prettier prints for the edge cases in `crates/tidy/tests/json.json`, whose `options` are Prettier's.
// Run it after adding a case; `cargo test --test json` then checks tidy against the recorded outputs.
//
//   bun tests/oracle/json-cases.ts
import { join } from "node:path";
import * as prettier from "prettier";

type Case = {
  input: string;
  options?: Record<string, unknown>;
  output?: string;
};

let path = join(import.meta.dir, "../../crates/tidy/tests/json.json");
let cases: Case[] = await Bun.file(path).json();
for (let test of cases) {
  try {
    test.output = await prettier.format(test.input, {
      parser: "json",
      ...test.options,
    });
  } catch {
    test.output = "!error";
  }
}
await Bun.write(path, `${JSON.stringify(cases, null, 1)}\n`);
