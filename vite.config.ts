import { defineConfig } from "vite-plus";

const jsIgnorePatterns = [
  "node_modules/**",
  "apps/**/node_modules/**",
  "apps/web/dist/**",
  "apps/web/coverage/**",
  "apps/web/.vite/**",
  "apps/desktop/ui/dist/**",
  "apps/desktop/ui/.vite/**",
  "apps/desktop/src-tauri/gen/**",
  "schemas/generated/**",
  "security-artifacts/**",
  "target/**"
];

export default defineConfig({
  fmt: {
    ignorePatterns: jsIgnorePatterns,
    semi: true,
    singleQuote: false
  },
  lint: {
    ignorePatterns: jsIgnorePatterns,
    options: {
      typeAware: true,
      typeCheck: true
    },
    plugins: ["typescript"],
    rules: {
      "typescript/consistent-type-imports": "error",
      "typescript/no-explicit-any": "error"
    },
    overrides: [
      {
        files: ["apps/web/**/*.{ts,tsx}", "apps/desktop/ui/**/*.{ts,tsx}"],
        env: {
          browser: true
        }
      },
      {
        files: [
          "apps/web/**/*.{test,spec}.{ts,tsx}",
          "apps/web/**/__tests__/**/*.{ts,tsx}",
          "apps/web/vitest.setup.ts"
        ],
        env: {
          browser: true,
          node: true,
          vitest: true
        }
      },
      {
        files: ["apps/browser-extension/**/*.{js,mjs,cjs}"],
        env: {
          node: true
        }
      },
      {
        files: ["scripts/**/*.{js,mjs,cjs}"],
        env: {
          node: true
        }
      }
    ]
  },
  run: {
    tasks: {
      "lint:web": {
        command: "node scripts/run-vp-lint.mjs apps/web"
      },
      "lint:desktop-ui": {
        command: "vp lint apps/desktop/ui"
      },
      "lint:browser-extension": {
        command: "vp lint apps/browser-extension"
      },
      "lint:tooling": {
        command: "vp lint scripts"
      }
    }
  }
});
