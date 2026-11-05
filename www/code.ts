import { language as bash } from "@twinkleplop/bash";
import { language as css } from "@twinkleplop/css";
import { language as html } from "@twinkleplop/html";
import { language as javascript } from "@twinkleplop/javascript";
import { language as json } from "@twinkleplop/json";
import { language as markdown } from "@twinkleplop/markdown";
import { language as rust } from "@twinkleplop/rust";
import { language as toml } from "@twinkleplop/toml";
import { language as typescript } from "@twinkleplop/typescript";
import { language as yaml } from "@twinkleplop/yaml";

import type { Build } from "bongo";
import type { Fence } from "@bongo/markdown";

const NAMES: Record<string, string> = {
  bash: "Shell",
  css: "CSS",
  diff: "Diff",
  html: "HTML",
  javascript: "JavaScript",
  js: "JavaScript",
  json: "JSON",
  jsonc: "JSON",
  markdown: "Markdown",
  md: "Markdown",
  rs: "Rust",
  rust: "Rust",
  scss: "SCSS",
  sh: "Shell",
  toml: "TOML",
  ts: "TypeScript",
  typescript: "TypeScript",
  vue: "Vue",
  xml: "XML",
  yaml: "YAML",
  yml: "YAML",
};

const ESCAPE: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
};

const escape = (text: string) =>
  text.replace(/[&<>"]/g, (char) => ESCAPE[char]);

const HIGHLIGHTERS: Record<string, (code: string) => string> = {
  bash: bash(),
  css: css(),
  html: html(),
  javascript: javascript(),
  json: json(),
  markdown: markdown(),
  rust: rust(),
  toml: toml(),
  typescript: typescript(),
  yaml: yaml(),
};

/**
 * The twinkleplop highlighters for each fence language the docs use. Vue and
 * SCSS borrow the HTML and CSS grammars, as tidy itself treats them.
 */
export const languages = {
  ...HIGHLIGHTERS,
  scss: "css",
  less: "css",
  vue: "html",
  xml: "html",
};

/**
 * Give each code block a title bar with its file name or language, and a copy
 * button. A block with no language gets a plain frame, as terminal transcripts
 * do.
 */
export const fence: Fence = (html, block) => {
  if (!block.lang)
    return `<figure class="code"><pre class="twinkleplop"><code>${escape(block.code)}</code></pre></figure>`;
  let label = block.filename
    ? `<span>${escape(block.filename)}</span><span class="lang">${escape(block.lang)}</span>`
    : `<span>${NAMES[block.lang] ?? escape(block.lang)}</span>`;
  return `<figure class="code"><div class="bar">${label}<button class="copy" type="button" data-copy-code>Copy</button></div>${html}</figure>`;
};

// A code span this short never wraps, so an option name keeps its dashes.
const SHORT = 32;

/**
 * Let wide tables scroll, and mark each short code span and each table cell of
 * code spans alone, such as `-w, --write`, which stay on one line.
 */
export const transform: Build.Transform = (text, ctx) =>
  ctx.kind === "page"
    ? text
        .replace(
          /<table>[\s\S]*?<\/table>/g,
          (table) => `<div class="tbl">${table}</div>`,
        )
        .replace(
          /<td>((?:<code>[^<]*<\/code>(?:, )?){2,})<\/td>/g,
          '<td class="codes">$1</td>',
        )
        .replace(/<code>([^<]*)<\/code>/g, (span, inner: string) =>
          inner.length > SHORT ? span : `<code class="short">${inner}</code>`,
        )
    : text;

/**
 * Highlight source as a bare `<pre class="twinkleplop">` block, for the panes
 * of the home page, which have their own frame.
 */
export function highlight(code: string, lang: string): string {
  let fn = HIGHLIGHTERS[lang];
  if (!fn) throw new Error(`No highlighter for "${lang}"`);
  return fn(code);
}
