import { readFileSync } from "node:fs";
import { join } from "node:path";

/** The version of the workspace, which every crate and the npm package share. */
let manifest = readFileSync(
  join(import.meta.dirname, "../../Cargo.toml"),
  "utf8",
);
let version = /^\s*version\s*=\s*"([^"]+)"/m.exec(
  manifest.slice(manifest.indexOf("[workspace.package]")),
)?.[1];

export default {
  version: version ?? "0.0.0",
  prettier: "3.9.9",
};
