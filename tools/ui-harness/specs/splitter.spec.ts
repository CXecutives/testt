// The handle between the list and the reader: the list keeps 320 px, the reader 440 px, and
// the list takes at most 60 % of the content; the limits and the first width (40 % of the
// content, at most 460 px) follow the window and the sidebar; a kept width that does not fit
// shows at the limit and comes back when there is room; a tooltip names what it does, over
// what a double click does.

import type { Page } from '@playwright/test';
import { expect, open, settle, test } from './fixtures';

const WIN = '?platform=windows';

interface Handle {
  width: number;
  min: number;
  max: number;
}

/** The list column's width as laid out, and the handle's live limits. */
async function handle(page: Page): Promise<Handle> {
  await settle(page);
  const splitter = page.getByTestId('list-splitter');
  const sheet = (await page.locator('main.views').boundingBox())!;
  const at = (await splitter.boundingBox())!;
  const value = async (name: string): Promise<number> => Number(await splitter.getAttribute(name));
  // The content starts after the sheet's hairline; the column ends where the handle lies.
  const width = Math.round(at.x - (sheet.x + 1));
  expect(width, 'laid out = aria-valuenow').toBe(await value('aria-valuenow'));
  return { width, min: await value('aria-valuemin'), max: await value('aria-valuemax') };
}

async function drag(page: Page, dx: number): Promise<void> {
  const box = (await page.getByTestId('list-splitter').locator('.hit').boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx / 2, y, { steps: 3 });
  await page.mouse.move(x + dx, y, { steps: 3 });
  await page.mouse.up();
}

for (const [width, expected] of [
  // content = window - sidebar (196) - hairline: 903, 1163, 1723
  [1100, { width: 361, min: 320, max: 463 }],
  [1360, { width: 460, min: 320, max: 698 }],
  [1920, { width: 460, min: 320, max: 1034 }],
] as const) {
  test(`the list's first width and limits at ${width} px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await open(page, WIN);
    expect(await handle(page)).toEqual(expected);
    await drag(page, 2000);
    await expect.poll(async () => (await handle(page)).width).toBe(expected.max);
    await drag(page, -2000);
    await expect.poll(async () => (await handle(page)).width).toBe(expected.min);
  });
}

test('the limits follow the window; a kept width waits for its room', async ({ page }) => {
  await open(page, WIN);
  await drag(page, 2000);
  await expect.poll(async () => (await handle(page)).width).toBe(698);
  // A wider window gives the list more room; dragged to its end there.
  await page.setViewportSize({ width: 1920, height: 800 });
  await expect.poll(async () => (await handle(page)).max).toBeGreaterThan(698);
  await drag(page, 2000);
  const wide = (await handle(page)).max;
  await expect.poll(async () => (await handle(page)).width).toBe(wide);
  // Narrower: shown at the limit, the choice stays and comes back.
  await page.setViewportSize({ width: 1100, height: 800 });
  await expect.poll(async () => (await handle(page)).max).toBe(463);
  expect((await handle(page)).width).toBe(463);
  await page.setViewportSize({ width: 1920, height: 800 });
  await expect.poll(async () => (await handle(page)).width).toBe(wide);
  await page.reload();
  expect((await handle(page)).width).toBe(wide);
});

test('a kept width that is too wide shows at the limit', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('jobs-list-width', '900'));
  await open(page, WIN);
  expect(await handle(page)).toEqual({ width: 698, min: 320, max: 698 });
  await page.setViewportSize({ width: 1920, height: 900 });
  await expect.poll(async () => (await handle(page)).width).toBe(900);
});

test('the handle says what it does; a double click sets the first width back', async ({ page }) => {
  await open(page, WIN);
  const hit = page.getByTestId('list-splitter').locator('.hit');
  const box = (await hit.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  // Only the grip shows (no line along the border), 44 px long.
  await expect(hit.locator('.line')).toHaveCount(0);
  expect((await hit.locator('.grip').boundingBox())!.height).toBe(44);
  await expect(hit.locator('.grip')).toHaveCSS('opacity', '1');
  const tip = page.getByRole('tooltip');
  await expect(tip).toContainText('Breite ändern');
  await expect(tip.locator('.hint')).toHaveText('Doppelklick setzt zurück');
  // Beside the grip in the middle of the handle.
  const grip = (await hit.locator('.grip').boundingBox())!;
  const bubble = (await tip.locator('div').boundingBox())!;
  expect(bubble.x).toBeGreaterThan(grip.x + grip.width);
  expect(Math.abs(bubble.y + bubble.height / 2 - (grip.y + grip.height / 2))).toBeLessThan(2);
  await expect(hit).toHaveCSS('cursor', 'col-resize');

  await drag(page, -100);
  await expect.poll(async () => (await handle(page)).width).toBe(360);
  await page.reload();
  expect((await handle(page)).width).toBe(360);
  await page.getByTestId('list-splitter').locator('.hit').dblclick();
  await expect.poll(async () => (await handle(page)).width).toBe(460);
  await page.reload();
  expect((await handle(page)).width).toBe(460);
});

test('at its narrowest the list header still shows every control in the column', async ({
  page,
}) => {
  await open(page, WIN);
  await drag(page, -2000);
  await expect.poll(async () => (await handle(page)).width).toBe(320);
  const problems = await page.getByTestId('list-header').evaluate((header) => {
    const out: string[] = [];
    const column = header.getBoundingClientRect();
    if (header.scrollWidth > header.clientWidth) out.push('the header scrolls sideways');
    for (const control of header.querySelectorAll('button, input')) {
      const box = control.getBoundingClientRect();
      if (box.width === 0) continue;
      if (box.left < column.left - 0.5 || box.right > column.right + 0.5) {
        out.push(`clipped: ${control.getAttribute('aria-label') ?? control.textContent}`);
      }
    }
    return out;
  });
  expect(problems).toEqual([]);
});
