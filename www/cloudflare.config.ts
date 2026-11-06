import { bindings, defineConfig } from "cf/config";

export default defineConfig({
  worker: {
    name: "tidy-docs",
    compatibilityDate: "2026-10-13",
    entrypoint: "./worker.ts",
    assets: {
      htmlHandling: "auto-trailing-slash",
      notFoundHandling: "404-page",
      // The Worker runs only on these: crowne.io's two paths, the install script,
      // and `/`, where it chooses between the script and the home page by user agent.
      runWorkerFirst: ["/x/script.js", "/x", "/install.sh", "/"],
    },
    domains: ["tidy.lukeed.com"],
    env: {
      // The crowne.io site id. The key is a secret: wrangler secret put CROWNE_KEY --name tidy-docs
      CROWNE_SITE: bindings.text("01M6P92KE67F1WGBTFQY7EHERA"),
      CROWNE_KEY: bindings.secret(),
      ASSETS: bindings.assets(),
    },
  },
});
