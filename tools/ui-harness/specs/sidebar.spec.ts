// The sidebar: the order of a click on a place when an unsaved profile asks first, and a
// click on the place that is open.

import { calls, expect, open, settle, test } from './fixtures';

const WIN = '?platform=windows';

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
