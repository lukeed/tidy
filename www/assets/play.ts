import { language as bash } from "@twinkleplop/bash";
import { language as css } from "@twinkleplop/css";
import { language as html } from "@twinkleplop/html";
import { language as javascript } from "@twinkleplop/javascript";
import { language as json } from "@twinkleplop/json";
import { language as markdown } from "@twinkleplop/markdown";
import { language as toml } from "@twinkleplop/toml";
import { language as typescript } from "@twinkleplop/typescript";
import { language as yaml } from "@twinkleplop/yaml";

import init, { format, version } from "./wasm/tidy.js";

/**
 * A grammar per parser; one tidy formats but twinkleplop has no grammar for
 * stays plain.
 */
const GRAMMAR: Record<string, () => (code: string) => string> = {
  typescript,
  babel: javascript,
  html,
  vue: html,
  handlebars: html,
  xml: html,
  css,
  scss: css,
  less: css,
  json,
  jsonc: json,
  json5: json,
  yaml,
  toml,
  markdown,
  dockerfile: bash,
};

let grammars = new Map<string, (code: string) => string>();

function paint(code: string, parser: string): string {
  let make = GRAMMAR[parser];
  if (!make) return escape(code);
  let run = grammars.get(parser) ?? make();
  grammars.set(parser, run);
  try {
    let html = run(code);
    // The grammars return a full <pre><code> frame; the shade supplies its own.
    return html
      .replace(/^<pre[^>]*><code[^>]*>/, "")
      .replace(/<\/code><\/pre>$/, "");
  } catch {
    return escape(code);
  }
}

const ESCAPE: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
};

function escape(text: string): string {
  return text.replace(/[&<>]/g, (char) => ESCAPE[char]);
}

type Parser = string;

const SAMPLES: Record<string, string> = {
  typescript: `import {readFile} from 'node:fs/promises'\n\ntype Route={\npath:string,run:(req:Request)=>Promise<Response>}\n\nconst routes:Route[]=[\n{path:'/',run:()=>page("index.html")},\n  {path:"/up",run:async()=>new Response('ok')},\n]\n`,
  babel: `const routes=[{path:'/',run:()=>page("index.html")},{path:"/up",run:async()=>new Response('ok')}]\n\nexport function match(path){return routes.find(r=>r.path===path)??null}\n`,
  html: `<section class=hero><h1 class="title">tidy</h1><p>Prettier's style, at native speed.</p>\n<a class=btn href="/docs/">Docs</a><a class=btn href=https://github.com/lukeed/tidy>Source</a></section>\n`,
  vue: `<template><div   class="card"><slot/></div></template>\n<script setup lang="ts">const a   =1</script>\n<style scoped>.card{padding:1rem;color:red}</style>\n`,
  handlebars: `{{#if user}}<p class=greeting>Hello {{user.name}}</p>{{else}}<p>Hello stranger</p>{{/if}}\n`,
  css: `:root{--gap:1rem;--brand:#FF4D00}\n.card,.panel{display:grid;gap:var(--gap);padding:calc(var(--gap)*1.5)}\n@media (max-width:640px){.card{grid-template-columns:1fr}}\n`,
  scss: `$gap:1rem;\n.card{padding:$gap;&:hover{box-shadow:0 1px 2px rgba(0,0,0,.06)}\n.title{font-weight:600}}\n`,
  less: `@gap:1rem;\n.card{padding:@gap;.title{font-weight:600}}\n`,
  json: `{"name":"tidy","version":"0.1.0","keywords":["formatter","prettier"],"engines":{"node":">=22"}}\n`,
  jsonc: `{\n// the line width\n"width":100,"js":{"semi":false,},}\n`,
  json5: `{width:100,js:{semi:false,quote:'single'},}\n`,
  yaml: `name:   ci\non:\n  push:\n    branches:   [main]\njobs:\n  test:\n    runs-on:   ubuntu-latest\n`,
  toml: `[package]\nname="tidy"\nversion="0.1.0"\n[dependencies]\nserde={version="1",features=["derive"]}\n`,
  markdown: `#  tidy\n\n*  Prettier's style\n*  at native speed\n\n|  a |  b |\n| -- | -- |\n|  1 |  2 |\n`,
  graphql: `query User($id:ID!){user(id:$id){id,name,email,posts(first:10){title}}}\n`,
  xml: `<note><to>You</to><from>tidy</from><body   attr="1">Read me</body></note>\n`,
  dockerfile: `FROM   node:22-alpine\nWORKDIR   /app\nCOPY package.json .\nRUN npm ci --omit=dev\nCMD ["node","index.js"]\n`,
  ini: `[server]\nhost=localhost\nport=8080\n[client]\nretries=3\n`,
};

