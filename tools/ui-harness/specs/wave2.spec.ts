// Wave 2: the reader's terms, the search over every word, and the fixes of the ship audit
// (see docs/PLAN.md).

import type { Page } from '@playwright/test';
import { expect, NOW, open, runFinished, test } from './fixtures';

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
  browserName,
}) => {
  // WebKit has no clipboard permissions to grant: its part is the answer alone.
  const clipboard = browserName === 'chromium';
  if (clipboard) await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page, `${WIN}&scenario=no-profile`);
  await page.getByTestId('nav-profile').click();
  const fromCv = page.getByTestId('profile-empty').getByRole('button', {
    name: 'Aus Lebenslauf anlegen',
  });
  await fromCv.click();
  const answer = page.getByTestId('paste-answer');
  await answer.fill('{"name": "Erika Muster"}');
  if (clipboard) await page.evaluate(() => navigator.clipboard.writeText('the answer'));
  await answer.press('Escape');
  await expect(page.getByTestId('profile-paste')).toHaveCount(0);
  // Another view and back: the answer is there, the clipboard untouched.
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('nav-profile').click();
  await fromCv.click();
  await expect(answer).toHaveValue('{"name": "Erika Muster"}');
  if (clipboard) {
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('the answer');
  }
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

test('the arrow keys follow the order on screen after an exclusion changes in place', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await row(page, 'linkedin-4100200305').click();
  await stage(page).getByTestId('override').click();
  const openKey = () =>
    list(page).locator('[data-open]').getAttribute('data-key', { timeout: 2_000 });
  const drawn = async () =>
    list(page)
      .locator('[data-key]')
      .evaluateAll((all) => all.map((item) => (item as HTMLElement).dataset.key ?? ''));
  // The counted row now stands above the divider; the keys walk the rows as drawn.
  await expect.poll(async () => (await drawn()).indexOf('linkedin:4100200305')).toBeGreaterThan(0);
  const order = await drawn();
  const at = order.indexOf('linkedin:4100200305');
  await row(page, 'linkedin-4100200305').focus();
  await page.keyboard.press('ArrowDown');
  await expect.poll(openKey).toBe(order[at + 1]);
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowUp');
  await expect.poll(openKey).toBe(order[at - 1]);
});

test('deleting the open job for good opens the next one', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  for (const key of ['freelancermap-2803', 'linkedin-4100200302']) {
    await row(page, key).hover();
    await page.getByTestId(`trash-${key}`).click();
    await page.waitForTimeout(550);
  }
  await page.getByTestId('nav-trash').click();
  const first = rows(page).first();
  const firstKey = await first.getAttribute('data-testid');
  const other =
    firstKey === 'job-row-freelancermap-2803' ? 'linkedin:4100200302' : 'freelancermap:2803';
  await first.click();
  await stage(page).getByTestId('reader-purge').click();
  await page.getByTestId('dialog-purge').getByTestId('dialog-confirm').click();
  await expect(list(page).locator(`[data-open][data-key="${other}"]`)).toHaveCount(1);
});

