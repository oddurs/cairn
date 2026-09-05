// Copies the two things the site shares with the repository, so neither is
// written twice: the format specification, and the recorded demo.
//
// A site that restates the specification would eventually contradict it. A
// site that restates the demo would eventually flatter it.
import { mkdirSync, copyFileSync } from "node:fs";

mkdirSync("src/content", { recursive: true });
mkdirSync("public", { recursive: true });

copyFileSync("../spec/README.md", "src/content/format.md");
copyFileSync("../doc/demo.svg", "public/demo.svg");

console.log("synced: format specification, recorded demo");
