// Wave 2, backend group: an undo puts a job back as it was (see docs/wave2/PLAN.md).

import type { Page } from '@playwright/test';
import { calls, expect, NOW, open, test } from './fixtures';

const WIN = '?platform=windows';
const row = (page: Page, key: string) => page.getByTestId('job-list').getByTestId(`job-row-${key}`);
const DAY_MS = 86_400_000;

/** Moves the page's clock and lets its "now" follow at once (it ticks on focus). */
async function later(page: Page, days: number): Promise<void> {
  await page.clock.setFixedTime(new Date(NOW.getTime() + days * DAY_MS));
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
}

test('undoing Wiederherstellen keeps the trash date and its days', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const key = 'freelancermap-2802';
  await row(page, key).hover();
  await page.getByTestId(`trash-${key}`).click();
  await expect(row(page, key)).toHaveCount(0);
  // Three days later in the Papierkorb: 27 of the 30 days are left.
  await later(page, 3);
  await page.getByTestId('nav-trash').click();
  await row(page, key).click();
  const line = page.getByTestId('place-line');
  await expect(line).toHaveText('Im Papierkorb, wird in 27 Tagen gelöscht');
  await page.getByTestId('reader-restore').click();
  await expect(row(page, key)).toHaveCount(0);
  const toast = page.getByTestId('toast').filter({ hasText: 'wiederhergestellt' });
  await toast.getByTestId('toast-action').click();
  await expect(row(page, key)).toHaveCount(1);
  // The undo sends the first trash date along, and a new list shows it too.
  expect((await calls(page, 'move_back')).map(([, args]) => args)).toEqual([
    {
      jobs: [
        {
          key: { portal: 'freelancermap', id: '2802' },
          to: 'trash',
          trashedAt: NOW.toISOString(),
        },
      ],
    },
  ]);
  await page.getByTestId('nav-jobs').click();
  await page.getByTestId('nav-trash').click();
  await row(page, key).click();
  await expect(line).toHaveText('Im Papierkorb, wird in 27 Tagen gelöscht');
});
