import { bindings, defineConfig } from "cf/config";

export default defineConfig({
  worker: {
    name: "tidy-docs",
    compatibilityDate: "2026-10-13",
    entrypoint: "./worker.ts",
    assets: {
      htmlHandling: "auto-trailing-slash",
      notFoundHandling: "404-page",
      runWorkerFirst: ["/x/script.js", "/x", "/install.sh", "/"],
    },
    domains: ["tidy.lukeed.com"],
    env: {
      CROWNE_SITE: bindings.text("01M6P92KE67F1WGBTFQY7EHERA"),
      ASSETS: bindings.assets(),
    },
  },
});
