// Wave 2, the jobs group: the run card's time, the search between places, the choice of
// rows in one column and in two (see docs/wave2/PLAN.md).

import type { Page } from '@playwright/test';
import { calls, expect, NOW, open, runFinished, test } from './fixtures';

const WIN = '?platform=windows';
const DAY = 86_400_000;
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');

test('the run card names the day of its fetch once midnight has passed', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('fetch').click();
  await runFinished(page);
  const time = page.getByTestId('run-finished').locator('.time');
  await expect(time).toHaveText('09:30');
  // The next day: the shared clock moves on when the window comes back to the front.
  await page.clock.setFixedTime(new Date(NOW.getTime() + DAY));
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect(time).toHaveText('24.09. 09:30');
});

test('a place chosen in the sidebar opens without the search; the link keeps it', async ({
  page,
}) => {
  await open(page, WIN);
  const search = page.getByTestId('search');
  const facet = page.getByTestId('facet');
  const whole = await facet.innerText();
  const lastSearch = async (): Promise<unknown> => {
    const [, args] = (await calls(page, 'list_jobs')).at(-1) ?? [];
    return (args as { query: { search: unknown } } | undefined)?.query.search;
  };
  await search.fill('Kreditoren');
  // The deliberate way: "Auch im Archiv" takes the search along.
  await page.getByTestId('also-archive').click();
  await expect(page.getByTestId('nav-archive')).toHaveAttribute('aria-current', 'page');
  await expect(search).toHaveValue('Kreditoren');
  await expect(rows(page)).toHaveCount(1);
  // Jobs in the sidebar: the inbox as a whole, like a folder of a mail app.
  await page.getByTestId('nav-jobs').click();
  await expect(search).toHaveValue('');
  await expect.poll(() => facet.innerText()).toBe(whole);
  // Another view and back keeps the place and its search; another place drops it.
  await search.fill('Kreditoren');
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('nav-jobs').click();
  await expect(search).toHaveValue('Kreditoren');
  await page.getByTestId('nav-archive').click();
  await expect(search).toHaveValue('');
  await expect.poll(lastSearch).toBeNull();
});

test('one column: a single chosen row keeps the selection bar; two columns open it', async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 700 });
  await open(page, WIN);
  const bar = page.getByTestId('selection-bar');
  const chosen = rows(page).and(page.locator('[aria-current="true"]'));
  await rows(page).nth(0).click();
  await page.getByTestId('back').click();
  await rows(page)
    .nth(1)
    .click({ modifiers: ['Control'] });
  await expect(bar).toContainText('1 ausgewählt');
  await rows(page)
    .nth(2)
    .click({ modifiers: ['Control'] });
  await expect(bar).toContainText('2 ausgewählt');
  // One taken out again: the other one stays chosen, and the bar acts on it.
  await rows(page)
    .nth(2)
    .click({ modifiers: ['Control'] });
  await expect(bar).toContainText('1 ausgewählt');
  await expect(chosen).toHaveCount(1);
  await expect(page.getByTestId('selection-trash')).toBeVisible();
  // Two columns: that job opens like after a plain click.
  const key = await chosen.getAttribute('data-testid');
  await page.setViewportSize({ width: 1280, height: 700 });
  await expect(bar).toHaveCount(0);
  await expect(page.getByTestId('reader')).toBeVisible();
  await expect(rows(page).and(page.locator('[aria-current="true"]'))).toHaveAttribute(
    'data-testid',
    key ?? '',
  );
});
