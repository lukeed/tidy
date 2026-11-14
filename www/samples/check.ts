/**
 * Keeps the home page honest: `routes.output.ts` must be what tidy prints for
 * `routes.input.ts`. Run with `bun run check` from `www/`.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

let dir = import.meta.dirname;
let binary = join(dir, "../../target/release/tidy");

let run = spawnSync(binary, ["--no-config", join(dir, "routes.input.ts")], {
  encoding: "utf8",
});
if (run.error) {
  console.error(
    `cannot run ${binary} — build it first: cargo build --release -p tidy`,
  );
  process.exit(2);
}

let expected = readFileSync(join(dir, "routes.output.ts"), "utf8");
if (run.stdout !== expected) {
  console.error(
    "samples/routes.output.ts is not what tidy prints. Regenerate it:",
  );
  console.error(
    `  ${binary} --no-config samples/routes.input.ts > samples/routes.output.ts`,
  );
  process.exit(1);
}

console.log("samples are what tidy prints");
