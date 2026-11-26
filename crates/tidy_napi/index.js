// The native addon for this platform, or tidyrs-wasm where none loads, or when TIDYRS_WASM=1 asks for it.
import { createRequire } from "node:module";

let require = createRequire(import.meta.url);

function load() {
  if (process.env.TIDYRS_WASM === "1") return require("tidyrs-wasm");
  try {
    return require("./native.cjs");
  } catch (native) {
    try {
      return require("tidyrs-wasm");
    } catch (wasm) {
      if (wasm?.code !== "MODULE_NOT_FOUND") throw wasm;
      let where = `${process.platform}-${process.arch}`;
      let message = `tidyrs has no native addon for ${where}; install tidyrs-wasm for its WebAssembly build`;
      throw new Error(message, { cause: native });
    }
  }
}

export let { format, check } = load();
