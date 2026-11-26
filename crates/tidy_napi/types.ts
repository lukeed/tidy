// Prints the index.d.ts of `tidyrs` or of `tidyrs-wasm`: the options from tidy.schema.json, the hook, and the calls.
//
//   bun types.ts <tidyrs|tidyrs-wasm>
import schema from "./tidy.schema.json" with { type: "json" };

let target = process.argv[2];
if (target !== "tidyrs" && target !== "tidyrs-wasm")
  throw new Error("usage: bun types.ts <tidyrs|tidyrs-wasm>");

type Property = {
  description?: string;
  default?: unknown;
  type?: "boolean" | "integer" | "string" | "array" | "object";
  enum?: string[];
  anyOf?: Property[];
  items?: Property;
  properties?: Record<string, Property>;
  title?: string;
};

/** The object types that have a `title`, which `Options` declares by that name. */
let named = new Map<string, Property>();

let langs = [
  "css",
  "scss",
  "less",
  "html",
  "vue",
  "mjml",
  "hbs",
  "tempura",
  "js",
  "jsx",
  "ts",
  "tsx",
  "json",
  "jsonc",
  "json5",
  "yaml",
  "toml",
  "md",
  "graphql",
  "xml",
  "dockerfile",
  "ini",
  "gitconfig",
];

function type(property: Property): string {
  if (property.anyOf) return property.anyOf.map(type).join(" | ");
  if (property.enum)
    return property.enum.map((it) => JSON.stringify(it)).join(" | ");
  switch (property.type) {
    case "boolean":
      return "boolean";
    case "integer":
      return "number";
    case "string":
      return "string";
    case "array": {
      let item = type(property.items!);
      return item.includes(" | ") ? `(${item})[]` : `${item}[]`;
    }
    case "object": {
      let entries = Object.entries(property.properties!);
      if (property.title) {
        named.set(property.title, property);
        return property.title;
      }
      if (entries.some(([, it]) => it.description))
        throw new Error(
          `a documented object needs a title: ${JSON.stringify(property)}`,
        );
      return `{ ${entries.map(([name, it]) => `${name}?: ${type(it)}`).join("; ")} }`;
    }
  }
  throw new Error(`no type for ${JSON.stringify(property)}`);
}

function doc(property: Property): string {
  let lines = property.description ? [property.description] : [];
  let empty = Array.isArray(property.default) && property.default.length === 0;
  if (property.default !== undefined && !empty)
    lines.push(`@default ${literal(property.default)}`);
  if (!lines.length) return "";
  return `/**\n${lines.map((line) => ` * ${line}`).join("\n")}\n */\n`;
}

