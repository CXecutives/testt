// Window sizes the app must survive: the minimum 480 × 360 (small enough to snap into every
// Windows 11 layout, quarters of 1366 × 768 included), typical laptops at 125 % scaling
// (1280 × 720, 1536 × 864) and common desktops.

import { expect, open, test } from './fixtures';

const SIZES = [
  { width: 480, height: 360 },
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
        // Every visible control of the list header lies fully inside the window.
        const width = document.documentElement.clientWidth;
        for (const control of document.querySelectorAll('[data-testid="list-header"] button')) {
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
    expect(sidebar?.width).toBe(rail ? 64 : 196);
    const label = page.getByTestId('nav-profile');
    if (rail) {
      await expect(label).toHaveAttribute('aria-label', 'Profil');
      await label.hover();
      await expect(page.getByRole('tooltip')).toHaveText('Profil');
    } else {
      await expect(label).toHaveText('Profil');
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
  // The next step is the one primary on screen: "Profil anlegen" ("Abrufen" lives in the
  // list of the Jobs view), with the two other ways in next to it.
  await expect(empty.locator('.btn.primary')).toHaveCount(1);
  await expect(page.getByTestId('fetch')).toHaveCount(0);
  await expect(empty.getByRole('button')).toHaveCount(3);
  // Centred across, at about 38 % of the height (not dead centre).
  const place = await empty.evaluate((node) => {
    const view = node.closest('.view')!.getBoundingClientRect();
    const box = node.getBoundingClientRect();
    return {
      across: Math.abs(box.left + box.width / 2 - (view.left + view.width / 2)),
      down: (box.top + box.height / 2 - view.top) / view.height,
    };
  });
  expect(place.across).toBeLessThanOrEqual(1);
  expect(place.down).toBeGreaterThan(0.3);
  expect(place.down).toBeLessThan(0.46);
  expect(await page.locator('.btn.primary').count()).toBe(1);
  // Back in the Jobs view "Abrufen" is the primary again.
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('fetch')).toHaveClass(/primary/);
});

test('toasts: at most three, they stay while hovered and leave on their own', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.getByTestId('nav-settings').click();
  // Switches answer by themselves; rewriting the text files still reports by toast.
  for (let i = 0; i < 4; i += 1) await page.getByTestId('txt-rewrite').click();
  const toasts = page.getByTestId('toast');
  await expect(toasts).toHaveCount(3);
  await toasts.first().hover();
  await page.waitForTimeout(4500);
  await expect(toasts).toHaveCount(1);
  await page.mouse.move(5, 5);
  await expect(toasts).toHaveCount(0, { timeout: 6000 });
});

// The first-run page keeps the sidebar inert, so only its Jobs tab is reachable there;
// Profil and Einstellungen without a profile come from the no-profile scenario.
for (const [width, height] of [
  [480, 360],
  [780, 560],
] as const) {
  for (const scenario of ['default', 'first-run', 'running', 'no-profile']) {
    for (const tab of scenario === 'first-run'
      ? ['nav-jobs']
      : ['nav-jobs', 'nav-profile', 'nav-settings']) {
      test(`nothing clipped or scrolling sideways at ${width}x${height}: ${scenario} ${tab}`, async ({
        page,
      }) => {
        await page.setViewportSize({ width, height });
        await open(page, `?platform=windows&scenario=${scenario}`);
        if (scenario !== 'first-run') await page.getByTestId(tab).click();
        await page.waitForTimeout(300);
        const wide = await page.evaluate(() =>
          [...document.querySelectorAll('.view, .view *')]
            .filter(
              (node) =>
                node.scrollWidth > node.clientWidth + 1 &&
                getComputedStyle(node).overflowX !== 'visible' &&
                // A one-line text that ends in an ellipsis is cut on purpose.
                getComputedStyle(node).textOverflow !== 'ellipsis' &&
                // Meters and skeletons clip their moving light on purpose.
                node.closest('[role="progressbar"], [aria-hidden="true"]') === null &&
                // A long value scrolls inside its own field, as in every native field.
                !node.matches('input, textarea'),
            )
            .map((node) => `${node.tagName}.${node.className}`),
        );
        expect(wide).toEqual([]);
      });
    }
  }
}
