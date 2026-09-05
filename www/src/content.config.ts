import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

// Documentation is Markdown in a collection rather than pages of JSX: writing a
// document should be writing prose, with a component available where one is
// genuinely needed.
const docs = defineCollection({
  loader: glob({ base: "./src/content/docs", pattern: "**/*.{md,mdx}" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    summary: z.string().optional(),
    order: z.number(),
  }),
});

export const collections = { docs };
