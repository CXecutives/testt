// Wave 2, backend group: an undo and Wiederherstellen put a job back as it was (see
// docs/wave2/PLAN.md).

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

/** Waits out the moment after a move in which a click does nothing (actions.ts GUARD_MS). */
const settleMoves = (page: Page): Promise<void> => page.waitForTimeout(550);

/** A row's tool: it exists while the pointer is on the row. */
async function tool(page: Page, id: string, key: string): Promise<void> {
  await settleMoves(page);
  await row(page, key).hover();
  await page.getByTestId(`${id}-${key}`).click();
  await expect(row(page, key)).toHaveCount(0);
}

test('Wiederherstellen puts a job thrown away from the Archiv back there', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const key = 'linkedin-4100200301';
  await tool(page, 'archive', key);
  await page.getByTestId('nav-archive').click();
  await tool(page, 'trash', key);
  await page.getByTestId('nav-trash').click();
  await tool(page, 'restore', key);
  expect((await calls(page, 'restore_jobs')).map(([, args]) => args)).toEqual([
    { keys: [{ portal: 'linkedin', id: '4100200301' }] },
  ]);
  // The undo puts it into the trash again, a second Wiederherstellen back into the Archiv.
  const toast = page.getByTestId('toast').filter({ hasText: 'wiederhergestellt' });
  await toast.getByTestId('toast-action').click();
  await expect(row(page, key)).toHaveCount(1);
  expect((await calls(page, 'move_back')).at(-1)?.[1]).toEqual(
    expect.objectContaining({ jobs: [expect.objectContaining({ to: 'trash' })] }),
  );
  await tool(page, 'restore', key);
  await page.getByTestId('nav-archive').click();
  await expect(row(page, key)).toHaveCount(1);
  await page.getByTestId('nav-jobs').click();
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(
    page
      .getByTestId('job-list')
      .getByTestId(/^job-row-/)
      .first(),
  ).toBeVisible();
  await expect(row(page, key)).toHaveCount(0);
});
