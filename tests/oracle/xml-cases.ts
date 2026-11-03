// Records what @prettier/plugin-xml prints for the edge cases in `crates/tidy/tests/xml.json`, whose `options` are
// the plugin's. Run it after adding a case; `cargo test --test xml` then checks tidy against the recorded outputs.
//
//   bun tests/oracle/xml-cases.ts
import { join } from "node:path";
import plugin from "@prettier/plugin-xml";
import * as prettier from "prettier";

type Case = {
  input: string;
  options?: Record<string, unknown>;
  output?: string;
};

let path = join(import.meta.dir, "../../crates/tidy/tests/xml.json");
let cases: Case[] = await Bun.file(path).json();
for (let test of cases) {
  try {
    test.output = await prettier.format(test.input, {
      parser: "xml",
      plugins: [plugin],
      ...test.options,
    });
  } catch {
    test.output = "!error";
  }
}
await Bun.write(path, `${JSON.stringify(cases, null, 1)}\n`);
