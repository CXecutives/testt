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

test('placeholders are never dead: mark, one sentence, one action, centred', async ({ page }) => {
  await open(page, '?platform=windows');
  for (const [tab, id] of [['tab-profile', 'placeholder-profile']] as const) {
    await page.getByTestId(tab).click();
    const placeholder = page.getByTestId(id);
    await expect(placeholder).toBeVisible();
    await expect(placeholder.getByRole('button')).toHaveCount(1);
    const offset = await placeholder.evaluate((node) => {
      const box = node.getBoundingClientRect();
      return Math.abs(box.left + box.width / 2 - document.documentElement.clientWidth / 2);
    });
    expect(offset).toBeLessThanOrEqual(1);
  }
});
