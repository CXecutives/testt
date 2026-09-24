// Stylelint for ui/src (.css and the <style> of .svelte files).
//
// Makes hard-coded values impossible: every colour, length, duration, radius, shadow and
// layer comes from a token in ui/src/styles/tokens.css - the only exception file.
// Documented exceptions:
//   - tokens.css: the value source itself.
//   - motion.css: the only place with @keyframes.
//   - components/Disclosure.svelte: may transition grid-template-rows (0fr -> 1fr).
//   - `@media (width < 900px)`: px in width media features (custom properties do not work there).

const ANIMATABLE = [
  'opacity',
  'transform',
  'color',
  'background-color',
  'border-color',
  'stroke-dashoffset',
];
const KEYFRAMES = ['shimmer', 'spin', 'pulse', 'breathe', 'sweep', 'shake', 'draw'];

/** `transition` shorthand: every comma-separated item starts with an allowed property. */
const transitionShorthand = (properties) => {
  const prop = `(${properties.join('|')})`;
  return `/^(none|${prop}\\s[^,]+(,\\s*${prop}\\s[^,]+)*)$/`;
};
const transitionProperty = (properties) => {
  const prop = `(${properties.join('|')})`;
  return `/^(none|${prop}(\\s*,\\s*${prop})*)$/`;
};

const KEYWORDS = [
  '0',
  'auto',
  'none',
  'inherit',
  'initial',
  'unset',
  'transparent',
  'currentcolor',
  'currentColor',
  'normal',
  'solid',
  'dashed',
  'inset',
  '100%',
  '50%',
  '1fr',
  'fit-content',
  'max-content',
  'min-content',
];

const STRICT = [
  '/^color$/',
  '/^background/',
  '/^fill$/',
  '/^stroke$/',
  '/^border/',
  '/^font/',
  '/^line-height$/',
  '/^letter-spacing$/',
  '/^margin/',
  '/^padding/',
  '/^gap$/',
  '/^row-gap$/',
  '/^column-gap$/',
  '/^inset/',
  '/^box-shadow$/',
  '/^z-index$/',
  '/^transition/',
  '/^animation/',
  '/^outline/',
];

const strictValues = [
  STRICT,
  {
    ignoreVariables: true,
    ignoreFunctions: true,
    ignoreValues: {
      '': KEYWORDS,
      '/^font/': [...KEYWORDS, 'tabular-nums', 'optimizelegibility', 'italic', 'uppercase'],
      '/^background/': [
        ...KEYWORDS,
        'text',
        'no-repeat',
        'content-box',
        'padding-box',
        'border-box',
        'center',
      ],
      '/^transition/': [
        ...KEYWORDS,
        ...ANIMATABLE,
        'grid-template-rows',
        '/^var\\(--[a-z0-9-]+\\),?$/',
      ],
      '/^animation/': [
        ...KEYWORDS,
        ...KEYFRAMES,
        'linear',
        'infinite',
        'both',
        'forwards',
        'alternate',
        'running',
        'paused',
        '1',
        '/^var\\(--[a-z0-9-]+\\),?$/',
      ],
      '/^border/': [...KEYWORDS, '/^var\\(--[a-z0-9-]+\\),?$/'],
      '/^box-shadow$/': [...KEYWORDS, '/^var\\(--[a-z0-9-]+\\),?$/'],
    },
    disableFix: true,
    message: 'Use a token (var(--...)) for "${property}", not "${value}".',
  },
];

/** @type {import('stylelint').Config} */
export default {
  plugins: ['stylelint-declaration-strict-value'],
  rules: {
    'color-no-hex': true,
    'color-named': 'never',
    'color-no-invalid-hex': true,
    'function-disallowed-list': [
      'rgb',
      'rgba',
      'hsl',
      'hsla',
      'hwb',
      'lab',
      'lch',
      'oklab',
      'oklch',
      'color',
      'color-mix',
      'light-dark',
    ],
    'unit-disallowed-list': [
      ['px', 'rem', 'em', 'ms', 's'],
      { ignoreMediaFeatureNames: { px: ['width', 'min-width', 'max-width'] } },
    ],
    'declaration-no-important': true,
    'at-rule-disallowed-list': ['keyframes', 'starting-style', 'view-transition', 'import'],
    'media-feature-name-disallowed-list': ['prefers-color-scheme', 'prefers-reduced-motion'],
    'property-disallowed-list': [
      'scrollbar-gutter',
      'scrollbar-width',
      'scrollbar-color',
      'content-visibility',
      'view-transition-name',
      'view-transition-class',
    ],
    'selector-disallowed-list': ['/::view-transition/'],
    'declaration-property-value-disallowed-list': {
      '/.*/': ['/var\\(--p-/'],
    },
    'declaration-property-value-allowed-list': {
      transition: [transitionShorthand(ANIMATABLE)],
      'transition-property': [transitionProperty(ANIMATABLE)],
    },
    'scale-unlimited/declaration-strict-value': strictValues,
  },
  overrides: [
    { files: ['**/*.svelte'], customSyntax: 'postcss-html' },
    {
      files: ['ui/src/styles/tokens.css'],
      rules: {
        'color-named': null,
        'function-disallowed-list': null,
        'unit-disallowed-list': null,
        'declaration-property-value-disallowed-list': null,
        'scale-unlimited/declaration-strict-value': null,
      },
    },
    {
      files: ['ui/src/styles/motion.css'],
      rules: {
        'at-rule-disallowed-list': ['starting-style', 'view-transition', 'import'],
      },
    },
    {
      files: ['ui/src/styles/base.css'],
      rules: {
        // @font-face descriptors are not declarations of the page.
        'scale-unlimited/declaration-strict-value': [
          strictValues[0],
          { ...strictValues[1], ignoreAtRules: ['@font-face'] },
        ],
      },
    },
    {
      files: ['ui/src/components/Disclosure.svelte'],
      rules: {
        'declaration-property-value-allowed-list': {
          transition: [transitionShorthand([...ANIMATABLE, 'grid-template-rows'])],
          'transition-property': [transitionProperty([...ANIMATABLE, 'grid-template-rows'])],
        },
      },
    },
  ],
};
