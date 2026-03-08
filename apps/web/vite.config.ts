import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const isCi = Boolean(
  (globalThis as typeof globalThis & { process?: { env?: { CI?: string } } }).process?.env?.CI
);

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./vitest.setup.ts",
    testTimeout: isCi ? 20_000 : 5_000,
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"]
  }
});
