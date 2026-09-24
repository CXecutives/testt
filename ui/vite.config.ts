// Build configuration of the UI. Vite root is `ui/`; the release build lands in `ui/dist`.
//
// Modes:
//   development  `npm run dev` - dev server for `tauri dev`, gallery available via `?gallery`.
//   production   `npm run build` - what the app ships; the gallery is compiled out.
//   harness      `vite build --mode harness` - Playwright harness: `@tauri-apps/api` is replaced by a
//                typed stub, the gallery is included, output goes to node_modules/.ui-harness.
//
// `vite preview` always sends the production Content-Security-Policy of the app (read from
// src-tauri/tauri.conf.json), so the harness sees exactly the policy the WebView enforces.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

const here = (path: string): string => fileURLToPath(new URL(path, import.meta.url));

interface TauriConfig {
  app: { security: { csp: string | Record<string, string> } };
}

/** The `app.security.csp` of the Tauri config as one header value. */
export function productionCsp(): string {
  const config = JSON.parse(
    readFileSync(here('../src-tauri/tauri.conf.json'), 'utf8'),
  ) as TauriConfig;
  const csp = config.app.security.csp;
  if (typeof csp === 'string') return csp;
  return Object.entries(csp)
    .map(([directive, sources]) => `${directive} ${sources}`)
    .join('; ');
}

export default defineConfig(({ mode }) => {
  const harness = mode === 'harness';
  return {
    root: here('.'),
    base: '/',
    publicDir: false,
    plugins: [svelte()],
    define: {
      // Dead-code eliminated in the release build: the gallery chunk is never emitted there.
      __GALLERY__: JSON.stringify(mode !== 'production'),
    },
    resolve: {
      alias: [
        { find: '$lib', replacement: here('src/lib') },
        { find: '$components', replacement: here('src/components') },
        ...(harness
          ? [
              {
                find: /^@tauri-apps\/api(\/.*)?$/,
                replacement: here('../tools/ui-harness/stub.ts'),
              },
            ]
          : []),
      ],
    },
    build: {
      outDir: harness ? here('../node_modules/.ui-harness/dist') : here('dist'),
      emptyOutDir: true,
      target: ['safari17', 'chrome120'],
      cssTarget: 'safari17',
      modulePreload: { polyfill: false },
      // Fonts and images stay files: `font-src 'self'` does not allow data: URLs.
      assetsInlineLimit: 0,
      reportCompressedSize: false,
    },
    server: { port: 5173, strictPort: true },
    preview: {
      host: '127.0.0.1',
      port: Number(process.env.HARNESS_PORT) || 5177,
      strictPort: true,
      headers: { 'Content-Security-Policy': productionCsp() },
    },
  };
});
