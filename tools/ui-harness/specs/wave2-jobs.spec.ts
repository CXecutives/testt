// Wave 2, the jobs group: the run card's time, the search between places, the choice of
// rows in one column and in two (see docs/wave2/PLAN.md).

import type { Page } from '@playwright/test';
import { expect, NOW, open, runFinished, test } from './fixtures';

const WIN = '?platform=windows';
const DAY = 86_400_000;
const list = (page: Page) => page.getByTestId('job-list');
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
