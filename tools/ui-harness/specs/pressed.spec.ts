// A held control that the pointer leaves looks at rest again, like a native button: the
// pressed look follows the pointer, and releasing outside does nothing.
import type { Locator, Page } from '@playwright/test';
import { calls, expect, open, test } from './fixtures';

const WIN = '?platform=windows';

async function look(target: Locator): Promise<string> {
  return target.evaluate((node) => {
    const style = getComputedStyle(node);
    return `${style.backgroundColor} ${style.color} ${style.transform}`;
  });
}

async function pressAndLeave(page: Page, target: Locator): Promise<{ rest: string; left: string }> {
  const box = (await target.boundingBox())!;
  // At rest, with the pointer away.
  await page.mouse.move(box.x + box.width + 200, box.y + box.height + 200);
  await page.waitForTimeout(250);
  const rest = await look(target);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width + 200, box.y + box.height + 200, { steps: 4 });
  await page.waitForTimeout(250);
  const left = await look(target);
  await page.mouse.up();
  return { rest, left };
}

test('a held button that the pointer leaves looks at rest and does not fire', async ({ page }) => {
  await open(page, WIN);
  const fetch = page.getByTestId('fetch');
  const { rest, left } = await pressAndLeave(page, fetch);
  expect(left).toBe(rest);
  expect(await calls(page, 'start_run')).toHaveLength(0);
});

test('a held sidebar entry that the pointer leaves looks at rest', async ({ page }) => {
  await open(page, WIN);
  const entry = page.getByTestId('nav-settings');
  const { rest, left } = await pressAndLeave(page, entry);
  expect(left).toBe(rest);
  await expect(page.getByTestId('nav-jobs')).toHaveAttribute('aria-current', 'page');
});
