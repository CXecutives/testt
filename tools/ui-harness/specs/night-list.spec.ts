// Neu, Alle, Favoriten: one white thumb slides to the chosen option like the sidebar's pill,
// and rests exactly on it.

import { expect, open, test } from './fixtures';

test('the filter thumb slides to the chosen option and rests exactly on it', async ({ page }) => {
  await open(page, '?platform=windows');
  const facet = page.getByTestId('facet');
  const thumb = facet.locator('.thumb');
  const all = facet.getByRole('radio', { name: /Alle/ });
  const x = (): Promise<number> =>
    thumb.evaluate((node) => new DOMMatrix(getComputedStyle(node).transform).m41);
  const start = await x();
  await all.click();
  // On its way: at least one frame between the old and the new place.
  const samples: number[] = [];
  for (let frame = 0; frame < 12; frame += 1) {
    samples.push(await x());
    await page.waitForTimeout(16);
  }
  await page.waitForTimeout(250);
  const end = await x();
  expect(end).not.toBe(start);
  expect(
    samples.some((value) => value > Math.min(start, end) + 1 && value < Math.max(start, end) - 1),
  ).toBe(true);
  // At rest it covers exactly the chosen option.
  const [box, option] = await Promise.all([thumb.boundingBox(), all.boundingBox()]);
  expect(Math.abs(box!.x - option!.x)).toBeLessThan(1);
  expect(Math.abs(box!.width - option!.width)).toBeLessThan(1);
});
