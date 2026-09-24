// ESLint for the UI (ui/src) and the harness (tools/ui-harness).
//
// The UI rules make the design system's promises impossible to break by accident:
// no inline styles, raw controls only inside components/, each outside dependency behind
// exactly one door (Icon.svelte, api.ts, lib/motion/), no literal colours, pixels or
// durations, input listeners only in input.ts, no `title` attribute, no empty catch.

import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';
import svelteConfig from './ui/svelte.config.js';

const UI = 'ui/src/**/*.{ts,svelte,svelte.ts}';

/* ---------------------------------------------------------------- imports */

const IMPORT_DOORS = {
  lucide: {
    group: ['@lucide/*', '@lucide/**', 'lucide', 'lucide-*'],
    message: 'Icons only through components/Icon.svelte.',
  },
  tauri: {
    group: ['@tauri-apps/api', '@tauri-apps/api/*', '@tauri-apps/plugin-*'],
    message: 'Tauri only through lib/ipc/api.ts.',
  },
  motion: {
    group: ['svelte/transition', 'svelte/animate', 'svelte/motion', 'svelte/easing'],
    message: 'Motion only through lib/motion/ (token-bound, reduced-motion aware).',
  },
};

const restrictedImports = (...open) => [
  'error',
  {
    patterns: Object.entries(IMPORT_DOORS)
      .filter(([name]) => !open.includes(name))
      .map(([, pattern]) => pattern),
  },
];

/* ----------------------------------------------------------------- syntax */

const POLICY_EVENTS =
  'key(down|up|press)|contextmenu|auxclick|dblclick|dragstart|selectstart|wheel|mousewheel|gesture(start|change|end)';

const SYNTAX = {
  hex: [
    {
      selector: 'Literal[value=/#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\\b/]',
      message: 'No literal colours - use a token from tokens.css.',
    },
    {
      selector:
        'TemplateElement[value.raw=/#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\\b/]',
      message: 'No literal colours - use a token from tokens.css.',
    },
  ],
  units: [
    {
      selector: 'Literal[value=/\\b\\d+(\\.\\d+)?(px|ms|rem|em)\\b/]',
      message: 'No literal px/ms values - read tokens (lib/tokens.ts, lib/motion/motion.ts).',
    },
    {
      selector: 'TemplateElement[value.raw=/\\b\\d+(\\.\\d+)?(px|ms|rem|em)\\b/]',
      message: 'No literal px/ms values - read tokens (lib/tokens.ts, lib/motion/motion.ts).',
    },
  ],
  inlineStyle: [
    {
      selector: "MemberExpression[property.name='cssText']",
      message: 'No inline styles (CSP style-src self) - use the cssVars action.',
    },
    {
      selector:
        "CallExpression[callee.property.name='setAttribute'][arguments.0.value=/^(style|title)$/]",
      message: 'No style/title attributes - use cssVars and the tooltip action.',
    },
    {
      selector: "AssignmentExpression > MemberExpression.left[object.property.name='style']",
      message: 'No inline styles - use the cssVars action.',
    },
  ],
  setProperty: [
    {
      selector: 'CallExpression[callee.property.name=/^(setProperty|removeProperty)$/]',
      message: 'Custom properties only through lib/actions/cssVars.ts.',
    },
  ],
  animate: [
    {
      selector: "CallExpression[callee.property.name='animate']",
      message: 'WAAPI only through lib/motion/motion.ts (play).',
    },
  ],
  listeners: [
    {
      selector: `CallExpression[callee.property.name='addEventListener'][arguments.0.value=/^(${POLICY_EVENTS})$/]`,
      message:
        'Key, context-menu, aux-button, wheel and gesture handling only in lib/input/input.ts.',
    },
    {
      selector: `AssignmentExpression > MemberExpression.left[property.name=/^on(${POLICY_EVENTS})$/]`,
      message:
        'Key, context-menu, aux-button, wheel and gesture handling only in lib/input/input.ts.',
    },
    {
      selector: `SvelteAttribute[key.name=/^on(${POLICY_EVENTS})$/]`,
      message:
        'Key, context-menu, aux-button, wheel and gesture handling only in lib/input/input.ts.',
    },
    {
      selector: `SvelteDirective[kind='EventHandler'][key.name.name=/^(${POLICY_EVENTS})$/]`,
      message:
        'Key, context-menu, aux-button, wheel and gesture handling only in lib/input/input.ts.',
    },
  ],
  title: [
    {
      selector: "SvelteAttribute[key.name='title'], SvelteShorthandAttribute[key.name='title']",
      message: 'No native title tooltips - use the tooltip action.',
    },
    {
      selector: "AssignmentExpression > MemberExpression.left[property.name='title']",
      message: 'No native title tooltips - use the tooltip action.',
    },
  ],
  emptyCatch: [
    {
      selector: 'CatchClause[body.body.length=0]',
      message: 'No empty catch - handle the error or report it (reportUiError).',
    },
  ],
};

