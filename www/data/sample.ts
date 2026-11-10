import { readFileSync } from "node:fs";
import { join } from "node:path";

import { highlight } from "../code";

/**
 * The before and after that the home page animates between. `routes.output.ts`
 * is what `tidy --no-config routes.input.ts` prints; `bun run check` fails if
 * it drifts.
 */
let read = (name: string) =>
  readFileSync(join(import.meta.dirname, "../samples", name), "utf8").trimEnd();

export default {
  /** The hero reads these as text and tokenises them in the browser. */
  mess: read("routes.input.ts"),
  tidy: read("routes.output.ts"),
  config: highlight(read("example-config.json"), "json"),
  usage: highlight(read("usage.sh"), "bash"),
};
