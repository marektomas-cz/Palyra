import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const isCi = Boolean(
  (globalThis as typeof globalThis & { process?: { env?: { CI?: string } } }).process?.env?.CI
);

function resolveFsPath(relativePath: string): string {
  return decodeURIComponent(new URL(relativePath, import.meta.url).pathname).replace(
    /^\/([A-Za-z]:\/)/,
    "$1"
  );
}

const appRoot = resolveFsPath("./");
const sharedUiRoot = resolveFsPath("../shared-ui");

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    fs: {
      allow: [appRoot, sharedUiRoot]
    }
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./vitest.setup.ts",
    testTimeout: isCi ? 20_000 : 5_000,
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"]
  }
});
