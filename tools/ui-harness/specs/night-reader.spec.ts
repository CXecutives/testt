// The reader's details that must say themselves: every state mark names its state under the
// pointer (a half circle alone was not understood).

import { expect, open, test } from './fixtures';

test('every reason mark says its state in words under the pointer', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.locator('[data-testid^="job-row-"]').first().click();
  const marks = page.getByTestId('stage').locator('.reason .icon[aria-label]');
  await expect(marks.first()).toBeVisible();
  const count = Math.min(await marks.count(), 4);
  for (let index = 0; index < count; index += 1) {
    const mark = marks.nth(index);
    const word = (await mark.getAttribute('aria-label'))!;
    await mark.scrollIntoViewIfNeeded();
    await mark.hover();
    await expect(page.getByRole('tooltip')).toHaveText(word);
    await page.mouse.move(0, 0);
  }
});
