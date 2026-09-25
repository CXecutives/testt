// The sidebar: the arrow on Jobs that hides and shows Archiv and Papierkorb (kept, forced open
// while one of them is open), their grouping in the icon rail, and the order of a click on a
// place when an unsaved profile asks first.

import type { Page } from '@playwright/test';
import { calls, expect, open, settle, test } from './fixtures';

const WIN = '?platform=windows';
const MAC = '?platform=macos';

/** The one pill lies exactly on the entry (full or rail, a main entry or a smaller sub). */
async function pillOn(page: Page, id: string): Promise<void> {
  const nav = page.getByTestId('sidebar').locator('nav');
  await expect
    .poll(async () => {
      const pill = (await nav.locator('.indicator').boundingBox())!;
      const entry = (await page.getByTestId(id).boundingBox())!;
      return [
        pill.x - entry.x,
        pill.y - entry.y,
        pill.width - entry.width,
        pill.height - entry.height,
      ].map((value) => Math.round(value));
    }, id)
    .toEqual([0, 0, 0, 0]);
}

/** Nothing moves any more (a fade, a slide, a turn). */
async function still(page: Page): Promise<void> {
  await expect
    .poll(() =>
      page.evaluate(() => document.getAnimations().filter((a) => a.playState === 'running').length),
    )
    .toBe(0);
}

test('the arrow hides and shows Archiv and Papierkorb; the choice is kept, at start nothing moves', async ({
  page,
}) => {
  await open(page, WIN);
  const toggle = page.getByTestId('places-toggle');
  // Shown by default (the first start and the smoke probe of the app see all five entries).
  await expect(page.locator('[data-testid^="nav-"]')).toHaveCount(5);
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
  await expect(toggle).toHaveAttribute('aria-label', 'Archiv und Papierkorb ausblenden');
  const group = page.getByRole('group', { name: 'Jobs' });
  await expect(toggle).toHaveAttribute('aria-controls', (await group.getAttribute('id'))!);
  await expect(group.getByRole('button')).toHaveText(['Archiv', 'Papierkorb']);
  // At the end of the Jobs row, after the count, and a button of its own.
  const jobs = (await page.getByTestId('nav-jobs').boundingBox())!;
  const arrow = (await toggle.boundingBox())!;
  const count = (await page.getByTestId('nav-jobs').locator('.count').first().boundingBox())!;
  expect(arrow.x).toBeGreaterThan(count.x + count.width);
  expect(arrow.x + arrow.width).toBeLessThanOrEqual(jobs.x + jobs.width);
  expect(Math.abs(arrow.y + arrow.height / 2 - (jobs.y + jobs.height / 2))).toBeLessThan(1);
  expect(await page.getByTestId('nav-jobs').locator('button').count()).toBe(0);
  await toggle.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Archiv und Papierkorb ausblenden');

  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  await expect(toggle).toHaveAttribute('aria-label', 'Archiv und Papierkorb einblenden');
  // They fade out, then the entries below move up (no height animation).
  await expect(page.getByTestId('nav-archive')).toBeHidden();
  await expect(page.getByTestId('nav-trash')).toBeHidden();
  await expect(group).toBeHidden();
  // Hidden, not gone: every entry of the nav always exists (the smoke probe finds five).
  await expect(page.locator('[data-testid^="nav-"]')).toHaveCount(5);
  const profile = (await page.getByTestId('nav-profile').boundingBox())!;
  expect(Math.round(profile.y - jobs.y)).toBe(38);
  // The arrow points right: a quarter turn.
  await still(page);
  const turned = await toggle
    .locator('.chevron')
    .evaluate((node) => new DOMMatrix(getComputedStyle(node).transform));
  expect([Math.round(turned.a), Math.round(turned.b)]).toEqual([0, -1]);

  // Kept across a reload, and simply there: nothing slides, fades or turns at start.
  await page.reload();
  await settle(page);
  expect(
    await page.evaluate(
      () => document.getAnimations().filter((a) => a.playState === 'running').length,
    ),
  ).toBe(0);
  await expect(page.getByTestId('places-toggle')).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByTestId('nav-archive')).toBeHidden();
  await page.getByTestId('places-toggle').click();
  await expect(page.getByTestId('nav-archive')).toBeVisible();
  await page.reload();
  await expect(page.getByTestId('nav-archive')).toBeVisible();
});

test('the pill stays on the current entry when the places fold and unfold', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-settings').click();
  await pillOn(page, 'nav-settings');
  const toggle = page.getByTestId('places-toggle');
  await toggle.click();
  await pillOn(page, 'nav-settings');
  await toggle.click();
  await pillOn(page, 'nav-settings');
  await page.getByTestId('nav-trash').click();
  await pillOn(page, 'nav-trash');
  await page.getByTestId('nav-jobs').click();
  await pillOn(page, 'nav-jobs');
});

test('Archiv or Papierkorb open: the places stay, the arrow waits and says why', async ({
  page,
}) => {
  await open(page, WIN);
  const toggle = page.getByTestId('places-toggle');
  await toggle.click();
  await expect(page.getByTestId('nav-archive')).toBeHidden();
  // Another way into the archive (a search hit there): the places show at once.
  await page.getByTestId('search').fill('Kreditoren');
  await page.getByTestId('also-archive').click();
  await expect(page.getByTestId('nav-archive')).toHaveAttribute('aria-current', 'page');
  await pillOn(page, 'nav-archive');
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
  await expect(toggle).toHaveAttribute('aria-disabled', 'true');
  await toggle.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Bleibt offen, solange du im Archiv bist.');
  // A click changes nothing (the arrow waits).
  await toggle.click({ force: true });
  await page.waitForTimeout(300);
  await expect(page.getByTestId('nav-archive')).toBeVisible();
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
  await page.getByTestId('nav-trash').click();
  await toggle.hover();
  await expect(page.getByRole('tooltip')).toHaveText(
    'Bleibt offen, solange du im Papierkorb bist.',
  );
  // Back to Jobs: they fold away again, as chosen.
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('nav-archive')).toBeHidden();
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  await expect(toggle).not.toHaveAttribute('aria-disabled');
  await pillOn(page, 'nav-jobs');
  // From the archive straight to a view below: they go at once and the pill lands on it.
  await page.getByTestId('also-archive').click();
  await expect(page.getByTestId('nav-archive')).toHaveAttribute('aria-current', 'page');
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('nav-archive')).toBeHidden();
  await pillOn(page, 'nav-settings');
});

