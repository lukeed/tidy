import { existsSync } from "node:fs";
import { join } from "node:path";

import type { Template } from "bongo";
import type { Row } from "./data/bench";

export type Link = {
  title: string;
  path: string;
  /** `page` on the page itself, `true` on a page below it. */
  current?: "page" | "true";
};

export type Group = {
  title: string;
  pages: Link[];
};

/**
 * The sidebar groups, in order. The shape follows prettier.io's: what tidy is,
 * then how to run it, then how to configure it, then what it formats.
 */
const GROUPS = ["About", "Usage", "Configuration", "Languages", "API", "Misc"];

const REPO = "https://github.com/lukeed/tidy";

/**
 * Group the docs pages for the sidebar, in the `GROUPS` order, then by each
 * page's `order` frontmatter.
 *
 * @example
 *   $utils.nav($pages, "/docs/install/"); //=> [{ title: 'About', pages: [{ title: 'Introduction', path: '/docs/', current: undefined }] }]
 */
export function nav(pages: Template.PageInfo[], current: string): Group[] {
  return GROUPS.map((title) => ({
    title,
    pages: pages
      .filter((page) => page.params.group === title)
      .toSorted((a, b) => Number(a.params.order) - Number(b.params.order))
      .map((page) => ({
        title: page.title,
        path: page.path,
        current:
          page.path === current
            ? ("page" as const)
            : current.startsWith(page.path)
              ? ("true" as const)
              : undefined,
      })),
  })).filter((group) => group.pages.length > 0);
}

/**
 * The docs pages before and after the current one, in sidebar order.
 *
 * @example
 *   $utils.pager($pages, "/docs/cli/"); //=> { prev: { title: 'Install', … }, next: { title: 'Ignoring code', … } }
 */
export function pager(
  pages: Template.PageInfo[],
  current: string,
): { prev?: Link; next?: Link } {
  let links = nav(pages, current).flatMap((group) => group.pages);
  let index = links.findIndex((link) => link.path === current);
  if (index === -1) return {};
  return { prev: links[index - 1], next: links[index + 1] };
}

/**
 * Text with `code` spans as HTML: the text is escaped, and each backtick pair
 * becomes `<code>`.
 *
 * @example
 *   $utils.inline("reads `tidy.json`"); //=> 'reads <code>tidy.json</code>'
 */
export function inline(text: string): string {
  let escaped = text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
  return escaped.replace(/`([^`]+)`/g, "<code>$1</code>");
}

/**
 * Text without the backticks of its `code` spans, for places that show no
 * markup, such as meta tags.
 *
 * @example
 *   $utils.plain("reads `tidy.json`"); //=> 'reads tidy.json'
 */
export function plain(text: string): string {
  return text.replace(/`([^`]+)`/g, "$1");
}

/**
 * The source file of a docs page, relative to `www/`: a `.hbs` file, else a
 * `.md` file.
 *
 * @example
 *   $utils.file("/docs/cli/"); //=> 'content/docs/cli.md'
 */
export function file(path: string): string {
  let name = "content/" + path.replace(/^\/|\/$/g, "");
  return existsSync(join(import.meta.dirname, name + ".hbs"))
    ? name + ".hbs"
    : name + ".md";
}

/**
 * The GitHub URL that edits the source of a docs page.
 *
 * @example
 *   $utils.source("/docs/cli/"); //=> 'https://github.com/lukeed/tidy/edit/main/www/content/docs/cli.md'
 */
export function source(path: string): string {
  return `${REPO}/edit/main/www/${file(path)}`;
}

/**
 * Whether this is a production build. `bongo dev` runs no Worker, so it has no
 * crowne.io paths.
 *
 * @example
 *   {{#if $utils.production()}}<script defer src="/x/script.js" data-endpoint="/x"></script>{{/if}}
 */
export function production(): boolean {
  return process.env.NODE_ENV === "production";
}

/**
 * A millisecond count as the page writes it: `78ms`, `7.2ms`, `18.5s`.
 *
 * @example
 *   $utils.ms(18548); //=> '18.5s'
 */
export function ms(value: number): string {
  return value < 1000 ? `${value}ms` : `${(value / 1000).toFixed(1)}s`;
}

export type Bar = {
  who: string;
  ms: number;
  me?: boolean;
  width: number;
};

export type Bars = {
  tools: Bar[];
  /** Prettier's time, which is two orders of magnitude off the same scale. */
  prettier: number | null;
  /** How many times slower that is than tidy. */
  slower: number | null;
};

/**
 * One benchmark row as the home page draws it. Prettier is two orders of
 * magnitude away, so one linear scale would make every native tool an
 * identical sliver: they are scaled against each other, and Prettier is
 * written out as a multiple underneath.
 *
 * @example
 *   $utils.bars($data.bench.featured[0]);
 */
export function bars(row: Row): Bars {
  let native = [
    { who: "tidy", ms: row.tidy, me: true },
    { who: "bun", ms: row.bun },
    { who: "oxfmt", ms: row.oxfmt },
    { who: "dprint", ms: row.dprint },
  ].filter((bar) => bar.ms != null) as Omit<Bar, "width">[];

  let top = Math.max(...native.map((bar) => bar.ms));

  return {
    tools: native.map((bar) => ({ ...bar, width: (bar.ms / top) * 100 })),
    prettier: row.prettier,
    slower: row.prettier == null ? null : Math.round(row.prettier / row.tidy),
  };
}

/** A number with thousands separators: `8528` becomes `8,528`. */
export function num(value: number): string {
  return value.toLocaleString("en-US");
}