test('the day overview starts without a hairline when only open points are left', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=offline`);
  await page
    .getByTestId('mark-all-read')
    .click()
    .catch(() => undefined);
  const firstBlock = page.getByTestId('day-overview').locator(':scope > .block').first();
  await expect(firstBlock).toBeVisible();
  const border = await firstBlock.evaluate((el) => getComputedStyle(el).borderTopWidth);
  expect(border).toBe('0px');
});

test('one column: back after the arrow keys shows the open row, and Ctrl+F reaches the search', async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await open(page, WIN);
  await rows(page).first().click();
  for (let step = 0; step < 6; step += 1) await page.keyboard.press('ArrowDown');
  const key = await list(page).locator('[data-open]').getAttribute('data-key');
  await page.keyboard.press('Escape');
  const item = list(page).locator(`[data-key="${key}"]`);
  await expect(item).toBeInViewport();
  await expect(item.locator('[data-testid^="job-row-"]')).toBeFocused();
  await rows(page).first().click();
  await page.keyboard.press('Control+f');
  await expect(page.getByTestId('search')).toBeFocused();
});

test('a new form and the steps from a CV put the caret where the work starts', async ({ page }) => {
  await open(page, `${WIN}&scenario=no-profile`);
  await page.getByTestId('nav-profile').click();
  const empty = page.getByTestId('profile-empty');
  await empty.getByRole('button', { name: 'Aus Lebenslauf anlegen' }).click();
  await expect(page.getByTestId('paste-answer')).toBeFocused();
  await page.getByTestId('paste-cancel').click();
  await empty.getByRole('button', { name: 'Profil anlegen' }).click();
  await expect(page.getByTestId('profile-name-field')).toBeFocused();
});

test('typing a country that is chosen already says nothing and Enter clears it', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('nav-profile').click();
  const input = page.getByTestId('profile-countries').locator('input');
  await input.fill('Österreich');
  await expect(page.getByTestId('profile-countries-none')).toHaveCount(0);
  await input.press('Enter');
  await expect(input).toHaveValue('');
  await expect(page.getByTestId('profile-countries').locator('.chip')).toHaveCount(2);
});

test('at 480 px the countries field keeps its width, DACH sits under it', async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 800 });
  await open(page, WIN);
  await page.getByTestId('nav-profile').click();
  const field = await page.getByTestId('profile-countries').boundingBox();
  const tools = await page.getByTestId('profile-tools').boundingBox();
  const dach = await page.getByTestId('profile-dach').boundingBox();
  expect(Math.abs(field!.width - tools!.width)).toBeLessThan(2);
  expect(dach!.y).toBeGreaterThan(field!.y + field!.height - 1);
});

test('an unreadable country is said at its field like any other value', async ({ page }) => {
  await open(page, `${WIN}&scenario=profile-unreadable`);
  await page.getByTestId('nav-profile').click();
  const scope = page.locator('[data-field="countries"]');
  await expect(scope.locator('input')).toHaveAttribute('aria-invalid', 'true');
  await expect(scope.getByTestId('value-remove')).toBeVisible();
});

test('undoing moves in another order puts the rows back where they stood', async ({ page }) => {
  await open(page, WIN);
  const keys = async () =>
    (await rows(page).evaluateAll((all) => all.map((r) => r.dataset.testid ?? ''))).slice(0, 3);
  const before = await keys();
  for (const [index, id] of before.slice(0, 2).entries()) {
    const key = id.replace('job-row-', '');
    await row(page, key).hover();
    await page.getByTestId(`archive-${key}`).click();
    await page.waitForTimeout(550);
    // Later than the 2 s in which moves join one toast.
    await page.clock.setFixedTime(new Date(NOW.getTime() + (index + 1) * 5_000));
  }
  const toasts = page.getByTestId('toast');
  await expect(toasts).toHaveCount(2);
  // The older toast first, then the newer one.
  await toasts.first().getByRole('button', { name: 'Rückgängig' }).click();
  await page.waitForTimeout(300);
  await toasts.last().getByRole('button', { name: 'Rückgängig' }).click();
  await expect.poll(keys).toEqual(before);
});

test('the end of a run keeps the jobs read in Neu, the open one in its place', async ({ page }) => {
  await open(page, WIN);
  const keys = async () => rows(page).evaluateAll((all) => all.map((r) => r.dataset.testid ?? ''));
  const before = await keys();
  const first = before[0]!.replace('job-row-', '');
  const third = before[2]!.replace('job-row-', '');
  await row(page, first).click();
  await row(page, third).click();
  await page.getByTestId('fetch').click();
  await runFinished(page);
  const after = await keys();
  // Both read jobs are still listed, in the list's order.
  expect(after).toContain(`job-row-${first}`);
  expect(after.indexOf(`job-row-${first}`)).toBeLessThan(after.indexOf(`job-row-${third}`));
  await expect(list(page).locator('[data-open]')).toHaveAttribute(
    'data-key',
    third.replace('-', ':'),
  );
});

test('with the focus nowhere the arrows, Home and End scroll Profil too', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 600 });
  await open(page, WIN);
  await page.getByTestId('nav-profile').click();
  const view = page.getByTestId('view-profile');
  const top = (): Promise<number> => view.evaluate((node) => node.scrollTop);
  await page.getByTestId('section-person').locator('h2').first().click();
  await page.keyboard.press('ArrowDown');
  await expect.poll(top).toBe(40);
  await page.keyboard.press('End');
  await expect
    .poll(() => view.evaluate((node) => node.scrollHeight - node.clientHeight - node.scrollTop))
    .toBeLessThanOrEqual(1);
  await page.keyboard.press('Home');
  await expect.poll(top).toBe(0);
});

test('a fetch started from the Archiv leads to its new jobs', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-archive').click();
  await page.getByTestId('fetch').click();
  await runFinished(page);
  const show = page.getByTestId('run-show-new');
  if (!(await show.isVisible())) await page.getByTestId('run-toggle').click();
  await show.click();
  await expect(page.getByTestId('facet').getByRole('radio', { name: /Neu/ })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  // In Jobs the list itself shows the new jobs: no such button.
  await expect(page.getByTestId('run-show-new')).toHaveCount(0);
});
