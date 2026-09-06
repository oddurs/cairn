// Copies the things the site shares with the repository, so none of them is
// written twice: the format specification, the promise, and the recorded demo.
//
// A site that restates the specification would eventually contradict it. A
// site that restates the demo would eventually flatter it. A site that restates
// the promise in its own words has already broken it.
import { mkdirSync, copyFileSync } from "node:fs";

mkdirSync("src/content", { recursive: true });
mkdirSync("public", { recursive: true });

copyFileSync("../spec/README.md", "src/content/format.md");
copyFileSync("../PROMISE.md", "src/content/promise.md");
copyFileSync("../doc/demo.svg", "public/demo.svg");
copyFileSync("../doc/samples.json", "src/content/samples.json");

console.log("synced: format specification, promise, recorded demo, terminal samples");
