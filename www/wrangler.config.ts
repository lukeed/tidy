import { defineWranglerConfig } from "wrangler/experimental-config";

export default defineWranglerConfig({
  rules: [
    {
      type: "Text",
      globs: ["**/*.sh"],
      fallthrough: true,
    },
  ],
  assetsDirectory: "./public",
});
