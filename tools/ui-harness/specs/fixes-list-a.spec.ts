// Fixes of the final audit round for the job list, its header and the selection: paging,
// choosing several jobs, the keys, the header row, errors and the empty states.

import type { Page } from '@playwright/test';
import { calls, expect, open, settle, test } from './fixtures';

const WIN = '?platform=windows';
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
const row = (page: Page, key: string) => page.getByTestId('job-list').getByTestId(`job-row-${key}`);
const facet = (page: Page, name: string) =>
  page.getByTestId('facet').getByRole('radio', { name: new RegExp(name) });

/** Keys of the listed rows, in the order of the list. */
function listedKeys(page: Page): Promise<string[]> {
  return page
    .getByTestId('job-list')
    .locator('[data-key]')
    .evaluateAll((items) => items.map((item) => (item as HTMLElement).dataset.key ?? ''));
}

/** Scroll the list to its end until at least `count` rows are mounted. */
async function mountRows(page: Page, count: number): Promise<void> {
  const scroll = page.getByTestId('list-scroll');
  await expect
    .poll(
      async () => {
        await scroll.evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
        return page.getByTestId('job-list').locator('[data-key]').count();
      },
      { timeout: 20_000 },
    )
    .toBeGreaterThanOrEqual(count);
}

/** The sample jobs of the default scenario (stub.ts). */
const SAMPLE = [
  ['freelancermap', '2801'],
  ['linkedin', '4100200301'],
  ['freelance', '900411'],
  ['freelancermap', '2803'],
  ['linkedin', '4100200302'],
  ['freelance', '900412'],
  ['linkedin', '4100200303'],
  ['linkedin', '4100200306'],
  ['linkedin', '4100200304'],
  ['freelancermap', '2805'],
  ['freelancermap', '2802'],
  ['freelancermap', '2804'],
  ['freelance', '900413'],
  ['linkedin', '4100200305'],
  ['freelancermap', '2806'],
] as const;

/** Every sample job is read before the page asks for the app state. */
async function allReadAtStart(page: Page): Promise<void> {
  await page.addInitScript((keys) => {
    let harness: Window['__harness'] | undefined;
    Object.defineProperty(window, '__harness', {
      configurable: true,
      get: () => harness,
      set: (value: Window['__harness']) => {
        harness = value;
        // After the stub has built its jobs (the rest of its module), before app_state.
        queueMicrotask(() => {
          for (const [portal, id] of keys) {
            const job = value.job({ portal, id });
            if (job?.unread)
              value.emit({ type: 'jobUpdated', job: { ...job, unread: false }, fresh: false });
          }
        });
      },
    });
  }, SAMPLE);
}

test.describe('paging and the remembered tab', () => {
  test('jobs read under Neu keep their row, and the next page skips no job', async ({ page }) => {
    await open(page, `${WIN}&scenario=many`);
    // Read three jobs: Neu keeps their rows (like Mail), the backend no longer lists them.
    for (const index of [0, 1, 2]) await rows(page).nth(index).click();
    await mountRows(page, 240);
    const seen = await listedKeys(page);
    const offsets = (await calls(page, 'list_jobs'))
      .map(([, args]) => (args as { query: { offset: number; limit: number } }).query)
      .filter((query) => query.offset > 0 && query.limit > 0)
      .map((query) => query.offset);
    expect(offsets[0]).toBe(117);
    // Neu entered again: the read jobs leave it, and every job it lists now was listed before.
    await facet(page, 'Alle').click();
    await facet(page, 'Neu').click();
    await mountRows(page, 237);
    const again = (await listedKeys(page)).slice(0, 237);
    expect(again.filter((key) => !seen.includes(key))).toEqual([]);
  });

  test('a start on Alle keeps Alle as the tab Jobs comes back to', async ({ page }) => {
    await allReadAtStart(page);
    await open(page, WIN);
    await expect(facet(page, 'Alle')).toHaveAttribute('aria-checked', 'true');
    await page.getByTestId('nav-archive').click();
    await page.getByTestId('nav-jobs').click();
    await settle(page);
    await expect(facet(page, 'Alle')).toHaveAttribute('aria-checked', 'true');
    await expect(rows(page).first()).toBeVisible();
    await expect(row(page, 'freelancermap-2801')).toBeVisible();
  });
});
