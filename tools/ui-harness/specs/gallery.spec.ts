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
  'cards',
  'badges',
  'loading',
  'inputs',
  'rings',
  'stats',
  'notices',
  'dialogs',
  'reasons',
  'rows',
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

test('score rings count up once visible; excluded and unscorable show no number', async ({
  page,
}) => {
  await open(page, '?gallery');
  const high = page.getByTestId('ring-high-lg');
  await high.scrollIntoViewIfNeeded();
  await expect(high).toHaveText('91');
  await expect(high).toHaveAttribute(
    'aria-label',
    new RegExp(`Passung 91${String.fromCharCode(0x202f)}% · Hohe Passung`),
  );
  await expect(page.getByTestId('ring-excluded-lg')).toHaveText('');
  await expect(page.getByTestId('ring-unscorable-lg')).toHaveText('–');
});

test('under reduced motion the rings jump to their value', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, '?gallery');
  const mid = page.getByTestId('ring-mid-lg');
  await mid.scrollIntoViewIfNeeded();
  await settle(page);
  await expect(mid).toHaveText('64', { timeout: 200 });
});

test('controls: toggle, segmented, password reveal, search clear, disclosure', async ({ page }) => {
  await open(page, '?gallery');
  const section = page.getByTestId('gallery-inputs');
  const toggle = section.getByRole('switch').first();
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');

  const facet = page.getByTestId('segmented-facet');
  await facet.getByRole('radio', { name: /Alle/ }).click();
  await expect(facet.getByRole('radio', { name: /Alle/ })).toHaveAttribute('aria-checked', 'true');

  const password = page.locator('#gallery-password');
  await expect(password).toHaveAttribute('type', 'password');
  await section.getByRole('button', { name: 'Passwort zeigen' }).click();
  await expect(password).toHaveAttribute('type', 'text');
  await expect(password).toHaveAttribute('spellcheck', 'false');

  const search = section.getByRole('textbox', { name: 'Jobs durchsuchen' }).first();
  await expect(search).toHaveValue('Controlling');
  await section.getByRole('button', { name: 'Suche leeren' }).click();
  await expect(search).toHaveValue('');

  const disclosure = page.getByTestId('disclosure').getByRole('button');
  await expect(disclosure).toHaveAttribute('aria-expanded', 'true');
  await disclosure.click();
  await expect(disclosure).toHaveAttribute('aria-expanded', 'false');
});

test('dialog: Esc cancels, a press that ends on the scrim keeps it open', async ({ page }) => {
  await open(page, '?gallery');
  await page.getByTestId('open-danger').click();
  const dialog = page.getByTestId('dialog-danger');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Abbrechen' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);

  await page.getByTestId('open-danger').click();
  await expect(dialog).toBeVisible();
  const box = (await dialog.boundingBox())!;
  await page.mouse.move(box.x + 20, box.y + 20);
  await page.mouse.down();
  await page.mouse.move(5, 5);
  await page.mouse.up();
  await expect(dialog).toBeVisible();
  await page.mouse.click(5, 5);
  await expect(dialog).toHaveCount(0);
});

test('job rows select on click and reorder without losing a row', async ({ page }) => {
  await open(page, '?gallery');
  const list = page.getByTestId('job-list');
  await list.scrollIntoViewIfNeeded();
  const rows = list.locator('[data-testid^="job-row-"]');
  await expect(rows).toHaveCount(6);
  await rows.nth(1).click();
  await expect(rows.nth(1)).toHaveAttribute('aria-current', 'true');
  const first = await rows.first().getAttribute('data-testid');
  await page.getByTestId('rows-shuffle').click();
  await expect(rows).toHaveCount(6);
  await expect(rows.last()).toHaveAttribute('data-testid', first!);
});

test('baseline: gallery (reduced motion, so counters and loops are at rest)', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, '?gallery');
  const height = await page.getByTestId('gallery').evaluate((node) => node.scrollHeight);
  await page.setViewportSize({ width: 1360, height: Math.ceil(height) });
  await settle(page);
  // A full-page capture of the long gallery takes WebKit a few seconds per frame.
  test.setTimeout(90_000);
  await expectShot(page, 'gallery', { maxDiffPixelRatio: 0.004, timeout: 30_000 });
});
