// Generated from tidy.schema.json by types.ts: run `mise run types`.

/** A language, by the name a code fence gives it, or by a file extension. */
export type Lang =
  | "css"
  | "scss"
  | "less"
  | "html"
  | "vue"
  | "mjml"
  | "hbs"
  | "tempura"
  | "js"
  | "jsx"
  | "ts"
  | "tsx"
  | "json"
  | "jsonc"
  | "json5"
  | "yaml"
  | "toml"
  | "md"
  | "graphql"
  | "xml"
  | "dockerfile"
  | "ini"
  | "gitconfig"
  | (string & {});

/**
 * The options of `format` and `check`: those of a `tidy.json`, and what to
 * format.
 */
export type Options = Options.Common &
  Options.Languages & {
    /** The language. Default: by `filepath`. */
    lang?: Lang;
    /** The file's path, which decides the language and some details. */
    filepath?: string;
    /**
     * Formats each piece of code in another language, before tidy does. Not
     * called when `embedded` is off.
     */
    embed?: Embed.Hook;
  };

export namespace Options {
  /**
   * The options every language takes, at the top level and in each language's
   * own section.
   */
  export type Common = {
    /**
     * Keep the `>` of a multi-line tag on the last line.
     *
     * @default false
     */
    bracketSameLine?: boolean;

    /**
     * Spaces inside the braces of JSON and JS objects, and YAML flow mappings.
     *
     * @default true
     */
    bracketSpacing?: boolean;

    /**
     * Format the code of other languages inside a file.
     *
     * @default true
     */
    embedded?: boolean;

    /**
     * The line ending to write.
     *
     * @default "lf"
     */
    eol?: "lf" | "crlf" | "cr";

    /**
     * The name of the ignore directive.
     *
     * @default "prettier-ignore"
     */
    ignore?: string;

    /**
     * The width of one indent.
     *
     * @default 2
     */
    indent?: number;

    /**
     * Quotes in Handlebars, style sheets, JSON5, YAML and JS.
     *
     * @default "double"
     */
    quote?: "double" | "single";

    /**
     * Put each attribute on its own line.
     *
     * @default false
     */
    singleAttributePerLine?: boolean;

    /**
     * Indent with tabs.
     *
     * @default false
     */
    tabs?: boolean;

    /**
     * Trailing commas in SCSS, JSONC, JSON5, YAML flow collections, TOML arrays
     * and JS.
     *
     * @default true
     */
    trailingComma?: boolean;

    /**
     * Whitespace sensitivity in HTML, Vue, Handlebars and XML. `strict` for
     * XML.
     *
     * @default "auto"
     */
    whitespace?: "strict" | "auto" | "ignore";

    /**
     * The line width the printer fits to. 120 for Dockerfiles.
     *
     * @default 80
     */
    width?: number;
  };

