// Shared test fixture: every console error, page error and CSP violation fails the test.

import { test as base, expect, type Page } from '@playwright/test';

/**
 * Pages currently being captured. For a screenshot Playwright injects its own <style>
 * (caret, animations); WebKit reports that as a CSP violation of the page. Those, and only
 * those, are not the app's fault.
 */
const capturing = new WeakSet<Page>();

export const test = base.extend<{ problems: string[] }>({
  problems: [
    async ({ page }, use) => {
      const problems: string[] = [];
      page.on('console', (message) => {
        if (message.type() === 'error' && !capturing.has(page)) {
          problems.push(`console: ${message.text()}`);
        }
      });
      page.on('pageerror', (error) => problems.push(`page: ${error.message}`));
      await page.addInitScript(() => {
        document.addEventListener('securitypolicyviolation', (event) => {
          console.error(`CSP violation: ${event.violatedDirective} ${event.blockedURI}`);
        });
      });
      await use(problems);
      expect(problems, 'console errors, page errors or CSP violations').toEqual([]);
    },
    { auto: true },
  ],
});

export { expect };

/** Open a page of the harness build and wait until fonts and the first frame are ready. */
export async function open(page: Page, query = ''): Promise<void> {
  await page.goto(`/${query}`);
  await settle(page);
}

export async function settle(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  });
}

/**
 * Compare with the baseline and, if SHOTS_DIR is set, also save a copy for humans.
 * Console output during the capture is Playwright's own and is not counted.
 */
export async function expectShot(
  page: Page,
  name: string,
  options: { maxDiffPixelRatio?: number; timeout?: number } = {},
): Promise<void> {
  capturing.add(page);
  try {
    await expect(page).toHaveScreenshot(`${name}.png`, options);
    const dir = process.env.SHOTS_DIR;
    if (dir) {
      const engine = test.info().project.name;
      await page.screenshot({ path: `${dir}/${name}-${engine}.png`, animations: 'disabled' });
    }
    await settle(page);
  } finally {
    capturing.delete(page);
  }
}