const EXTENSION: Record<string, string> = {
  typescript: "ts",
  babel: "js",
  html: "html",
  vue: "vue",
  handlebars: "hbs",
  css: "css",
  scss: "scss",
  less: "less",
  json: "json",
  jsonc: "jsonc",
  json5: "json5",
  yaml: "yaml",
  toml: "toml",
  markdown: "md",
  graphql: "graphql",
  xml: "xml",
  dockerfile: "Dockerfile",
  ini: "ini",
};

let input: HTMLTextAreaElement;
let config: HTMLTextAreaElement;
let output: HTMLPreElement;
let parser: HTMLSelectElement;
let state: HTMLElement;
let inStat: HTMLElement;
let outStat: HTMLElement;
let tabs: HTMLElement;
let panes: Record<string, HTMLElement>;
let inShade: HTMLElement;
let cfgShade: HTMLElement;
let list: HTMLElement;

let ready = false;
let loading: Promise<unknown> | undefined;

/** The whole page state, small enough to live in the URL. */
function share(): string {
  let payload = JSON.stringify({
    p: parser.value,
    s: input.value,
    c: config.value,
  });
  return btoa(String.fromCharCode(...new TextEncoder().encode(payload)));
}

function restore(): boolean {
  let hash = location.hash.slice(1);
  if (!hash) return false;
  try {
    let bytes = Uint8Array.from(atob(hash), (char) => char.charCodeAt(0));
    let { p, s, c } = JSON.parse(new TextDecoder().decode(bytes));
    if (typeof p === "string") parser.value = p;
    if (typeof s === "string") input.value = s;
    if (typeof c === "string") config.value = c;
    return true;
  } catch {
    return false;
  }
}

/** The shade must end in a line break, or the last empty line has no height. */
function shade(target: HTMLElement, text: string, parser: string) {
  target.innerHTML = paint(text.endsWith("\n") ? text : `${text}\n`, parser);
}

function run() {
  shade(inShade, input.value, parser.value);
  shade(
    cfgShade,
    config.value,
    config.value.trim().startsWith("{") ? "json" : "toml",
  );
  if (!ready) return;

  let source = input.value;
  inStat.textContent = `${source.length.toLocaleString()} chars`;

  if (!source.trim()) {
    output.textContent = "";
    outStat.textContent = "";
    output.classList.remove("failed");
    return;
  }

  let name = EXTENSION[parser.value];
  let filepath = name === "Dockerfile" ? name : `playground.${name}`;
  let clock = performance.now();
  try {
    let text = format(source, parser.value, config.value, filepath);
    let took = performance.now() - clock;
    output.innerHTML = paint(text, parser.value);
    output.classList.remove("failed");
    outStat.textContent = `${took < 1 ? took.toFixed(2) : Math.round(took)} ms`;
  } catch (error) {
    output.textContent = error instanceof Error ? error.message : String(error);
    output.classList.add("failed");
    outStat.textContent = "error";
  }
}

function sample() {
  input.value = SAMPLES[parser.value] ?? "";
}

/** Which editor the left column shows; the output stays beside it either way. */
function show(which: string) {
  for (let tab of tabs.querySelectorAll<HTMLElement>("[data-tab]")) {
    tab.setAttribute("aria-selected", String(tab.dataset.tab === which));
  }
  for (let [name, pane] of Object.entries(panes)) {
    pane.hidden = name !== which;
  }
  if (which === "config") config.focus();
}

// ───────────────────────────── completion ─────────────────────────────

type Property = {
  type?: string;
  enum?: string[];
  default?: unknown;
  description?: string;
};
type Schema = {
  properties: Record<
    string,
    Property & { properties?: Record<string, Property> }
  >;
};

let schema: Schema | undefined;
let hits: { key: string; property: Property }[] = [];
let picked = 0;

