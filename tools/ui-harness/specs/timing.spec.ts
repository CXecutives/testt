// Timing checks: they run in Chromium in the `timing` project, after every other test and
// alone (a browser beside them on the same CPU would lengthen their tasks), on a defined
// machine (`asReferenceMachine`). WebKit runs them in its own project for what they check
// besides the timing (it has no Long Tasks API).

import type { Page } from '@playwright/test';
import { asReferenceMachine, expect, open, settle, test } from './fixtures';

const WIN = '?platform=windows';
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');

test('2000 jobs render in windows without long tasks', async ({ page, browserName }) => {
  // Long tasks and the start of the measurement on one clock: an event's time stamp is the
  // timeline of the entries themselves (performance.now() is Playwright's clock here).
  await page.addInitScript(() => {
    (window as unknown as { __long: number[][] }).__long = [];
    if (PerformanceObserver.supportedEntryTypes?.includes('longtask')) {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          (window as unknown as { __long: number[][] }).__long.push([
            entry.startTime,
            entry.duration,
          ]);
        }
      }).observe({ type: 'longtask', buffered: true });
    }
  });
  await open(page, `${WIN}&scenario=many`);
  await expect(rows(page).first()).toBeVisible();
  // Only the list work counts, not the start of the app: wait until the first window has
  // mounted completely and the main thread is idle, then measure the interactions.
  await expect
    .poll(async () => {
      const before = await rows(page).count();
      await settle(page);
      return before > 0 && before === (await rows(page).count());
    })
    .toBe(true);
  const idle = (): Promise<void> =>
    page.evaluate(
      () =>
        new Promise<void>((resolve) =>
          'requestIdleCallback' in window
            ? requestIdleCallback(() => resolve(), { timeout: 2000 })
            : setTimeout(resolve, 200),
        ),
    );
  await idle();
  // The same machine everywhere: as slow as the reference, locally and on CI.
  await asReferenceMachine(page);
  await idle();
  const since = await page.evaluate(() => new Event('start').timeStamp);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(page.getByTestId('facet').getByRole('radio', { name: /Alle/ })).toContainText(
    '2.000',
  );
  const count = await page.locator('[data-testid^="job-row-"]').count();
  expect(count).toBeLessThanOrEqual(70);
  for (let i = 0; i < 4; i += 1) {
    await page
      .getByTestId('list-scroll')
      .evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
    await page.waitForTimeout(150);
  }
  expect(await page.locator('[data-testid^="job-row-"]').count()).toBeGreaterThan(count);
  if (browserName === 'chromium') {
    // The last entries arrive a task after their task ended.
    await settle(page);
    const long = await page.evaluate(() => (window as unknown as { __long: number[][] }).__long);
    expect(
      long.filter(([start, d]) => start! > since && d! > 50),
      JSON.stringify({ since, long }),
    ).toEqual([]);
  }
});
