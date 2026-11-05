import init, { format, version } from "./wasm/tidy.js";

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

let input = document.querySelector<HTMLTextAreaElement>("[data-input]")!;
let config = document.querySelector<HTMLTextAreaElement>("[data-config]")!;
let output = document.querySelector<HTMLPreElement>("[data-output]")!;
let parser = document.querySelector<HTMLSelectElement>("[data-parser]")!;
let state = document.querySelector<HTMLElement>("[data-state]")!;
let inStat = document.querySelector<HTMLElement>("[data-in-stat]")!;
let outStat = document.querySelector<HTMLElement>("[data-out-stat]")!;

let ready = false;

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

function run() {
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
    output.textContent = text;
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

parser.addEventListener("change", () => {
  sample();
  run();
});

for (let field of [input, config]) {
  field.addEventListener("input", run);
}

document
  .querySelector("[data-config-toggle]")
  ?.addEventListener("click", (event) => {
    let button = event.currentTarget as HTMLElement;
    let open = button.getAttribute("aria-expanded") === "true";
    button.setAttribute("aria-expanded", String(!open));
    document.getElementById("config")!.hidden = open;
  });

document
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

/**
 * Tab indents instead of leaving the editor, which is what a code box should
 * do.
 */
input.addEventListener("keydown", (event) => {
  if (event.key !== "Tab" || event.shiftKey) return;
  event.preventDefault();
  let { selectionStart: start, selectionEnd: end, value } = input;
  input.value = `${value.slice(0, start)}\t${value.slice(end)}`;
  input.selectionStart = input.selectionEnd = start + 1;
  run();
});

if (restore()) {
  if (config.value.trim()) {
    document
      .querySelector("[data-config-toggle]")!
      .setAttribute("aria-expanded", "true");
    document.getElementById("config")!.hidden = false;
  }
} else {
  sample();
}

let binary = document.querySelector<HTMLElement>(".play")!.dataset.wasm!;

init({ module_or_path: binary })
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
