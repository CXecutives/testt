// Window sizes the app must survive (minimum 760 × 520 in tauri.conf.json).

import { expect, open, test } from './fixtures';

const SIZES = [
  { width: 780, height: 560 },
  { width: 1360, height: 900 },
  { width: 1920, height: 1080 },
];

for (const size of SIZES) {
  for (const query of ['?platform=windows', '?platform=macos', '?gallery']) {
    test(`no horizontal scroll at ${size.width}x${size.height} ${query}`, async ({ page }) => {
      await page.setViewportSize(size);
      await open(page, query);
      const overflow = await page.evaluate(() =>
        [document.documentElement, ...document.querySelectorAll('.view, [data-testid="gallery"]')]
          .filter((node) => node.scrollWidth > node.clientWidth)
          .map(
            (node) =>
              `${node.tagName}.${node.className}: ${node.scrollWidth} > ${node.clientWidth}`,
          ),
      );
      expect(overflow).toEqual([]);
    });
  }
}

test('the brand name hides below 900 px, the tabs stay centered', async ({ page }) => {
  await page.setViewportSize({ width: 880, height: 600 });
  await open(page, '?platform=windows');
  await expect(page.getByTestId('brand').getByText('Job-Alert-Monitor')).toBeHidden();
  const { center, tabs } = await page.evaluate(() => {
    const bar = document.querySelector('[data-testid="titlebar"]')!.getBoundingClientRect();
    const list = document.querySelector('[role="tablist"]')!.getBoundingClientRect();
    return { center: bar.left + bar.width / 2, tabs: list.left + list.width / 2 };
  });
  expect(Math.abs(center - tabs)).toBeLessThanOrEqual(1);
});
