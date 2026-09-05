import { defineConfig } from "astro/config";

import mdx from "@astrojs/mdx";

// Built statically and served from GitHub Pages — no server, which is the same
// promise the tool itself makes.
export default defineConfig({
  site: "https://oddurs.github.io",
  base: "/cairn",
  trailingSlash: "ignore",
  build: { format: "directory" },
  integrations: [mdx()],
});