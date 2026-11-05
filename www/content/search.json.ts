import { readFileSync } from "node:fs";
import { join } from "node:path";

import { file, plain } from "../utils";

import type { Build } from "bongo";

/** How much of a page's text to index. Every docs page is well under this. */
const LIMIT = 16000;

/**
 * The page's prose as one searchable string: its Markdown source without the
 * frontmatter, the link syntax or the table rules. Code fences stay, because
 * people search for flag and option names, which only appear in them.
 */
function text(path: string): string {
  let source: string;
  try {
    source = readFileSync(join(import.meta.dirname, "..", file(path)), "utf8");
  } catch {
    return "";
  }
  return source
    .replace(/^---\n[\s\S]*?\n---\n/, "")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^[|\s:-]+$/gm, " ")
    .replace(/[`*#>|]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, LIMIT);
}

export default ({ $pages }: Build.Data) =>
  $pages
    .filter((page) => page.params.group)
    .map((page) => ({
      title: page.title,
      path: page.path,
      group: page.params.group,
      summary: plain(page.summary ?? ""),
      headings: page.headings
        .filter((heading) => heading.level === 2 || heading.level === 3)
        .map((heading) => [heading.id, heading.text]),
      text: text(page.path),
    }));
