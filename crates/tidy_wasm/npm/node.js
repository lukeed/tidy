import { createRequire } from "node:module";

let tidy = createRequire(import.meta.url)("./tidy.cjs");

export let { format, check, config, version } = tidy;

/**
 * The wasm is loaded already in Node; this keeps `await init()` working there
 * too.
 */
export default async function init() {}
