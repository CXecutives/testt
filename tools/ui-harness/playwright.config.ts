// UI harness: the harness build (Tauri replaced by stub.ts) served by `vite preview` with the
// production CSP of the app, tested in Chromium (WebView2 on Windows) and WebKit (WKWebView
// on macOS). `npm run harness`; refresh baselines with `npm run harness -- --update-snapshots`.

import { defineConfig, devices } from '@playwright/test';

const viewport = { width: 1360, height: 900 };

// One port per checkout, so parallel worktrees never test each other's build. HARNESS_PORT
// overrides it; vite.config.ts reads the same variable for `vite preview`.
function checkoutPort(): number {
  let hash = 0;
  for (const char of process.cwd()) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return 5200 + (hash % 700);
}
const port = Number(process.env.HARNESS_PORT) || checkoutPort();
process.env.HARNESS_PORT = String(port);
const origin = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: './specs',
  outputDir: '../../test-results',
  snapshotPathTemplate: '{testDir}/../baselines/{projectName}/{arg}{ext}',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: 0,
  reporter: [['list']],
  timeout: 30_000,
  expect: {
    toHaveScreenshot: {
      animations: 'disabled',
      caret: 'hide',
      maxDiffPixelRatio: 0.002,
      threshold: 0.2,
    },
  },
  use: {
    baseURL: origin,
    locale: 'de-DE',
    timezoneId: 'Europe/Berlin',
    colorScheme: 'light',
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'], viewport, deviceScaleFactor: 1 },
    },
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'], viewport, deviceScaleFactor: 1 },
    },
  ],
  webServer: {
    command: 'npx vite build ui --mode harness && npx vite preview ui --mode harness',
    cwd: '../..',
    url: origin,
    // Always build and serve fresh: a stale server would test an old build.
    reuseExistingServer: false,
    timeout: 120_000,
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
