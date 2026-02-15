import eslint from "@eslint/js";
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
    ignores: ["dist/**", "node_modules/**", "eslint.config.mjs"]
  },
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...typeCheckedConfigs,
  {
    files: ["**/*.ts", "**/*.tsx"],
    rules: {
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/no-explicit-any": "error"
    }
  }
];