for (const os of [WIN, MAC]) {
  test(`the rail groups the places under Jobs, with the arrow and tooltips ${os}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1000, height: 700 });
    await open(page, os);
    const box = async (id: string) => (await page.getByTestId(id).boundingBox())!;
    const jobs = await box('nav-jobs');
    const arrow = await box('places-toggle');
    const archive = await box('nav-archive');
    const trash = await box('nav-trash');
    const profile = await box('nav-profile');
    // Smaller squares right under the Jobs icon, centred on it; the arrow between them.
    expect([jobs.width, jobs.height, archive.width, archive.height]).toEqual([40, 40, 32, 32]);
    const centre = (b: { x: number; width: number }): number => b.x + b.width / 2;
    expect(centre(archive)).toBe(centre(jobs));
    expect(centre(arrow)).toBe(centre(jobs));
    expect(arrow.y).toBe(jobs.y + jobs.height);
    expect(archive.y).toBeGreaterThan(arrow.y + arrow.height - 1);
    // A hairline closes them: the views below stand further off than the places apart.
    expect(profile.y - (trash.y + trash.height)).toBeGreaterThan(
      trash.y - (archive.y + archive.height),
    );
    for (const [id, name] of [
      ['nav-archive', 'Archiv'],
      ['places-toggle', 'Archiv und Papierkorb ausblenden'],
    ] as const) {
      await page.getByTestId(id).hover();
      const tip = page.getByRole('tooltip');
      await expect(tip).toHaveText(name);
      await expect(tip.locator('div')).toHaveCSS('opacity', '1');
      const anchor = await box(id);
      expect((await tip.locator('div').boundingBox())!.x).toBeGreaterThan(anchor.x + anchor.width);
    }
    for (const id of ['nav-archive', 'nav-trash', 'nav-settings', 'nav-jobs']) {
      await page.getByTestId(id).click();
      await pillOn(page, id);
    }
    // Folded in the rail too: only the arrow stays under the Jobs icon.
    await page.getByTestId('places-toggle').click();
    await expect(page.getByTestId('nav-archive')).toBeHidden();
    await page.getByTestId('nav-profile').click();
    await pillOn(page, 'nav-profile');
  });

  test(`at 480 x 360 the rail keeps every entry and the status in the window ${os}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 480, height: 360 });
    await open(page, os);
    const ids = [
      'nav-jobs',
      'places-toggle',
      'nav-archive',
      'nav-trash',
      'nav-profile',
      'nav-settings',
      'run-status',
    ];
    const boxes = await Promise.all(
      ids.map(async (id) => (await page.getByTestId(id).boundingBox())!),
    );
    for (const [at, b] of boxes.entries()) {
      expect(b.y, ids[at]).toBeGreaterThanOrEqual(0);
      expect(b.y + b.height, ids[at]).toBeLessThanOrEqual(360);
      const next = boxes[at + 1];
      if (next)
        expect(next.y, `${ids[at + 1]} below ${ids[at]}`).toBeGreaterThanOrEqual(b.y + b.height);
    }
  });
}

test('an unsaved profile keeps the place until the question is answered', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-profile').click();
  const field = page.getByTestId('view-profile').getByRole('textbox').first();
  await field.fill('Erika Muster');
  await page.getByTestId('nav-archive').click();
  const dialog = page.getByTestId('dialog-leave-profile');
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Abbrechen' }).click();
  // Still the Profil, and the Jobs view is where it was (the inbox, not the archive).
  await expect(page.getByTestId('view-profile')).toBeVisible();
  await expect(page.getByTestId('nav-profile')).toHaveAttribute('aria-current', 'page');
  await expect(field).toHaveValue('Erika Muster');
  expect(
    (await calls(page, 'list_jobs')).some(
      ([, args]) => (args as { query: { place: string } }).query.place === 'archive',
    ),
  ).toBe(false);
  // Asked again and left without saving: the archive it was asked for.
  await page.getByTestId('nav-archive').click();
  await dialog.getByRole('button', { name: 'Verwerfen' }).click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await expect(page.getByTestId('nav-archive')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('search')).toHaveAttribute('placeholder', 'Archiv durchsuchen');
});

test('a click on the place that is open reloads nothing, like Jobs', async ({ page }) => {
  await open(page, WIN);
  const loads = async (place?: string): Promise<number> =>
    (await calls(page, 'list_jobs')).filter(
      ([, args]) =>
        place === undefined || (args as { query: { place: string } }).query.place === place,
    ).length;
  await page.getByTestId('nav-archive').click();
  await expect(page.getByTestId('search')).toHaveAttribute('placeholder', 'Archiv durchsuchen');
  await settle(page);
  const archive = await loads('archive');
  await page.getByTestId('nav-archive').click();
  await settle(page);
  expect(await loads('archive')).toBe(archive);
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('search')).toHaveAttribute('placeholder', 'Jobs durchsuchen');
  await settle(page);
  const all = await loads();
  await page.getByTestId('nav-jobs').click();
  await settle(page);
  expect(await loads()).toBe(all);
});
