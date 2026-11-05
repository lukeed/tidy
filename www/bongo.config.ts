import { markdown } from "@bongo/markdown";
import { twinkleplop } from "@bongo/markdown/twinkleplop";

import * as code from "./code";
import * as utils from "./utils";

import type { Config } from "bongo";

export default {
  $site: {
    lang: "en",
    title: "tidy",
    url: "https://tidy.lukeed.com/",
    description:
      "A code formatter written in Rust that prints what Prettier prints, for every language it supports.",
  },
  $utils: { ...utils },
  $hooks: {
    transform: code.transform,
  },
  plugins: [
    markdown({
      headingIds: true,
      highlighter: twinkleplop({ languages: code.languages }),
      fence: code.fence,
    }),
  ],
} satisfies Config;
