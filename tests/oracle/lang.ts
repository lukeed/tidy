// Prettier's parser names, which the oracles pass to Prettier, against the binding's `lang` and hook requests.
import type { Embed, Options } from "../../crates/tidy_napi/index.js";

let langs: Record<string, string> = {
  babel: "js",
  acorn: "js",
  espree: "js",
  meriyah: "js",
  flow: "js",
  "babel-flow": "js",
  "babel-ts": "ts",
  typescript: "ts",
  markdown: "md",
  glimmer: "hbs",
};

/**
 * The options that pick the language of Prettier's `parser`. JSON.stringify's
 * has no `lang`, only file names.
 */
export function languageOf(parser: string): Pick<Options, "lang" | "filepath"> {
  if (parser === "json-stringify") return { filepath: "package.json" };
  return { lang: langs[parser] ?? parser };
}

let blocks: Record<string, string> = {
  js: "babel",
  ts: "typescript",
  md: "markdown",
  hbs: "glimmer",
};

let hosted: Record<string, string> = {
  "vue/expression/js": "__vue_expression",
  "vue/expression/ts": "__vue_ts_expression",
  "vue/event/js": "__vue_event_binding",
  "vue/event/ts": "__vue_ts_event_binding",
  "vue/bindings/js": "__vue_bindings",
  "vue/bindings/ts": "__vue_ts_bindings",
  "vue/for/js": "__vue_for_left",
  "vue/for/ts": "__vue_ts_for_left",
  "vue/generic/js": "__vue_generic",
  "vue/generic/ts": "__vue_generic",
  "ng/directive/js": "__ng_directive",
  "ng/directive/ts": "__ng_directive",
  "ng/block/html": "angular",
};

/**
 * Prettier's parser for the code of `request`, as tidy's `Request::parser`
 * names it.
 */
export function parser({ lang, kind, host }: Embed.Request): string {
  let named = host && hosted[`${host}/${kind}/${lang}`];
  if (named) return named;
  if (!host && kind === "expression" && (lang === "js" || lang === "ts"))
    return `__${lang}_expression`;
  if (!host && (kind === "block" || kind === "frontmatter"))
    return blocks[lang] ?? lang;
  return lang;
}