/** `value` as JSON, with a space after each comma, as tidy prints it. */
function literal(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(literal).join(", ")}]`;
  return JSON.stringify(value);
}

/**
 * `text` at `depth` indents, where tidy prints it, so that a doc comment wraps
 * to its final width the first time.
 */
function indented(text: string, depth: number): string {
  let indent = "  ".repeat(depth);
  return text
    .split("\n")
    .map((line) => (line ? indent + line : line))
    .join("\n");
}

/**
 * The members of an object type, at no indent: a nested object's are one
 * deeper.
 */
function members(properties: [string, Property][]): string {
  let all = properties.map(([name, it]) => `${doc(it)}${name}?: ${type(it)};`);
  return all.join(all.some((it) => it.includes("\n")) ? "\n\n" : "\n");
}

let all = Object.entries(schema.properties as Record<string, Property>);
let sections = all.filter(([, it]) => it.properties);
let common = all.filter(([name, it]) => !it.properties && name !== "$schema");
let shared = new Map(common);

function section([name, it]: [string, Property]): string {
  let own = Object.entries(it.properties!).filter(
    ([key, property]) =>
      type(property) !== type(shared.get(key) ?? property) || !shared.has(key),
  );
  let replaced = own
    .filter(([key]) => shared.has(key))
    .map(([key]) => JSON.stringify(key));
  let base = replaced.length
    ? `Omit<Common, ${replaced.join(" | ")}>`
    : "Common";
  let head = indented(
    `${doc({ description: it.description })}${name}?: ${base}`,
    2,
  );
  let body = own.length
    ? ` & {\n${indented(members(own), 3)}\n${indented("}", 2)}`
    : "";
  return `${head}${body};`;
}

/** The types in `named`, including those that their members name. */
function hoisted(): string {
  let done = new Set<string>();
  let out: string[] = [];
  for (let next of named.keys()) {
    if (done.has(next)) continue;
    done.add(next);
    let it = named.get(next)!;
    let body = indented(members(Object.entries(it.properties!)), 1);
    out.push(
      indented(
        `${doc({ description: it.description })}export type ${next} = {\n${body}\n};`,
        1,
      ),
    );
  }
  return out.join("\n\n");
}

let calls = `
/** Format a file. \`options.embed\` receives each piece of code in another language first. */
export declare function format(source: string, options?: Options): string;

/** Whether a file is formatted already, with the same options as \`format\`. */
export declare function check(source: string, options?: Options): boolean;
`;

let wasm = `
/** The options in the text of a \`tidy.json\`, \`tidy.jsonc\` or \`tidy.toml\`, to pass to \`format\`. */
export declare function config(text: string): Options;

/** The version of tidy this build came from. */
export declare function version(): string;

/** What \`init\` loads the wasm from: a URL or a response to fetch, its bytes, or a compiled module. */
export type Source = string | URL | Request | Response | BufferSource | WebAssembly.Module;

/**
 * Loads the wasm, once, before the first call. Without \`module_or_path\`, it fetches \`tidy_bg.wasm\` next to this
 * module. In Node, the wasm is loaded already.
 */
export default function init(input?: { module_or_path?: Source | Promise<Source> }): Promise<unknown>;
`;

let output = `// Generated from tidy.schema.json by types.ts: run \`mise run types\`.

/** A language, by the name a code fence gives it, or by a file extension. */
export type Lang = ${langs.map((it) => JSON.stringify(it)).join(" | ")} | (string & {});

/**
 * What a call throws. An error in the syntax of the source also says where, as Prettier's errors do: \`loc.start\` has
 * its \`line\` and \`column\`, from 1. The column counts characters.
 */
export type TidyError = Error & { loc?: { start: { line: number; column: number } } };

/** The options of \`format\` and \`check\`: those of a \`tidy.json\`, and what to format. */
export type Options = Options.Common &
  Options.Languages & {
    /** The language. Default: by \`filepath\`. */
    lang?: Lang;
    /** The file's path, which decides the language and some details. */
    filepath?: string;
    /** Formats each piece of code in another language, before tidy does. Not called when \`embedded\` is off. */
    embed?: Embed.Hook;
  };

export namespace Options {
  /** The options every language takes, at the top level and in each language's own section. */
  export type Common = {
${indented(members(common), 2)}
  };

  /** The options of one language, over the common ones. */
  export type Languages = {
${sections.map(section).join("\n\n")}
  };

${hoisted()}
}

export namespace Embed {
  /** Returns the formatted \`request.code\`, or \`undefined\` to let tidy format it, or keep it as written if tidy can't. */
  export type Hook = (request: Request) => string | undefined;

  /**
   * What the code is:
   *
   * - \`block\`: a whole script, style sheet or document
   * - \`expression\`: an expression, as in \`{{ value }}\` or \`:value="..."\`
   * - \`event\`: the statements of a Vue event handler, as in \`@click="..."\`
   * - \`bindings\`: the parameters of a Vue slot, as in \`v-slot="{ item }"\`
   * - \`for\`: the left side of a \`v-for\`
   * - \`generic\`: the type parameters of \`<script setup generic="...">\`
   * - \`directive\`: an Angular directive's microsyntax, as in \`*ngFor="..."\`
   * - \`frontmatter\`: the front matter of a file
   */
  export type Kind = "block" | "expression" | "event" | "bindings" | "for" | "generic" | "directive" | "frontmatter";

  /** A piece of code in another language, for the hook to format. */
  export type Request = {
    /** The code to format. */
    code: string;
    /** The language, by the name \`lang\` takes: \`js\`, \`ts\`, \`css\`, \`scss\`, \`json\`, \`yaml\`, \`md\`, ... */
    lang: Lang;
    /** What the code is. */
    kind: Kind;
    /** The framework whose rules the code follows, where they differ from the language's own. */
    host?: "vue" | "ng";
    /** The code is an attribute's value, as in \`style="..."\` or \`@click="..."\`. */
    attribute: boolean;
    /** How many indents the code starts at. Indent the lines after the first by this much. */
    indent: number;
    /** The width of the text before the code on its first line, after the indents, as \`name="\` before a value. */
    prefix: number;
    /** The width of the text after the code on its last line, as the \`"\` that closes a value. */
    suffix: number;
    /** The line width. */
    width: number;
    /** JavaScript that is a module, not a classic script. */
    module: boolean;
  };
}
`;

process.stdout.write(output + calls + (target === "tidyrs-wasm" ? wasm : ""));
