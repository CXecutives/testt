import { expect, open, test } from './fixtures';

test('debug compact bar focus', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.getByTestId('job-rows').locator('[data-testid^="job-row-"]').first().click();
  const bar = page.getByTestId('reader-compact');
  const stage = page.getByTestId('stage');
  await stage.evaluate((node) => ((node as HTMLElement).style.scrollPaddingTop = '0px'));
  await stage.evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
  await expect(bar).toHaveCSS('opacity', '1');
  const before = await stage.evaluate((node) => node.scrollTop);
  // Keyboard: from the reader's last control backwards into the compact bar.
  await bar.getByTestId('compact-close').focus();
  await page.waitForTimeout(300);
  const afterFocus = await stage.evaluate((node) => node.scrollTop);
  await page.keyboard.press('Shift+Tab');
  await page.waitForTimeout(300);
  const afterTab = await stage.evaluate((node) => node.scrollTop);
  const opacity = await bar.evaluate((node) => getComputedStyle(node).opacity);
  console.log('SCROLL', before, afterFocus, afterTab, opacity);
  expect(true).toBe(true);
});
