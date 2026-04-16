import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { existsSync, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, relative } from "node:path";

const require = createRequire(import.meta.url);
const scriptDir = dirname(fileURLToPath(import.meta.url));

function resolveTsgolintPath() {
  const tsgolintPath = require.resolve("oxlint-tsgolint/bin/tsgolint");
  if (process.platform !== "win32") {
    return tsgolintPath;
  }

  const localBinDir = join(scriptDir, "..", "node_modules", ".bin");
  const projectBinDir = join(dirname(dirname(tsgolintPath)), "..", ".bin");
  const pathCandidates = [
    join(localBinDir, "tsgolint.exe"),
    join(localBinDir, "tsgolint.cmd"),
    join(projectBinDir, "tsgolint.exe"),
    join(projectBinDir, "tsgolint.cmd"),
  ];

  let executablePath = pathCandidates.find((path) => existsSync(path)) ?? "";
  if (!executablePath) {
    try {
      const realBinDir = join(dirname(realpathSync(join(scriptDir, ".."))), ".bin");
      executablePath =
        [join(realBinDir, "tsgolint.exe"), join(realBinDir, "tsgolint.cmd")].find((path) =>
          existsSync(path),
        ) ?? "";
    } catch {
      // Fall through to the explicit error below.
    }
  }

  if (!executablePath) {
    throw new Error(
      `Unable to resolve oxlint-tsgolint executable, tried:\n${pathCandidates
        .map((path) => `- ${path}`)
        .join("\n")}`,
    );
  }

  const relativePath = relative(process.cwd(), executablePath);
  return /^[a-zA-Z]:/.test(relativePath) ? relativePath : `.\\${relativePath}`;
}

const tsgolintPath = resolveTsgolintPath();

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