/** Which object the caret sits in: a language section, or the top level. */
function scopeAt(text: string, caret: number): string | null {
  let depth = 0;
  let quoted = false;
  for (let at = caret - 1; at >= 0; at--) {
    let char = text[at];
    if (char === '"' && text[at - 1] !== "\\") quoted = !quoted;
    if (quoted) continue;
    if (char === "}") depth++;
    else if (char === "{") {
      if (depth === 0) {
        // The key that opened this object, if any, names the section.
        let before = text.slice(0, at);
        let name = /"([A-Za-z]+)"\s*:\s*$/.exec(
          before.trimEnd().endsWith(":") ? before : before,
        );
        return name ? name[1] : null;
      }
      depth--;
    }
  }
  return null;
}

/** The partial key being typed, and where it starts. */
function wordAt(
  text: string,
  caret: number,
): { word: string; start: number } | null {
  let at = caret;
  while (at > 0 && /[A-Za-z]/.test(text[at - 1]!)) at--;
  let word = text.slice(at, caret);
  let before = text.slice(0, at).trimEnd();
  let opens = before.endsWith("{") || before.endsWith(",") || before === "";
  if (!opens && !before.endsWith('"')) return null;
  return { word, start: at };
}

function hide() {
  list.hidden = true;
  hits = [];
}

function insert(key: string, property: Property) {
  let caret = config.selectionStart;
  let spot = wordAt(config.value, caret);
  if (!spot) return;

  let quoted = config.value.slice(0, spot.start).trimEnd().endsWith('"');
  let start = quoted ? spot.start - 1 : spot.start;
  let value = JSON.stringify(property.default ?? null);
  let text = property.type === "object" ? `"${key}": {}` : `"${key}": ${value}`;

  config.value =
    config.value.slice(0, start) + text + config.value.slice(caret);
  let end = start + text.length - (property.type === "object" ? 1 : 0);
  config.selectionStart = config.selectionEnd = end;
  hide();
  run();
  config.focus();
}

/** One character of the editor's monospace font, measured once per mount. */
let cell = 0;

function measure(): number {
  if (cell) return cell;
  let probe = document.createElement("span");
  probe.textContent = "0".repeat(40);
  probe.style.cssText = "position:absolute;visibility:hidden;white-space:pre";
  probe.style.font = getComputedStyle(config).font;
  config.parentElement!.append(probe);
  cell = probe.getBoundingClientRect().width / 40;
  probe.remove();
  return cell;
}

/** Put the list under the caret, inside the pane. */
function place(caret: number) {
  let before = config.value.slice(0, caret);
  let line = before.split("\n").length - 1;
  let column = caret - before.lastIndexOf("\n") - 1;
  let style = getComputedStyle(config);
  let padX = parseFloat(style.paddingLeft);
  let padY = parseFloat(style.paddingTop);
  let height = parseFloat(style.lineHeight);

  let x = padX + column * measure() - config.scrollLeft;
  let y = padY + (line + 1) * height - config.scrollTop + 4;
  let room = config.clientWidth - list.offsetWidth - 8;

  list.style.left = `${Math.max(8, Math.min(x, room))}px`;
  list.style.top = `${y}px`;
}

function draw() {
  list.innerHTML = hits
    .map(({ key, property }, index) => {
      let shown =
        property.default === undefined
          ? ""
          : `<span class="def">${escape(JSON.stringify(property.default))}</span>`;
      let about = property.description
        ? `<small>${escape(property.description)}</small>`
        : "";
      return `<li role="option" aria-selected="${index === picked}"><b>${escape(key)}</b>${shown}${about}</li>`;
    })
    .join("");
  list.hidden = hits.length === 0;
}

function complete() {
  // An empty pane shows its prompt instead of a list of everything.
  if (!schema || !config.value.trim()) return hide();

  let caret = config.selectionStart;
  let spot = wordAt(config.value, caret);
  if (!spot) return hide();

  let section = scopeAt(config.value, caret);
  let properties = section
    ? (schema.properties[section]?.properties ?? {})
    : schema.properties;

  let word = spot.word.toLowerCase();
  // Every option in scope, nearest match first; the list scrolls rather than truncating.
  hits = Object.entries(properties)
    .filter(([key]) => key.toLowerCase().includes(word))
    .sort(([a], [b]) => {
      let lead =
        Number(b.toLowerCase().startsWith(word)) -
        Number(a.toLowerCase().startsWith(word));
      return lead || a.localeCompare(b);
    })
    .map(([key, property]) => ({ key, property }));

  picked = 0;
  draw();
  if (hits.length) place(caret);
}

