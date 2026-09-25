// Wave 2: the reader's terms, the search over every word, and the fixes of the ship audit
// (see docs/PLAN.md).

import type { Page } from '@playwright/test';
import { expect, open, test } from './fixtures';

const WIN = '?platform=windows';
const list = (page: Page) => page.getByTestId('job-list');
const row = (page: Page, key: string) => list(page).getByTestId(`job-row-${key}`);
const stage = (page: Page) => page.getByTestId('stage');

test('the reader shows the ad rate and start without a profile minimum', async ({ page }) => {
  for (const [lang, rate, start] of [
    ['', '1.200 €/Tag', 'ab sofort'],
    ['&lang=en', '€1,200/day', 'starts now'],
  ] as const) {
    await open(page, `${WIN}&scenario=no-minimum${lang}`);
    await row(page, 'freelancermap-2801').click();
    // The profile sets neither a minimum rate nor a start: the ad's values join the line.
    const clean = stage(page).getByTestId('criteria-clean');
    await expect(clean).toContainText(rate);
    await expect(clean).toContainText(start);
  }
  // With a minimum the criterion chip says the rate, and no fact repeats it.
  await open(page, WIN);
  await row(page, 'freelancermap-2801').click();
  const clean = stage(page).getByTestId('criteria-clean');
  await expect(clean).toContainText('1.200 €/Tag');
  const text = async () => (await clean.textContent())?.replace(/\s/g, ' ') ?? '';
  await expect.poll(async () => (await text()).split('1.200 €/Tag').length).toBe(2);
});
