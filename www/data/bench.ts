/**
 * Every tool checks every file of each corpus, using all cores, on an M1 Max.
 * Times are the mean of both run orders, in milliseconds, and embedded code is
 * excluded so that every tool does the same amount of work. `null` is a tool
 * that does not format the corpus at all.
 *
 * Source: `bench/results/format`, run by `bench/run.ts`.
 */
export type Row = {
  corpus: string;
  files: number;
  tidy: number;
  bun: number | null;
  oxfmt: number | null;
  dprint: number | null;
  prettier: number | null;
};

let rows: Row[] = [
  {
    corpus: "JavaScript",
    files: 3906,
    tidy: 78,
    bun: 100,
    oxfmt: 123,
    dprint: null,
    prettier: 8528,
  },
  {
    corpus: "TypeScript",
    files: 3309,
    tidy: 58,
    bun: 85,
    oxfmt: 96,
    dprint: null,
    prettier: 7523,
  },
  {
    corpus: "JSON",
    files: 4569,
    tidy: 86,
    bun: 149,
    oxfmt: 114,
    dprint: 323,
    prettier: 3528,
  },
  {
    corpus: "CSS",
    files: 300,
    tidy: 7.2,
    bun: 35,
    oxfmt: 47,
    dprint: 25,
    prettier: 654,
  },
  {
    corpus: "SCSS",
    files: 101,
    tidy: 4.6,
    bun: 32,
    oxfmt: 46,
    dprint: 11,
    prettier: 340,
  },
  {
    corpus: "Less",
    files: 396,
    tidy: 9.7,
    bun: 37,
    oxfmt: 48,
    dprint: 58,
    prettier: 1491,
  },
  {
    corpus: "HTML",
    files: 151,
    tidy: 6.4,
    bun: 34,
    oxfmt: 237,
    dprint: 14,
    prettier: 327,
  },
  {
    corpus: "Vue",
    files: 230,
    tidy: 7.3,
    bun: 34,
    oxfmt: 435,
    dprint: 205,
    prettier: 684,
  },
  {
    corpus: "Handlebars",
    files: 57,
    tidy: 3.7,
    bun: 30,
    oxfmt: 201,
    dprint: null,
    prettier: 279,
  },
  {
    corpus: "Markdown",
    files: 2834,
    tidy: 37,
    bun: 71,
    oxfmt: 80,
    dprint: 188,
    prettier: 6618,
  },
  {
    corpus: "YAML",
    files: 435,
    tidy: 24,
    bun: 35,
    oxfmt: 46,
    dprint: 104,
    prettier: 1271,
  },
  {
    corpus: "TOML",
    files: 375,
    tidy: 8.6,
    bun: 36,
    oxfmt: 49,
    dprint: 22,
    prettier: null,
  },
  {
    corpus: "GraphQL",
    files: 2511,
    tidy: 37,
    bun: 63,
    oxfmt: 72,
    dprint: 271,
    prettier: 1988,
  },
  {
    corpus: "XML",
    files: 6784,
    tidy: 130,
    bun: null,
    oxfmt: null,
    dprint: 616,
    prettier: 6017,
  },
  {
    corpus: "Dockerfiles",
    files: 25,
    tidy: 3.6,
    bun: null,
    oxfmt: null,
    dprint: 3.7,
    prettier: 458,
  },
  {
    corpus: "INI",
    files: 75,
    tidy: 3.7,
    bun: null,
    oxfmt: null,
    dprint: null,
    prettier: 147,
  },
];

let total: Row = {
  corpus: "All but XML",
  files: 12071,
  tidy: 280,
  bun: 569,
  oxfmt: 1232,
  dprint: 3474,
  prettier: 18548,
};

/** The corpora the home page draws bars for: the four with the most files. */
let featured = ["JavaScript", "TypeScript", "JSON", "Markdown"];

export default {
  rows,
  total,
  featured: rows.filter((row) => featured.includes(row.corpus)),
  /** How many times faster tidy is than Prettier over everything but XML. */
  speedup: Math.round(total.prettier! / total.tidy),
  /** With embedded code formatted too, over all but XML. */
  embedded: { tidy: 286, bun: 609, oxfmt: 2400, prettier: 30900 },
  machine: "M1 Max",
};
