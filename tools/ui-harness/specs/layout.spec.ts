// Window sizes the app must survive: the minimum-ish 780 × 560, typical laptops at 125 %
// scaling (1280 × 720, 1536 × 864) and common desktops.

import { expect, open, test } from './fixtures';

const SIZES = [
  { width: 780, height: 560 },
  { width: 1280, height: 720 },
  { width: 1360, height: 900 },
  { width: 1536, height: 864 },
  { width: 1920, height: 1080 },
];

for (const size of SIZES) {
  for (const query of ['?platform=windows', '?platform=macos', '?gallery']) {
    test(`no horizontal scroll, nothing clipped at ${size.width}x${size.height} ${query}`, async ({
      page,
    }) => {
      await page.setViewportSize(size);
      await open(page, query);
      const problems = await page.evaluate(() => {
        const out: string[] = [];
        for (const node of [
          document.documentElement,
          ...document.querySelectorAll('.view, [data-testid="gallery"]'),
        ]) {
          if (node.scrollWidth > node.clientWidth) {
            out.push(
              `${node.tagName}.${node.className}: ${node.scrollWidth} > ${node.clientWidth}`,
            );
          }
        }
        // Every visible control of the shell lies fully inside the window.
        const width = document.documentElement.clientWidth;
        for (const control of document.querySelectorAll('[data-testid="titlebar"] button')) {
          const box = control.getBoundingClientRect();
          if (box.left < 0 || box.right > width + 0.5) {
            out.push(`clipped: ${control.getAttribute('aria-label') ?? control.textContent}`);
          }
        }
        return out;
      });
      expect(problems).toEqual([]);
    });
  }
}

for (const [width, rail] of [
  [1280, false],
  [1100, false],
  [1099, true],
  [780, true],
] as const) {
  test(`the sidebar ${rail ? 'is the icon rail' : 'is full'} at ${width} px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 700 });
    await open(page, '?platform=windows');
    const sidebar = await page.getByTestId('sidebar').boundingBox();
    expect(sidebar?.width).toBe(rail ? 64 : 232);
    const label = page.getByTestId('nav-profile');
    if (rail) {
      await expect(label).toHaveAttribute('aria-label', 'Profil');
      await label.hover();
      await expect(page.getByRole('tooltip')).toHaveText('Profil');
      await expect(page.getByTestId('brand').getByText('Job-Alert-Monitor')).toHaveCount(0);
    } else {
      await expect(label).toHaveText('Profil');
      await expect(page.getByTestId('brand')).toContainText('Job-Alert-Monitor');
    }
  });
}

test('empty screens are never dead: an icon, one sentence, one way on, centred', async ({
  page,
}) => {
  await open(page, '?platform=windows&scenario=no-profile');
  await page.getByTestId('nav-profile').click();
  const empty = page.getByTestId('profile-empty');
  await expect(empty).toBeVisible();
  // "Abrufen" in the sidebar is the window's primary; the empty state stays secondary.
  await expect(empty.locator('.btn.primary')).toHaveCount(0);
  await expect(empty.getByRole('button')).toHaveCount(2);
  const offset = await empty.evaluate((node) => {
    const view = node.closest('.view')!.getBoundingClientRect();
    const box = node.getBoundingClientRect();
    return Math.abs(box.left + box.width / 2 - (view.left + view.width / 2));
  });
  expect(offset).toBeLessThanOrEqual(1);
  expect(await page.locator('.btn.primary').count()).toBe(1);
});

test('toasts: at most three, they stay while hovered and leave on their own', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.getByTestId('nav-settings').click();
  for (let i = 0; i < 4; i += 1) await page.getByTestId('toggle-auto-fetch').click();
  const toasts = page.getByTestId('toast');
  await expect(toasts).toHaveCount(3);
  await toasts.first().hover();
  await page.waitForTimeout(4500);
  await expect(toasts).toHaveCount(1);
  await page.mouse.move(5, 5);
  await expect(toasts).toHaveCount(0, { timeout: 6000 });
});

for (const scenario of ['default', 'first-run', 'running']) {
  for (const tab of ['nav-jobs', 'nav-profile', 'nav-settings']) {
    test(`nothing clipped or scrolling sideways at 780x560: ${scenario} ${tab}`, async ({
      page,
    }) => {
      await page.setViewportSize({ width: 780, height: 560 });
      await open(page, `?platform=windows&scenario=${scenario}`);
      await page.getByTestId(tab).click();
      await page.waitForTimeout(300);
      const wide = await page.evaluate(() =>
        [...document.querySelectorAll('.view, .view *')]
          .filter(
            (node) =>
              node.scrollWidth > node.clientWidth + 1 &&
              getComputedStyle(node).overflowX !== 'visible' &&
              // The meter clips its travelling light edge on purpose.
              node.closest('[role="progressbar"]') === null,
          )
          .map((node) => `${node.tagName}.${node.className}`),
      );
      expect(wide).toEqual([]);
    });
  }
}
