// Prints the types that napi puts before the bindings in index.d.ts: the options from tidy.schema.json, and the hook.
import schema from "./tidy.schema.json" with { type: "json" };

type Property = {
  description?: string;
  default?: unknown;
  type?: "boolean" | "integer" | "string" | "array" | "object";
  enum?: string[];
  anyOf?: Property[];
  items?: Property;
  properties?: Record<string, Property>;
};

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
    case "array":
      return `${type(property.items!)}[]`;
  }
  throw new Error(`no type for ${JSON.stringify(property)}`);
}

function doc(property: Property): string {
  let lines = [property.description ?? ""];
  let empty = Array.isArray(property.default) && property.default.length === 0;
  if (property.default !== undefined && !empty)
    lines.push(`@default ${JSON.stringify(property.default)}`);
  return `/**\n${lines.map((line) => ` * ${line}`).join("\n")}\n */`;
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

function members(properties: [string, Property][], depth: number): string {
  return properties
    .map(([name, it]) => indented(`${doc(it)}\n${name}?: ${type(it)};`, depth))
    .join("\n\n");
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
    `${doc({ description: it.description })}\n${name}?: ${base}`,
    2,
  );
  let body = own.length ? ` & {\n${members(own, 3)}\n${indented("}", 2)}` : "";
  return `${head}${body};`;
}

let output = `// Generated from tidy.schema.json by types.ts: run \`mise run types\`.

/** A language, by the name a code fence gives it, or by a file extension. */
export type Lang = ${langs.map((it) => JSON.stringify(it)).join(" | ")} | (string & {});

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
${members(common, 2)}
  };

  /** The options of one language, over the common ones. */
  export type Languages = {
${sections.map(section).join("\n\n")}
  };
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

process.stdout.write(output);
