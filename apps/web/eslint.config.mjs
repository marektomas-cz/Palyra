import eslint from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";

const typeCheckedConfigs = tseslint.configs.recommendedTypeChecked.map((config) => ({
  ...config,
  files: ["**/*.ts", "**/*.tsx"],
  languageOptions: {
    ...(config.languageOptions ?? {}),
    parserOptions: {
      ...(config.languageOptions?.parserOptions ?? {}),
      projectService: true
    }
  }
}));

export default [
  {
    ignores: ["dist/**", "node_modules/**", "coverage/**", "eslint.config.mjs"]
  },
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...typeCheckedConfigs,
  {
    files: ["**/*.ts", "**/*.tsx"],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.es2021
      }
    },
    rules: {
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/no-explicit-any": "error"
    }
  },
  {
    files: ["**/*.test.ts", "**/*.test.tsx", "**/__tests__/**/*.{ts,tsx}", "vitest.setup.ts"],
    languageOptions: {
      globals: {
        ...globals.node,
        ...globals.vitest
      }
    }
  }
];
