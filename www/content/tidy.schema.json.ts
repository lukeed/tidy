import { readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * The schema tidy itself prints, served so an editor can point `$schema` at it.
 * `mise run schema` rewrites the file; `mise run fmt` fails when it is stale.
 */
export default () =>
  JSON.parse(
    readFileSync(join(import.meta.dirname, "../../crates/tidy_napi/tidy.schema.json"), "utf8"),
  );
