/**
 * Each language the extension formats, with the extension of the name an
 * untitled document gets, so that tidy can tell its language. tidy decides from
 * the real name of a saved file.
 */
export const EXTENSIONS: Record<string, string> = {
  css: "css",
  dockerfile: "dockerfile",
  graphql: "graphql",
  handlebars: "hbs",
  html: "html",
  ini: "ini",
  javascript: "js",
  javascriptreact: "jsx",
  json: "json",
  json5: "json5",
  jsonc: "jsonc",
  less: "less",
  markdown: "md",
  mjml: "mjml",
  properties: "properties",
  scss: "scss",
  svg: "svg",
  toml: "toml",
  typescript: "ts",
  typescriptreact: "tsx",
  vue: "vue",
  xml: "xml",
  xsl: "xsl",
  yaml: "yaml",
};