function keys(event: KeyboardEvent) {
  if (list.hidden) return;
  if (event.key === "Escape") return hide();
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    picked =
      (picked + (event.key === "ArrowDown" ? 1 : hits.length - 1)) %
      hits.length;
    return draw();
  }
  if (event.key === "Enter" || event.key === "Tab") {
    let hit = hits[picked];
    if (!hit) return;
    event.preventDefault();
    insert(hit.key, hit.property);
  }
}

/** Wires one `<tidy-playground>`; the element calls this when it connects. */
export function mount(root: HTMLElement) {
  input = root.querySelector<HTMLTextAreaElement>("[data-input]")!;
  config = root.querySelector<HTMLTextAreaElement>("[data-config]")!;
  output = root.querySelector<HTMLPreElement>("[data-output]")!;
  parser = root.querySelector<HTMLSelectElement>("[data-parser]")!;
  state = root.querySelector<HTMLElement>("[data-state]")!;
  inStat = root.querySelector<HTMLElement>("[data-in-stat]")!;
  outStat = root.querySelector<HTMLElement>("[data-out-stat]")!;
  tabs = root.querySelector<HTMLElement>(".seg")!;
  panes = {
    input: root.querySelector<HTMLElement>("#input-pane")!,
    config: root.querySelector<HTMLElement>("#config")!,
  };
  inShade = root.querySelector<HTMLElement>("[data-input-shade]")!;
  cfgShade = root.querySelector<HTMLElement>("[data-config-shade]")!;
  list = root.querySelector<HTMLElement>("[data-complete]")!;

  list.addEventListener("mousedown", (event) => {
    let item = (event.target as HTMLElement).closest("li");
    if (!item) return;
    event.preventDefault();
    let hit = hits[[...list.children].indexOf(item)];
    if (hit) insert(hit.key, hit.property);
  });

  schema ??= undefined;
  void fetch("/tidy.schema.json")
    .then((response) => response.json())
    .then((loaded: Schema) => {
      schema = loaded;
    })
    .catch(() => {
      /* completion is a bonus; the pane still works without it */
    });

  parser.addEventListener("change", () => {
    sample();
    run();
  });

  for (let [field, shade] of [
    [input, inShade],
    [config, cfgShade],
  ] as const) {
    field.addEventListener("input", run);
    field.addEventListener("scroll", () => {
      shade.parentElement!.scrollTop = field.scrollTop;
      shade.parentElement!.scrollLeft = field.scrollLeft;
    });
  }

  config.addEventListener("input", () => complete());
  config.addEventListener("click", () => hide());
  config.addEventListener("blur", () => setTimeout(hide, 120));
  config.addEventListener("keydown", keys);

  tabs.addEventListener("click", (event) => {
    let tab = (event.target as HTMLElement).closest<HTMLElement>("[data-tab]");
    if (tab) show(tab.dataset.tab!);
  });

  root
    .querySelector("[data-share]")
    ?.addEventListener("click", async (event) => {
      let button = event.currentTarget as HTMLElement;
      history.replaceState(null, "", `#${share()}`);
      try {
        await navigator.clipboard.writeText(location.href);
        button.textContent = "Copied";
        setTimeout(() => (button.textContent = "Copy link"), 1400);
      } catch {
        /* the URL is in the address bar either way */
      }
    });

  input.addEventListener("keydown", (event) => {
    if (event.key !== "Tab" || event.shiftKey) return;
    event.preventDefault();
    let { selectionStart: start, selectionEnd: end, value } = input;
    input.value = `${value.slice(0, start)}\t${value.slice(end)}`;
    input.selectionStart = input.selectionEnd = start + 1;
    run();
  });

  if (restore()) {
    if (config.value.trim()) show("config");
  } else {
    sample();
  }

  if (ready) {
    state.textContent = `tidy v${version()}`;
    state.classList.add("ok");
    run();
    return;
  }

  loading ??= init({ module_or_path: root.dataset.wasm! });
  loading
    .then(() => {
      ready = true;
      state.textContent = `tidy v${version()}`;
      state.classList.add("ok");
      run();
    })
    .catch((error: unknown) => {
      state.textContent = "tidy failed to load";
      state.classList.add("failed");
      output.textContent = String(error);
    });
}
