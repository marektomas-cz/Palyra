#!/usr/bin/env node

import { rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const generatedPathsByCommand = {
  "clean-all": [
    "apps/web/dist",
    "apps/web/coverage",
    "apps/web/.vite",
    "apps/desktop/ui/dist",
    "apps/desktop/ui/.vite",
  ],
  "clean-desktop-ui": ["apps/desktop/ui/dist", "apps/desktop/ui/.vite"],
  "clean-web": ["apps/web/dist", "apps/web/coverage", "apps/web/.vite"],
};

const command = process.argv[2];
const generatedPaths = generatedPathsByCommand[command];

if (!generatedPaths) {
  console.error("Usage: node ./scripts/js-artifacts.mjs <clean-all|clean-desktop-ui|clean-web>");
  process.exit(1);
}

for (const relativePath of generatedPaths) {
  rmSync(join(repoRoot, relativePath), { force: true, recursive: true });
}

console.log(`Removed generated JS outputs for ${command}.`);