  /** The options of one language, over the common ones. */
  export type Languages = {
    /** Options for CSS, SCSS and Less only. */
    css?: Common;

    /** Options for Dockerfiles only. */
    dockerfile?: Common & {
      /**
       * Put a HEALTHCHECK's CMD on its own line.
       *
       * @default false
       */
      healthcheckCmdNewLine?: boolean;

      /**
       * Indent the instructions of each stage.
       *
       * @default false
       */
      indentStages?: boolean;
    };

    /** Options for GraphQL only. */
    graphql?: Common;

    /** Options for Handlebars only. */
    handlebars?: Common & {
      /**
       * Which mustache grammar to parse.
       *
       * @default "auto"
       */
      dialect?: "auto" | "handlebars" | "tempura";
    };

    /** Options for HTML only. */
    html?: Common;

    /** Options for INI only. */
    ini?: Common & {
      /**
       * Spaces around the `=` of an entry.
       *
       * @default true
       */
      spaceAroundEquals?: boolean;
    };

    /** Options for JavaScript and TypeScript only. */
    js?: Omit<Common, "trailingComma"> & {
      /**
       * Parentheses around a lone arrow function parameter.
       *
       * @default "always"
       */
      arrowParens?: "always" | "avoid";

      /**
       * Which side of a line break a binary operator sits on.
       *
       * @default "end"
       */
      experimentalOperatorPosition?: "start" | "end";

      /**
       * Prettier's experimental ternary layout.
       *
       * @default false
       */
      experimentalTernaries?: boolean;

      /**
       * Format JSDoc comments, as `prettier-plugin-jsdoc` does.
       *
       * @default true
       */
      jsdoc?: boolean;

      /**
       * Single quotes in JSX attributes.
       *
       * @default false
       */
      jsxSingleQuote?: boolean;

      /**
       * Keep an object expanded when the source had a line break after `{`.
       *
       * @default "preserve"
       */
      objectWrap?: "preserve" | "collapse";

      /**
       * Import sorters to follow: `@trivago/prettier-plugin-sort-imports`,
       * `@ianvs/prettier-plugin-sort-imports` or
       * `prettier-plugin-organize-imports`.
       */
      plugins?: string[];

      /**
       * When to quote the keys of an object.
       *
       * @default "as-needed"
       */
      quoteProps?: "as-needed" | "consistent" | "preserve";

      /**
       * End statements with a semicolon.
       *
       * @default true
       */
      semi?: boolean;

      /**
       * Sort imports, with oxfmt's sorter.
       *
       * @default false
       */
      sortImports?: boolean;

      /**
       * Where to leave a trailing comma.
       *
       * @default "all"
       */
      trailingComma?: "all" | "es5" | "none" | boolean;
    };

    /** Options for JSON only. */
    json?: Common & {
      /**
       * Keep an object expanded when the source had a line break after `{`.
       *
       * @default "preserve"
       */
      objectWrap?: "preserve" | "collapse";

      /**
       * When to quote the keys of an object.
       *
       * @default "as-needed"
       */
      quoteProps?: "as-needed" | "consistent" | "preserve";
    };

    /** Options for Markdown only. */
    markdown?: Common & {
      /**
       * The marker of a bullet list item.
       *
       * @default "dash"
       */
      bullet?: "keep" | "dash" | "asterisk" | "plus";

      /**
       * The delimiter of emphasis.
       *
       * @default "underscore"
       */
      emphasis?: "keep" | "underscore" | "asterisk";

      /**
       * The fence of a code block.
       *
       * @default "backtick"
       */
      fence?: "keep" | "backtick" | "tilde";

      /**
       * Turn setext headings into `#` prefixes.
       *
       * @default "keep"
       */
      heading?: "keep" | "atx";

      /**
       * The numbers of ordered list items.
       *
       * @default "auto"
       */
      numbering?: "keep" | "auto";

      /**
       * A starting set of the marker options below.
       *
       * @default "prettier"
       */
      preset?: "prettier" | "preserve";

      /**
       * The characters of a thematic break.
       *
       * @default "dash"
       */
      rule?: "keep" | "dash" | "asterisk" | "underscore";

      /**
       * The delimiter of strong emphasis.
       *
       * @default "asterisk"
       */
      strong?: "keep" | "asterisk" | "underscore";

      /**
       * How to wrap prose.
       *
       * @default "preserve"
       */
      wrap?: "preserve" | "always" | "never";
    };

    /** Options for TOML only. */
    toml?: Common & {
      /**
       * Align the comments after consecutive entries.
       *
       * @default true
       */
      alignComments?: boolean;

      /**
       * Align the values of consecutive entries.
       *
       * @default false
       */
      alignEntries?: boolean;

      /**
       * Align a comment that stands alone.
       *
       * @default true
       */
      alignSingleComments?: boolean;

      /**
       * How many blank lines to keep in a row.
       *
       * @default 2
       */
      allowedBlankLines?: number;

      /**
       * Collapse an array that fits the line width.
       *
       * @default true
       */
      arrayAutoCollapse?: boolean;

      /**
       * Expand an array that does not fit the line width.
       *
       * @default true
       */
      arrayAutoExpand?: boolean;

      /**
       * No spaces inside the brackets of an array.
       *
       * @default true
       */
      compactArrays?: boolean;

      /**
       * No spaces around the `=` of an entry.
       *
       * @default false
       */
      compactEntries?: boolean;

      /**
       * No spaces inside the braces of an inline table.
       *
       * @default false
       */
      compactInlineTables?: boolean;

      /**
       * Indent entries under their table header.
       *
       * @default false
       */
      indentEntries?: boolean;

      /**
       * Indent the entries of a table.
       *
       * @default false
       */
      indentTables?: boolean;

      /**
       * Expand an inline table onto several lines.
       *
       * @default true
       */
      inlineTableExpand?: boolean;

      /**
       * Sort the values of each array.
       *
       * @default false
       */
      reorderArrays?: boolean;

      /**
       * Sort the keys of each inline table.
       *
       * @default false
       */
      reorderInlineTables?: boolean;

      /**
       * Sort the keys of each table.
       *
       * @default false
       */
      reorderKeys?: boolean;

      /**
       * End the file with a line break.
       *
       * @default true
       */
      trailingNewline?: boolean;
    };

    /** Options for Vue only. */
    vue?: Common & {
      /**
       * Indent the contents of `<script>` and `<style>`.
       *
       * @default false
       */
      indentBlocks?: boolean;
    };

    /** Options for XML only. */
    xml?: Common & {
      /**
       * Which quotes to write around an attribute value.
       *
       * @default "preserve"
       */
      attributeQuotes?: "preserve" | "single" | "double";

      /**
       * A space before the `/>` of a self-closing tag.
       *
       * @default true
       */
      selfClosingSpace?: boolean;

      /**
       * Sort attributes by name.
       *
       * @default false
       */
      sortAttributes?: boolean;
    };

    /** Options for YAML only. */
    yaml?: Common & {
      /**
       * How to wrap prose.
       *
       * @default "preserve"
       */
      wrap?: "preserve" | "always" | "never";
    };
  };
}

