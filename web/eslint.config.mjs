import eslint from "@eslint/js";
import tseslint from "typescript-eslint";
import solid from "eslint-plugin-solid/configs/recommended";
import eslintConfigPrettier from "eslint-config-prettier";
import globals from "globals";

/** Domain glossary — rejected synonyms that should never appear in code. */
const REJECTED_TERMS = [
  "SinkInput",
  "VirtualSink",
  "Connection",
  "Wire",
  "FaderLevel",
  "RouteVolume",
  "Snapshot",
  "Scene",
  "Profile",
  "Bus", // Use Mix
  "Destination", // Use Mix
];

export default tseslint.config(
  { ignores: ["dist/", "node_modules/"] },

  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  solid,

  {
    files: ["**/*.ts", "**/*.tsx"],
    languageOptions: {
      parser: tseslint.parser,
      globals: { ...globals.browser, ...globals.es2024 },
    },
    rules: {
      // --- TypeScript quality ---
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      "@typescript-eslint/consistent-type-imports": [
        "error",
        { prefer: "type-imports", fixStyle: "separate-type-imports" },
      ],

      // --- SolidJS reactivity ---
      "solid/reactivity": "warn",
      "solid/no-destructure": "warn",
      "solid/prefer-for": "warn",

      // --- ADR-005: Domain glossary enforcement ---
      // Ban rejected synonyms as identifiers (variables, functions, types, interfaces)
      "no-restricted-syntax": [
        "error",
        ...REJECTED_TERMS.map((term) => ({
          selector: `Identifier[name=/${term}/i]`,
          message: `"${term}" violates domain glossary (ADR-005). Use: Channel, Mix, Node, Route, App, CellVolume, or Preset.`,
        })),
        // Ban direct WebSocket in components (SOLID-D: components depend on signals, not transport)
        {
          selector: `CallExpression[callee.name="WebSocket"]`,
          message:
            "Don't use WebSocket directly in components — use a store (SOLID-D: depend on signals, not transport).",
        },
      ],

      // --- ADR-005: No console.log in production code ---
      "no-console": ["warn", { allow: ["warn", "error"] }],
    },
  },

  // Components: stricter rules
  {
    files: ["src/components/**/*.tsx"],
    rules: {
      // Components must not import from stores' internals — only the public hook
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["ws", "websocket", "socket.io*"],
              message: "Components must not import transport libraries — use stores.",
            },
          ],
        },
      ],
    },
  },

  eslintConfigPrettier,
);