const restrictedSyntax = (...open) => [
  'error',
  ...Object.entries(SYNTAX)
    .filter(([name]) => !open.includes(name))
    .flatMap(([, entries]) => entries),
];

/* ----------------------------------------------------------------- config */

export default ts.config(
  {
    ignores: [
      'ui/dist/**',
      'node_modules/**',
      'target/**',
      'test-results/**',
      'playwright-report/**',
      'src-tauri/**',
      'core/**',
    ],
  },
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs.recommended,
  {
    languageOptions: {
      globals: { ...globals.browser },
    },
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts'],
    languageOptions: {
      parserOptions: {
        projectService: false,
        extraFileExtensions: ['.svelte'],
        parser: ts.parser,
        svelteConfig,
      },
    },
  },
  {
    files: ['*.js', 'ui/*.{js,ts}', 'tools/**/*.{js,mjs,ts}'],
    languageOptions: { globals: { ...globals.node } },
  },
  {
    files: [UI],
    rules: {
      'no-empty': ['error', { allowEmptyCatch: false }],
      'no-restricted-imports': restrictedImports(),
      'no-restricted-syntax': restrictedSyntax(),
      // Svelte 5 transitions run on the Web Animations API, which the CSP allows; style=
      // and style: stay forbidden.
      'svelte/no-inline-styles': ['error', { allowTransitions: true }],
      'svelte/no-restricted-html-elements': [
        'error',
        {
          elements: ['button', 'input', 'textarea', 'select', 'a', 'svg', 'img', 'dialog'],
          message: 'Raw controls only inside components/ - use the design system.',
        },
      ],
      'svelte/no-at-html-tags': 'error',
      'svelte/button-has-type': 'error',
      'svelte/no-target-blank': 'error',
      '@typescript-eslint/consistent-type-imports': 'error',
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
    },
  },
  // The doors: each file may open exactly its own.
  {
    files: ['ui/src/components/**/*.svelte'],
    rules: { 'svelte/no-restricted-html-elements': 'off' },
  },
  {
    files: ['ui/src/components/Icon.svelte'],
    rules: { 'no-restricted-imports': restrictedImports('lucide') },
  },
  {
    files: ['ui/src/lib/ipc/api.ts'],
    rules: { 'no-restricted-imports': restrictedImports('tauri') },
  },
  {
    files: ['ui/src/lib/motion/**/*.ts'],
    rules: {
      'no-restricted-imports': restrictedImports('motion'),
      'no-restricted-syntax': restrictedSyntax('animate'),
    },
  },
  {
    files: ['ui/src/lib/input/input.ts'],
    rules: { 'no-restricted-syntax': restrictedSyntax('listeners') },
  },
  {
    files: ['ui/src/lib/actions/cssVars.ts'],
    rules: { 'no-restricted-syntax': restrictedSyntax('setProperty') },
  },
);
