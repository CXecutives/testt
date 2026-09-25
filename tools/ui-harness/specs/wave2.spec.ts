// Wave 2: the reader's terms, the search over every word, and the fixes of the ship audit
// (see docs/PLAN.md).

import type { Page } from '@playwright/test';
import { expect, open, test } from './fixtures';

const WIN = '?platform=windows';
const list = (page: Page) => page.getByTestId('job-list');
const row = (page: Page, key: string) => list(page).getByTestId(`job-row-${key}`);
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
const stage = (page: Page) => page.getByTestId('stage');

test('the reader shows the ad rate and start without a profile minimum', async ({ page }) => {
  for (const [lang, rate, start] of [
    ['', '1.200 €/Tag', 'ab sofort'],
    ['&lang=en', '€1,200/day', 'starts now'],
  ] as const) {
    await open(page, `${WIN}&scenario=no-minimum${lang}`);
    await row(page, 'freelancermap-2801').click();
    // The profile sets neither a minimum rate nor a start: the ad's values join the line.
    const clean = stage(page).getByTestId('criteria-clean');
    await expect(clean).toContainText(rate);
    await expect(clean).toContainText(start);
  }
  // With a minimum the criterion chip says the rate, and no fact repeats it.
  await open(page, WIN);
  await row(page, 'freelancermap-2801').click();
  const clean = stage(page).getByTestId('criteria-clean');
  await expect(clean).toContainText('1.200 €/Tag');
  const text = async () => (await clean.textContent())?.replace(/\s/g, ' ') ?? '';
  await expect.poll(async () => (await text()).split('1.200 €/Tag').length).toBe(2);
});

test('the search matches every word in any field and the portal name', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const search = page.getByTestId('search');
  // Two words that do not stand next to each other, in two fields.
  await search.fill('bremen CONTROLLING');
  await expect(rows(page)).toHaveCount(1);
  await expect(row(page, 'linkedin-4100200301')).toBeVisible();
  await search.fill('controlling berlin');
  await expect(rows(page)).toHaveCount(0);
  // The portal's name narrows to its jobs, and the count of the archive follows.
  await search.fill('linkedin bremen');
  await expect(page.getByTestId('also-archive')).toHaveText('Auch im Archiv (1)');
  const keys = await rows(page).evaluateAll((all) => all.map((r) => r.dataset.testid ?? ''));
  expect(keys.length).toBeGreaterThan(0);
  expect(keys.every((key) => key.startsWith('job-row-linkedin-'))).toBe(true);
});

test('a refused value of the hidden permanent rules shows them with the limit', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('nav-profile').click();
  await page.getByTestId('profile-remote-min').fill('150');
  await page.getByTestId('profile-no-permanent').click();
  await expect(page.getByTestId('profile-permanent')).toHaveCount(0);
  await page.getByTestId('profile-save').click();
  // The block comes back with the refused field, its limit and the caret.
  const field = page.getByTestId('profile-remote-min');
  await expect(field).toBeFocused();
  await expect(field).toHaveAttribute('aria-invalid', 'true');
  await expect(page.locator('[data-field="permanentRemoteMin"]')).toContainText('Höchstens 100.');
  // Under its own label the refusal does not name the field again.
  // It stays while the value is put right.
  await field.fill('50');
  await expect(field).toBeVisible();
  await page.getByTestId('profile-min-rate').fill('200000');
  await page.getByTestId('profile-save').click();
  const rate = page.locator('[data-field="minDayRate"]');
  await expect(rate).toContainText('Höchstens 100.000.');
  await expect(rate).not.toContainText('Der Wert bei');
});

test('a decimal number is cut to a whole one, never joined', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-profile').click();
  const years = page.getByTestId('profile-years');
  await years.fill('7,5');
  await expect(page.getByTestId('profile-years-rounded')).toHaveCount(0);
  await page.getByTestId('profile-name-field').focus();
  await expect(years).toHaveValue('7');
  await expect(page.getByTestId('profile-years-rounded')).toHaveText(
    'Auf eine ganze Zahl abgerundet.',
  );
  await page.getByTestId('competence-years').first().fill('2.5');
  await page.getByTestId('profile-save').click();
  const all = await page.evaluate(() => window.__harness.calls);
  const sent = all.filter(([name]) => name === 'save_profile').at(-1)![1] as {
    save: { after: { years: number; competences: { years: number | null }[] } };
  };
  expect(sent.save.after.years).toBe(7);
  expect(sent.save.after.competences[0]!.years).toBe(2);
});

test('text typed into a chip field is a change: Ctrl+S takes it, closing asks', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('nav-profile').click();
  const save = page.getByTestId('profile-save');
  const tools = page.getByTestId('profile-tools').locator('input');
  await tools.fill('Miro');
  // Typed, not yet a chip: Speichern waits no more, and the backend knows it is unsaved.
  await expect(save).not.toHaveAttribute('aria-disabled', 'true');
  await expect.poll(() => page.evaluate(() => window.__harness.unsaved)).toBe(true);
  await page.keyboard.press('Control+s');
  const saves = async () =>
    (await page.evaluate(() => window.__harness.calls)).filter(([name]) => name === 'save_profile');
  await expect.poll(async () => (await saves()).length).toBe(1);
  const sent = (await saves()).at(-1)![1] as { save: { after: { tools: string[] } } };
  expect(sent.save.after.tools).toContain('Miro');
  await expect(tools).toBeFocused();
  await expect(tools).toHaveValue('');

  // Closing the window with typed text asks first.
  await page.getByTestId('profile-regions').locator('input').fill('Berlin');
  await page.evaluate(() => window.__harness.requestClose());
  await expect(page.getByTestId('dialog-leave-profile')).toBeVisible();
  expect(await page.evaluate(() => window.__harness.closed)).toBe(false);
});

test('from a CV: the pasted answer outlasts Esc and the view, the clipboard stays', async ({
  page,
}) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page, `${WIN}&scenario=no-profile`);
  await page.getByTestId('nav-profile').click();
  const fromCv = page.getByTestId('profile-empty').getByRole('button', {
    name: 'Aus Lebenslauf erstellen',
  });
  await fromCv.click();
  const answer = page.getByTestId('paste-answer');
  await answer.fill('{"name": "Erika Muster"}');
  await page.evaluate(() => navigator.clipboard.writeText('the answer'));
  await answer.press('Escape');
  await expect(page.getByTestId('profile-paste')).toHaveCount(0);
  // Another view and back: the answer is there, the clipboard untouched.
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('nav-profile').click();
  await fromCv.click();
  await expect(answer).toHaveValue('{"name": "Erika Muster"}');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('the answer');
});

test('a failed save from the leave dialog leaves the caret in the refused field', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('nav-profile').click();
  const rate = page.getByTestId('profile-min-rate');
  await rate.fill('200000');
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('dialog-leave-profile').getByRole('button', { name: 'Speichern' }).click();
  await expect(page.getByTestId('dialog-leave-profile')).toHaveCount(0);
  await expect(rate).toBeFocused();
});
