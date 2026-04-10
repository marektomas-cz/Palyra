import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";

const require = createRequire(import.meta.url);
const tsgolintPath = require.resolve("oxlint-tsgolint/bin/tsgolint.js");

const targets = process.argv.slice(2);
if (targets.length === 0) {
  console.error("Usage: node scripts/run-vp-lint.mjs <path> [<path>...]");
  process.exit(1);
}

const env = {
  ...process.env,
  OXLINT_TSGOLINT_PATH: tsgolintPath,
};

const result =
  process.platform === "win32"
    ? spawnSync("cmd.exe", ["/d", "/s", "/c", `vp lint ${targets.join(" ")}`], {
        stdio: "inherit",
        env,
      })
    : spawnSync("vp", ["lint", ...targets], {
        stdio: "inherit",
        env,
      });

if (result.error) {
  throw result.error;
}

process.exit(result.status ?? 1);