export namespace Embed {
  /**
   * Returns the formatted `request.code`, or `undefined` to let tidy format it,
   * or keep it as written if tidy can't.
   */
  export type Hook = (request: Request) => string | undefined;

  /**
   * What the code is:
   *
   * - `block`: a whole script, style sheet or document
   * - `expression`: an expression, as in `{{ value }}` or `:value="..."`
   * - `event`: the statements of a Vue event handler, as in `@click="..."`
   * - `bindings`: the parameters of a Vue slot, as in `v-slot="{ item }"`
   * - `for`: the left side of a `v-for`
   * - `generic`: the type parameters of `<script setup generic="...">`
   * - `directive`: an Angular directive's microsyntax, as in `*ngFor="..."`
   * - `frontmatter`: the front matter of a file
   */
  export type Kind =
    | "block"
    | "expression"
    | "event"
    | "bindings"
    | "for"
    | "generic"
    | "directive"
    | "frontmatter";

  /** A piece of code in another language, for the hook to format. */
  export type Request = {
    /** The code to format. */
    code: string;
    /**
     * The language, by the name `lang` takes: `js`, `ts`, `css`, `scss`,
     * `json`, `yaml`, `md`, ...
     */
    lang: Lang;
    /** What the code is. */
    kind: Kind;
    /**
     * The framework whose rules the code follows, where they differ from the
     * language's own.
     */
    host?: "vue" | "ng";
    /** The code is an attribute's value, as in `style="..."` or `@click="..."`. */
    attribute: boolean;
    /**
     * How many indents the code starts at. Indent the lines after the first by
     * this much.
     */
    indent: number;
    /**
     * The width of the text before the code on its first line, after the
     * indents, as `name="` before a value.
     */
    prefix: number;
    /**
     * The width of the text after the code on its last line, as the `"` that
     * closes a value.
     */
    suffix: number;
    /** The line width. */
    width: number;
    /** JavaScript that is a module, not a classic script. */
    module: boolean;
  };
}
