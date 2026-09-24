import { expect, expectShot, open, settle, test } from './fixtures';

const SECTIONS = [
  'colours',
  'type',
  'spacing',
  'radii',
  'shadows',
  'gradients',
  'motion',
  'icons',
  'buttons',
  'navigation',
  'tiles',
  'empty',
  'activity',
];

test('the gallery renders every board and component section', async ({ page }) => {
  await open(page, '?gallery');
  await expect(page.getByTestId('gallery')).toBeVisible();
  for (const section of SECTIONS) {
    await expect(page.getByTestId(`gallery-${section}`)).toBeVisible();
  }
  // Contrast is computed from the live tokens: body text passes AA on white.
  await expect(page.getByTestId('swatch-text')).toContainText('AA');
});

test('baseline: gallery', async ({ page }) => {
  await open(page, '?gallery');
  const height = await page.getByTestId('gallery').evaluate((node) => node.scrollHeight);
  await page.setViewportSize({ width: 1360, height: Math.ceil(height) });
  await settle(page);
  await expectShot(page, 'gallery', { maxDiffPixelRatio: 0.004 });
});
