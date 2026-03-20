import js from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginVue from "eslint-plugin-vue";

export default [
  { ignores: ["dist/", "src-tauri/"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/recommended"],
  {
    files: ["**/*.vue"],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
  },
  {
    rules: {
      "vue/multi-word-component-names": "off",
      // Formatting rules handled by Prettier — disable to avoid conflicts
      "vue/max-attributes-per-line": "off",
      "vue/singleline-html-element-content-newline": "off",
      "vue/html-self-closing": "off",
      "vue/attributes-order": "off",
      "vue/html-closing-bracket-newline": "off",
    },
  },
  {
    // Auto-generated Vite type declarations
    files: ["src/vite-env.d.ts"],
    rules: {
      "@typescript-eslint/no-empty-object-type": "off",
      "@typescript-eslint/no-explicit-any": "off",
    },
  },
  {
    // Browser globals
    languageOptions: {
      globals: {
        console: "readonly",
        HTMLElement: "readonly",
        setInterval: "readonly",
        clearInterval: "readonly",
      },
    },
  },
];
