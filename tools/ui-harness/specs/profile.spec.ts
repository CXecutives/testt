// The Profil view against the stub: the profile as a form in its eight blocks, the three ways
// in, saving, discarding, leaving and closing the window with unsaved changes, an AI's answer,
// the chip and number fields, values of the file that do not read, refused values, removing
// with undo and what the app reads in the file.

import type { Locator, Page } from '@playwright/test';
import { calls, expect, expectShot, open, settle, test } from './fixtures';
import type { ProfileSave } from '../../../ui/src/lib/ipc/types';

async function profile(page: Page, scenario = 'default', extra = ''): Promise<void> {
  await open(page, `?platform=windows&scenario=${scenario}${extra}`);
  await page.getByTestId('nav-profile').click();
  await expect(page.getByTestId('profile')).toBeVisible();
}

const save = (page: Page): Locator => page.getByTestId('profile-save');
const discard = (page: Page): Locator => page.getByTestId('profile-discard');
const chips = (field: Locator): Locator => field.locator('.chip .text');
const badge = (page: Page): Locator => page.getByTestId('profile-quality');

async function lastSave(page: Page): Promise<ProfileSave> {
  const all = await calls(page, 'save_profile');
  return (all.at(-1)![1] as { save: ProfileSave }).save;
}

async function paste(field: Locator, text: string): Promise<void> {
  await field.evaluate((input, value) => {
    const data = new DataTransfer();
    data.setData('text/plain', value);
    input.dispatchEvent(
      new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true }),
    );
  }, text);
}

/** The tooltip of an element after hovering it. */
async function tooltipOf(page: Page, target: Locator): Promise<string> {
  await target.hover();
  const tip = page.getByRole('tooltip');
  await expect(tip).toBeVisible();
  return (await tip.textContent()) ?? '';
}

test('the profile is a form, filled from the stored profile', async ({ page }) => {
  await profile(page);
  // The person first, the time of the last save; the file name only as the tooltip.
  await expect(page.getByTestId('profile-name')).toHaveText('Erika Beispiel');
  await expect(page.getByTestId('profile-role')).toHaveText('Interim Managerin Finanzen');
  await expect(page.getByTestId('profile-saved-at')).toHaveText('Gespeichert 21.09.2026, 09:30');
  const head = page.getByTestId('profile-file');
  await expect(head).not.toContainText('KB');
  // Well filled, but something to check: the badge says so, its tooltip names it.
  await expect(badge(page)).toHaveText('Etwas prüfen');
  expect(await tooltipOf(page, badge(page).locator('.badge'))).toBe(
    '„Mindest-Remote-Anteil“ ist nicht lesbar.',
  );
  // What the app reads: terms for the match, the Schwerpunkte, its specialist vocabulary.
  await expect(page.getByTestId('profile-understood')).toHaveText(
    '42 Begriffe für die Passung · 2 Schwerpunkte · Fachwortschatz für Finanzen und SAP',
  );
  // The value the app could not read is said at its field, not in the head.
  await expect(page.getByTestId('profile-warning')).toHaveCount(0);
  await expect(page.getByTestId('section-criteria')).toContainText(
    'In der Datei stand „viel“, das ist keine Zahl.',
  );
  // ... with the red border and aria-invalid of every field with an error.
  await expect(page.getByTestId('profile-remote-min')).toHaveAttribute('aria-invalid', 'true');
  // The eight blocks, in the order a consultant thinks.
  const sections = await page
    .locator('[data-testid^="section-"]')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-testid')));
  expect(sections).toEqual([
    'section-person',
    'section-competences',
    'section-experience',
    'section-wishes',
    'section-criteria',
    'section-availability',
    'section-understood',
  ]);
  // Wishes say what they do: they nudge, they never exclude.
  await expect(page.getByTestId('section-wishes')).toContainText(
    'Wünsche verschieben die Bewertung leicht, sie schließen nichts aus.',
  );
  await expect(page.getByTestId('profile-name-field')).toHaveValue('Erika Beispiel');
  await expect(chips(page.getByTestId('profile-roles'))).toHaveText(['Interim CFO']);
  const rows = page.getByTestId('competence-row');
  await expect(rows).toHaveCount(6);
  await expect(rows.nth(1).getByTestId('competence-name')).toHaveValue('Controlling');
  await expect(rows.nth(1).getByTestId('competence-years')).toHaveValue('18');
  await expect(chips(rows.nth(1).getByTestId('competence-aliases'))).toHaveText([
    'Financial Controlling',
    'FP&A',
  ]);
  // Schwerpunkte are starred, listed under the competences, and the star explains itself.
  await expect(rows.nth(1).getByTestId('competence-star')).toHaveAttribute('aria-pressed', 'true');
  await expect(rows.nth(0).getByTestId('competence-star')).toHaveAttribute('aria-pressed', 'false');
  await expect(chips(page.getByTestId('focus'))).toHaveText([
    'Controlling',
    'Konzernrechnungslegung nach IFRS',
  ]);
  await expect(page.getByTestId('focus')).toContainText(
    'Kompetenzen mit Stern zählen doppelt, höchstens fünf.',
  );
  const english = page.getByTestId('language-row').nth(1);
  await expect(english.getByRole('button', { name: 'B2' })).toHaveAttribute('aria-pressed', 'true');
  await expect(
    page.getByTestId('profile-remote').getByRole('button', { name: 'Überwiegend remote' }),
  ).toHaveAttribute('aria-pressed', 'true');
  const countries = page.getByTestId('profile-countries');
  await expect(countries.getByRole('button', { name: 'Deutschland' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(countries.getByRole('button', { name: 'Schweiz' })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
  await expect(page.getByTestId('profile-no-anue')).toHaveAttribute('aria-checked', 'true');
  await expect(page.getByTestId('profile-no-permanent')).toHaveAttribute('aria-checked', 'false');
  // Nothing changed: nothing to save, and "Speichern" is the only primary of the view.
  await expect(save(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(discard(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(page.locator('.btn.primary')).toHaveCount(1);
});

test('one name per field: the labels, their hints and neutral examples', async ({ page }) => {
  await profile(page);
  const form = page.getByTestId('profile-form');
  for (const text of [
    'Die Rolle zählt für die Passung.',
    'Sie stützen die Passung, belegen aber keine Anforderung.',
    'Ab zehn Jahren bewertet die App Einstiegsstellen niedrig.',
    'Ohne Niveau rechnet die App mit B2.',
    'Remote-Anteil',
    'Den Mindest-Tagessatz legen die Ausschlusskriterien fest.',
    'Mindest-Tagessatz (€)',
    'Mindest-Erfahrung der Stelle (Jahre)',
    'Mindest-Jahresgehalt (€)',
    'Mindest-Remote-Anteil (%)',
  ]) {
    await expect(form).toContainText(text);
  }
  for (const gone of ['Auch genannt', 'Arbeitsort', 'Stellen ab so viel Erfahrung', 'Anderswo']) {
    await expect(form).not.toContainText(gone);
  }
  // The column heads explain themselves.
  const head = page.getByTestId('competences').locator('.head');
  expect(await tooltipOf(page, head.getByText('Andere Begriffe'))).toBe(
    'Synonyme oder englische Begriffe.',
  );
  // A level says what it means.
  const levels = page.getByTestId('language-row').first().getByTestId('language-level');
  expect(await tooltipOf(page, levels.getByRole('button', { name: 'C1' }))).toBe('Fließend');
  await expect(page.getByTestId('profile-wish-industries').locator('input')).toHaveAttribute(
    'placeholder',
    'Energie',
  );
});

test('edit and discard: the form goes back to what is stored', async ({ page }) => {
  await profile(page);
  const title = page.getByTestId('profile-title');
  await title.fill('Interim CFO');
  await expect(save(page)).not.toHaveAttribute('aria-disabled', 'true');
  await page.getByTestId('profile-countries').getByRole('button', { name: 'Schweiz' }).click();
  await discard(page).click();
  await expect(title).toHaveValue('Interim Managerin Finanzen');
  await expect(
    page.getByTestId('profile-countries').getByRole('button', { name: 'Schweiz' }),
  ).toHaveAttribute('aria-pressed', 'false');
  await expect(save(page)).toHaveAttribute('aria-disabled', 'true');
  expect(await calls(page, 'save_profile')).toHaveLength(0);
});

test('edit and save: both forms go to the backend, the change is confirmed', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-min-rate').fill('1.250');
  const keywords = page.getByTestId('profile-keywords').locator('input');
  await keywords.fill('Bilanzierung');
  await keywords.press('Enter');
  await page.getByTestId('profile-available').getByRole('button', { name: 'Ab Datum' }).click();
  await page.getByTestId('profile-date').fill('1.11.2026');
  await save(page).click();
  await expect(page.getByTestId('profile-saved')).toHaveText('Gespeichert, Jobs neu bewertet.');
  const sent = await lastSave(page);
  expect(sent.source).toBeNull();
  expect(sent.clear).toEqual([]);
  expect(sent.before.criteria.minDayRate).toBe(1100);
  expect(sent.after.criteria.minDayRate).toBe(1250);
  expect(sent.after.keywords).toEqual(['IFRS', 'HGB', 'Konzernabschluss', 'Bilanzierung']);
  expect(sent.after.criteria.available).toEqual({ kind: 'from', date: '2026-11-01' });
  // Saved: the form is the stored profile again.
  await expect(save(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(page.getByTestId('profile-saved-at')).toBeVisible();
  await expect(page.getByTestId('profile-date')).toHaveValue('01.11.2026');
});

test('the head and the first section keep the rhythm of all sections', async ({ page }) => {
  await profile(page);
  const head = (await page.getByTestId('profile-file').boundingBox())!;
  const person = (await page.getByTestId('section-person').boundingBox())!;
  const competences = (await page.getByTestId('section-competences').boundingBox())!;
  expect(person.y - (head.y + head.height)).toBe(competences.y - (person.y + person.height));
});

test('the save bar says what is unsaved and what was saved, once', async ({ page }) => {
  await profile(page);
  const status = page.getByTestId('profile-save-status');
  await expect(status).toHaveText('');
  await page.getByTestId('profile-title').fill('Interim CFO');
  await expect(status).toHaveText('Nicht gespeichert');
  await page.getByTestId('profile-save').click();
  await expect(status).toHaveText('Gespeichert, Jobs neu bewertet.');
  // No toast over the bar, and the result goes with the next change.
  await expect(page.getByTestId('toast')).toHaveCount(0);
  await page.getByTestId('profile-title').fill('Interim CFO und Controlling');
  await expect(status).toHaveText('Nicht gespeichert');
});

test('a focused field is never hidden under the save bar', async ({ page }) => {
  await profile(page);
  const bar = page.getByTestId('profile-save-bar');
  for (const id of ['competence-add', 'profile-strengths', 'profile-keywords']) {
    const target =
      id === 'competence-add' ? page.getByTestId(id) : page.getByTestId(id).locator('input');
    await target.focus();
    await expect
      .poll(async () => {
        const [box, barBox] = [await target.boundingBox(), await bar.boundingBox()];
        return box!.y + box!.height <= barBox!.y;
      }, id)
      .toBe(true);
  }
});

test('money is grouped like everywhere, number fields share one width', async ({ page }) => {
  await profile(page);
  const rate = page.getByTestId('profile-min-rate');
  await expect(rate).toHaveValue('1.100');
  // Typed plain, grouped again when the field is left.
  await rate.fill('1250');
  await expect(rate).toHaveValue('1250');
  await page.getByTestId('profile-name-field').focus();
  await expect(rate).toHaveValue('1.250');
  await expect(page.getByTestId('profile-years')).toHaveValue('20');
  const widths = await Promise.all(
    ['profile-years', 'profile-wish-rate', 'profile-min-rate', 'profile-target-years'].map(
      async (id) => (await page.getByTestId(id).boundingBox())!.width,
    ),
  );
  expect(new Set(widths).size).toBe(1);
});

test('money with cents counts whole euros and says so, never a hundred times more', async ({
  page,
}) => {
  await profile(page);
  const rate = page.getByTestId('profile-min-rate');
  await rate.fill('950,50');
  await expect(page.getByTestId('profile-min-rate-rounded')).toHaveText(
    'Auf ganze Euro abgerundet.',
  );
  await page.getByTestId('profile-name-field').focus();
  await expect(rate).toHaveValue('950');
  await expect(page.getByTestId('profile-min-rate-rounded')).toBeVisible();
  // A point works the same; a group of three digits stays a thousands separator.
  const wish = page.getByTestId('profile-wish-rate');
  await wish.fill('1.180.75');
  await page.getByTestId('profile-name-field').focus();
  await expect(wish).toHaveValue('1.180');
  await wish.fill('1,300');
  await expect(page.getByTestId('profile-wish-rate-rounded')).toHaveCount(0);
  await save(page).click();
  const sent = await lastSave(page);
  expect(sent.after.criteria.minDayRate).toBe(950);
  expect(sent.after.wishes.dayRate).toBe(1300);
});

test('narrow, a language row keeps its levels under the name', async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 600 });
  await profile(page);
  const row = page.getByTestId('language-row').first();
  const name = await row.getByTestId('language-name').boundingBox();
  const levels = await row.getByTestId('language-level').boundingBox();
  expect(levels!.y).toBeGreaterThanOrEqual(name!.y + name!.height);
  // The alias fields keep a word inside when the column header is gone.
  await expect(page.getByTestId('competence-aliases').first().locator('input')).toHaveAttribute(
    'placeholder',
    'Andere Begriffe',
  );
});

test('a wrong date is said at the field and nothing is saved', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-available').getByRole('button', { name: 'Ab Datum' }).click();
  await page.getByTestId('profile-date').fill('31.02.2026');
  await expect(page.getByTestId('profile-date-error')).toHaveText(
    'Gib das Datum im Format 01.11.2026 ein.',
  );
  await save(page).click();
  expect(await calls(page, 'save_profile')).toHaveLength(0);
  // Saving says why in the bar and puts the caret into the day.
  await expect(page.getByTestId('profile-save-status')).toHaveText(
    'Gib das Datum im Format 01.11.2026 ein.',
  );
  await expect(page.getByTestId('profile-date')).toBeFocused();
});

test('a value the backend refuses is said at its field, which gets the caret', async ({ page }) => {
  await profile(page);
  const rate = page.getByTestId('profile-min-rate');
  await rate.fill('250000');
  await save(page).click();
  const criteria = page.getByTestId('section-criteria');
  await expect(criteria).toContainText('Der Wert bei „Mindest-Tagessatz“ passt nicht.');
  await expect(rate).toHaveAttribute('aria-invalid', 'true');
  await expect(rate).toBeFocused();
  // The bar only says that nothing is saved; the reason is at the field.
  await expect(page.getByTestId('profile-save-status')).toHaveText('Nicht gespeichert');
  // The next change takes the mark away.
  await rate.fill('1200');
  await expect(rate).not.toHaveAttribute('aria-invalid', 'true');

  // A competence: its row is marked and gets the caret (counted without the empty rows).
  await page.getByTestId('competence-add').click();
  const years = page.getByTestId('competence-years');
  await years.nth(3).fill('80');
  await save(page).click();
  expect((await lastSave(page)).after.competences[3]!.years).toBe(80);
  await expect(page.getByTestId('competence-error')).toHaveText(
    'Der Wert bei „Kompetenzen“ passt nicht.',
  );
  const names = page.getByTestId('competence-name');
  await expect(names.nth(3)).toHaveAttribute('aria-invalid', 'true');
  await expect(names.nth(2)).not.toHaveAttribute('aria-invalid', 'true');
  await expect(names.nth(3)).toBeFocused();
});

test('leaving with unsaved changes asks once; cancel stays, discard leaves', async ({ page }) => {
  await profile(page);
  const name = page.getByTestId('profile-name-field');
  await name.fill('Erika Muster');
  await page.getByTestId('nav-jobs').click();
  const dialog = page.getByTestId('dialog-leave-profile');
  await expect(dialog).toContainText('Änderungen speichern?');
  await expect(dialog).toContainText('Die Änderungen am Profil sind nicht gespeichert.');
  await expect(dialog.getByRole('button')).toHaveText(['Speichern', 'Verwerfen', 'Abbrechen']);
  await dialog.getByRole('button', { name: 'Abbrechen' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTestId('view-profile')).toBeVisible();
  await expect(name).toHaveValue('Erika Muster');
  await page.getByTestId('nav-settings').click();
  await dialog.getByRole('button', { name: 'Verwerfen' }).click();
  await expect(page.getByTestId('view-settings')).toBeVisible();
  // Back in the profile: the stored value, no question on the next switch.
  await page.getByTestId('nav-profile').click();
  await expect(name).toHaveValue('Erika Beispiel');
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
});

test('leaving with changes can save first, then it leaves', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-name-field').fill('Erika Muster');
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('dialog-leave-profile').getByRole('button', { name: 'Speichern' }).click();
  await expect(page.getByTestId('view-settings')).toBeVisible();
  expect((await lastSave(page)).after.name).toBe('Erika Muster');
});

test('closing the window with unsaved changes asks; without any it closes at once', async ({
  page,
}) => {
  await profile(page);
  const closed = (): Promise<boolean> => page.evaluate(() => window.__harness.closed);
  const requestClose = (): Promise<void> => page.evaluate(() => window.__harness.requestClose());
  // Nothing unsaved: the backend knows, and the window closes without a question.
  await expect.poll(() => page.evaluate(() => window.__harness.unsaved)).toBe(false);
  await requestClose();
  expect(await closed()).toBe(true);
  await expect(page.getByTestId('dialog-leave-profile')).toHaveCount(0);
  await page.evaluate(() => (window.__harness.closed = false));

  // A change: the page tells the backend, and closing asks first.
  await page.getByTestId('profile-name-field').fill('Erika Muster');
  await expect.poll(() => page.evaluate(() => window.__harness.unsaved)).toBe(true);
  await requestClose();
  const dialog = page.getByTestId('dialog-leave-profile');
  await expect(dialog).toContainText('Änderungen speichern?');
  await dialog.getByRole('button', { name: 'Abbrechen' }).click();
  expect(await closed()).toBe(false);
  await expect(page.getByTestId('profile-name-field')).toHaveValue('Erika Muster');
  // Verwerfen: the change goes and the window closes.
  await requestClose();
  await dialog.getByRole('button', { name: 'Verwerfen' }).click();
  await expect.poll(closed).toBe(true);
  expect(await calls(page, 'close_window')).toHaveLength(1);
  expect(await calls(page, 'save_profile')).toHaveLength(0);
});

test('closing the window can save the changes first', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-title').fill('Interim CFO');
  await expect.poll(() => page.evaluate(() => window.__harness.unsaved)).toBe(true);
  await page.evaluate(() => window.__harness.requestClose());
  await page.getByTestId('dialog-leave-profile').getByRole('button', { name: 'Speichern' }).click();
  await expect.poll(() => page.evaluate(() => window.__harness.closed)).toBe(true);
  expect((await lastSave(page)).after.title).toBe('Interim CFO');
});

test('at most five Schwerpunkte: a sixth star is disabled and says why', async ({ page }) => {
  await profile(page);
  const stars = page.getByTestId('competence-star');
  await expect(page.getByTestId('focus-count')).toHaveText('Schwerpunkte 2 von 5');
  for (const index of [0, 3, 4]) await stars.nth(index).click();
  await expect(chips(page.getByTestId('focus'))).toHaveCount(5);
  await expect(page.getByTestId('focus-count')).toHaveText('Schwerpunkte 5 von 5');
  // The sixth star is off, its tooltip says why right at the pointer.
  await expect(stars.nth(5)).toHaveAttribute('aria-disabled', 'true');
  await stars.nth(5).hover();
  await expect(page.getByRole('tooltip')).toHaveText('Höchstens fünf Schwerpunkte.');
  await stars.nth(5).click({ force: true });
  await expect(stars.nth(5)).toHaveAttribute('aria-pressed', 'false');
  // Unstarring makes room; renaming a starred competence takes the Schwerpunkt along.
  await stars.nth(0).click();
  await expect(stars.nth(5)).not.toHaveAttribute('aria-disabled', 'true');
  await page.getByTestId('competence-name').nth(1).fill('Konzerncontrolling');
  await expect(chips(page.getByTestId('focus')).first()).toHaveText('Konzerncontrolling');
});

test('a file with seven Schwerpunkte: the first five are taken, saving works', async ({ page }) => {
  await profile(page, 'no-profile', '&file=focus');
  await page.getByTestId('profile-pick').click();
  await expect(chips(page.getByTestId('focus'))).toHaveCount(5);
  await expect(page.getByTestId('focus-trimmed')).toHaveText(
    'Die Datei nennt 7 Schwerpunkte, übernommen sind die ersten fünf.',
  );
  await expect(page.getByTestId('focus-count')).toHaveText('Schwerpunkte 5 von 5');
  await save(page).click();
  await expect(page.getByTestId('profile-saved')).toBeVisible();
  expect((await lastSave(page)).after.focus).toHaveLength(5);
});

test('chip field: Enter adds, a pasted list splits, x and Backspace remove, Esc drops', async ({
  page,
}) => {
  await profile(page);
  const field = page.getByTestId('profile-tools');
  const input = field.locator('input');
  await input.fill('Excel');
  await input.press('Enter');
  await expect(chips(field)).toHaveText(['SAP S/4HANA', 'LucaNet', 'Power BI', 'Excel']);
  // The same value in another case is not added twice; the typed text goes.
  await input.fill('excel');
  await input.press('Enter');
  await expect(chips(field)).toHaveCount(4);
  await expect(input).toHaveValue('');
  // A pasted list becomes one chip per entry.
  await input.focus();
  await paste(input, 'Tableau, Qlik\nJira');
  await expect(chips(field)).toHaveText([
    'SAP S/4HANA',
    'LucaNet',
    'Power BI',
    'Excel',
    'Tableau',
    'Qlik',
    'Jira',
  ]);
  // Backspace in the empty field removes the last chip, x removes any.
  await input.press('Backspace');
  await expect(chips(field)).toHaveCount(6);
  await field.getByRole('button', { name: 'LucaNet entfernen' }).click();
  await expect(chips(field)).toHaveText(['SAP S/4HANA', 'Power BI', 'Excel', 'Tableau', 'Qlik']);
  // Esc drops what was typed; leaving the field adds it.
  await input.fill('Visio');
  await input.press('Escape');
  await expect(input).toHaveValue('');
  await input.fill('Miro');
  await page.getByTestId('profile-name-field').click();
  await expect(chips(field).last()).toHaveText('Miro');
  // Enter never saves the long form; Ctrl+S (Cmd+S on macOS) does.
  await input.press('Enter');
  expect(await calls(page, 'save_profile')).toHaveLength(0);
  await input.press('Control+s');
  await expect(page.getByTestId('profile-saved')).toHaveText('Gespeichert, Jobs neu bewertet.');
  expect((await lastSave(page)).after.tools).toEqual([
    'SAP S/4HANA',
    'Power BI',
    'Excel',
    'Tableau',
    'Qlik',
    'Miro',
  ]);
});

test('sentences keep their commas; a double click takes a chip back to edit it', async ({
  page,
}) => {
  await profile(page);
  const strengths = page.getByTestId('profile-strengths');
  const input = strengths.locator('input');
  await input.fill('Verbindet Zahlen, Menschen und Wandel');
  await input.press('Enter');
  await expect(chips(strengths)).toHaveText([
    'Aufbau von Konzernreportings in weniger als 100 Tagen',
    'Verbindet Zahlen, Menschen und Wandel',
  ]);
  // Pasted, only line breaks split a list of sentences.
  await input.focus();
  await paste(input, 'Führt Teams, auch remote\nSpricht die Sprache der Werke');
  await expect(chips(strengths)).toHaveCount(4);
  await expect(chips(strengths).nth(2)).toHaveText('Führt Teams, auch remote');
  // Degrees and certificates keep their commas too; terms still split.
  const degrees = page.getByTestId('profile-degrees').locator('input');
  await degrees.fill('Master of Science, Wirtschaftsinformatik');
  await degrees.press('Enter');
  await expect(chips(page.getByTestId('profile-degrees')).last()).toHaveText(
    'Master of Science, Wirtschaftsinformatik',
  );
  // A double click on a chip puts its text back into the field, the rest stays.
  const tools = page.getByTestId('profile-tools');
  await chips(tools).nth(1).dblclick();
  await expect(tools.locator('input')).toBeFocused();
  await expect(tools.locator('input')).toHaveValue('LucaNet');
  await expect(chips(tools)).toHaveText(['SAP S/4HANA', 'Power BI']);
  await tools.locator('input').fill('LucaNet Financial');
  await tools.locator('input').press('Enter');
  await expect(chips(tools)).toHaveText(['SAP S/4HANA', 'Power BI', 'LucaNet Financial']);
});

test('Enter goes through the rows and never saves; on an empty last row it moves on', async ({
  page,
}) => {
  await profile(page);
  const names = page.getByTestId('competence-name');
  // A field outside the rows: Enter does nothing.
  await page.getByTestId('profile-name-field').press('Enter');
  // In a row: Enter goes to the next row.
  await names.nth(0).press('Enter');
  await expect(names.nth(1)).toBeFocused();
  // On the last row: Enter adds a row and puts the caret into it.
  await page.getByTestId('competence-years').nth(5).press('Enter');
  await expect(names).toHaveCount(7);
  await expect(names.nth(6)).toBeFocused();
  await names.nth(6).fill('Konzernabschluss');
  await page.getByTestId('competence-years').nth(6).fill('15');
  await page.getByTestId('competence-years').nth(6).press('Enter');
  await expect(names).toHaveCount(8);
  await expect(names.nth(7)).toBeFocused();
  // On an empty last row: the row goes and the caret moves on to the next field.
  await names.nth(7).press('Enter');
  await expect(names).toHaveCount(7);
  await expect(page.getByTestId('profile-strengths').locator('input')).toBeFocused();
  // Languages behave the same.
  const languages = page.getByTestId('language-name');
  await languages.nth(1).press('Enter');
  await expect(languages).toHaveCount(3);
  await expect(languages.nth(2)).toBeFocused();
  expect(await calls(page, 'save_profile')).toHaveLength(0);
  // Saving is Speichern or Ctrl+S.
  await languages.nth(2).fill('Spanisch');
  await languages.nth(2).press('Control+s');
  await expect(page.getByTestId('profile-saved')).toHaveText('Gespeichert, Jobs neu bewertet.');
  const sent = await lastSave(page);
  expect(sent.after.competences.map((row) => row.name)).toContain('Konzernabschluss');
  expect(sent.after.languages.map((row) => row.language)).toContain('Spanisch');
});

test('a new form starts with one row each; the add buttons are buttons', async ({ page }) => {
  await profile(page, 'no-profile');
  await page.getByTestId('profile-empty').getByRole('button', { name: 'Profil anlegen' }).click();
  // One empty row each: the table and the star show at once, with neutral examples.
  await expect(page.getByTestId('competence-name')).toHaveCount(1);
  await expect(page.getByTestId('language-name')).toHaveCount(1);
  await expect(page.getByTestId('competence-name')).toHaveAttribute(
    'placeholder',
    'Projektmanagement',
  );
  await expect(page.getByTestId('competence-aliases').locator('input')).toHaveAttribute(
    'placeholder',
    'Andere Begriffe',
  );
  await expect(page.getByTestId('profile-title')).toHaveAttribute(
    'placeholder',
    'Senior Consultant',
  );
  for (const id of ['competence-add', 'language-add']) {
    await expect(page.getByTestId(id)).toHaveClass(/secondary/);
    await expect(page.getByTestId(id).locator('svg')).toHaveCount(1);
  }
  // Without rows the add button starts at the edge of the card, not indented.
  await page.getByTestId('competence-remove').click();
  const [add, list] = [
    await page.getByTestId('competence-add').boundingBox(),
    await page.getByTestId('competences').boundingBox(),
  ];
  expect(Math.abs(add!.x - list!.x)).toBeLessThanOrEqual(1);
  // The other two ways in stay at hand in a new form; nothing is read yet.
  await expect(page.getByTestId('profile-from-cv')).toBeVisible();
  await expect(page.getByTestId('profile-pick')).toBeVisible();
  await expect(page.getByTestId('section-understood')).toHaveCount(0);
  await expect(badge(page)).toHaveCount(0);
});

test('no profile: one sentence and the three ways in', async ({ page }) => {
  await profile(page, 'no-profile');
  const empty = page.getByTestId('profile-empty');
  await expect(empty).toContainText('Noch kein Profil');
  await expect(empty).toContainText('Mit einem Profil zeigt jeder Job, wie gut er passt.');
  await expect(empty.getByRole('button')).toHaveText([
    'Profil anlegen',
    'Aus Lebenslauf erstellen',
    'Datei wählen',
  ]);
  await expect(empty.locator('.btn.primary')).toHaveText('Profil anlegen');
});

test('create from the empty form and save; the quality follows while typing', async ({ page }) => {
  await profile(page, 'no-profile');
  await page.getByTestId('profile-empty').getByRole('button', { name: 'Profil anlegen' }).click();
  await expect(page.getByTestId('profile-name')).toHaveText('Neues Profil');
  await expect(save(page)).toHaveAttribute('aria-disabled', 'true');
  await page.getByTestId('profile-name-field').fill('Erika Beispiel');
  // Typed, the form says how it reads: no competences yet, then little content.
  await expect(badge(page)).toHaveText('Ohne Kompetenzen');
  await expect(page.getByTestId('profile-save-status')).toHaveText('Nicht gespeichert');
  await page.getByTestId('competence-name').fill('Controlling');
  await page.getByTestId('competence-years').fill('18');
  await expect(badge(page)).toHaveText('Wenig Inhalt');
  await expect(page.getByTestId('section-competences')).toContainText(
    'Wenige Kompetenzen, die Passung bleibt grob.',
  );
  // An added row takes the caret; an empty one is not saved.
  await page.getByTestId('competence-add').click();
  await expect(page.getByTestId('competence-name').last()).toBeFocused();
  await page.getByTestId('language-name').last().fill('Englisch');
  await page.getByTestId('language-row').last().getByRole('button', { name: 'C1' }).click();
  await save(page).click();
  const sent = await lastSave(page);
  expect(sent.source).toBe('{}');
  expect(sent.after.competences).toEqual([
    { name: 'Controlling', years: 18, aliases: [], origin: null },
  ]);
  expect(sent.after.languages).toEqual([{ language: 'Englisch', level: 'c1', origin: null }]);
  // A new profile allows remote roles abroad, as the engine reads a missing key.
  expect(sent.after.criteria.remoteOutside).toBe(true);
  await expect(page.getByTestId('profile-name')).toHaveText('Erika Beispiel');
  await expect(save(page)).toHaveAttribute('aria-disabled', 'true');
});

test('a chosen file fills the form for review; discarding keeps what was there', async ({
  page,
}) => {
  await profile(page, 'no-profile');
  await page.getByTestId('profile-pick').click();
  await expect(page.getByTestId('profile-name')).toHaveText('Profil aus einer Datei');
  await expect(page.getByTestId('profile-review')).toHaveText(
    'Prüfe die Angaben und speichere sie.',
  );
  await expect(page.getByTestId('profile-name-field')).toHaveValue('Jonas Muster');
  // What does not read is said before saving, at its field.
  await expect(page.getByTestId('section-criteria')).toContainText(
    'In der Datei stand „ab 900“, das ist keine Zahl.',
  );
  await expect(badge(page)).toHaveText('Wenig Inhalt');
  // A draft is unsaved as it is: saving is possible at once, leaving asks.
  await expect(save(page)).not.toHaveAttribute('aria-disabled', 'true');
  await discard(page).click();
  await expect(page.getByTestId('profile-empty')).toBeVisible();
  expect(await calls(page, 'save_profile')).toHaveLength(0);

  await page.getByTestId('profile-pick').click();
  await save(page).click();
  expect((await lastSave(page)).source).toBe('{"name": "Jonas Muster"}');
  await expect(page.getByTestId('profile-name')).toHaveText('Jonas Muster');
});

const ANSWER = [
  'Gern, hier ist das Profil.',
  '',
  '```json',
  JSON.stringify(
    {
      name: 'Carla Exempel',
      titel: 'Interim CFO',
      berufserfahrung_jahre: 30,
      ausbildung: [{ abschluss: 'Diplom-Kauffrau (Univ.)' }],
      kernkompetenzen: [
        { kompetenz: 'Controlling', jahre: 28, auch: ['FP&A'] },
        { kompetenz: 'Treasury', jahre: 15, auch: [] },
      ],
      schwerpunkte: ['Controlling'],
      methoden_tools: [{ name: 'SAP S/4HANA' }],
      zertifizierungen: [],
      branchen: [{ branche: 'Chemie' }],
      sprachen: [{ sprache: 'Englisch', niveau: 'C1' }],
      alleinstellungsmerkmale: [],
      keywords: ['IFRS'],
    },
    null,
    2,
  ),
  '```',
].join('\n');

test('from a CV: the request is copied, the pasted answer fills the form', async ({
  page,
  browserName,
}) => {
  if (browserName === 'chromium') {
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  }
  await profile(page, 'no-profile');
  await page
    .getByTestId('profile-empty')
    .getByRole('button', { name: 'Aus Lebenslauf erstellen' })
    .click();
  const card = page.getByTestId('profile-paste');
  await expect(card).toBeVisible();
  // Where the CV goes, in one sentence; the prompt can be read before it goes out.
  await expect(page.getByTestId('paste-privacy')).toHaveText(
    'Der Lebenslauf geht an die KI, die du nutzt.',
  );
  await expect(page.getByTestId('paste-prompt')).toHaveCount(0);
  await page.getByTestId('paste-preview').getByRole('button', { name: 'Prompt ansehen' }).click();
  await expect(page.getByTestId('paste-prompt')).toContainText('Lebenslauf');
  await expect(card).toContainText('Füge ihn in eine KI ein und hänge den Lebenslauf an.');
  // The same words as the rest of the app: KI and Prompt, never Claude or Anfrage.
  await expect(card).not.toContainText('Claude');
  await expect(card).not.toContainText('Anfrage');
  if (browserName === 'chromium') {
    await expect(page.getByTestId('paste-copied')).toContainText('Der Prompt ist kopiert.');
    await expect(page.getByTestId('paste-copy')).toHaveText('Erneut kopieren');
    const copied = await page.evaluate(() => navigator.clipboard.readText());
    expect(copied).toContain('Lebenslauf');
  }
  const take = page.getByTestId('paste-take');
  await expect(take).toHaveAttribute('aria-disabled', 'true');
  await expect(card).toContainText('Antwort der KI');
  await page.getByTestId('paste-answer').fill('Das kann ich leider nicht.');
  await take.click();
  await expect(card).toContainText('In der Antwort steht kein Profil.');
  await page.getByTestId('paste-answer').fill(ANSWER);
  await take.click();
  await expect(page.getByTestId('profile-name')).toHaveText('Profil aus dem Lebenslauf');
  await expect(page.getByTestId('profile-name-field')).toHaveValue('Carla Exempel');
  await expect(page.getByTestId('competence-name')).toHaveCount(2);
  await expect(chips(page.getByTestId('focus'))).toHaveText(['Controlling']);
  expect((await calls(page, 'parse_profile')).at(-1)![1]).toEqual({ text: ANSWER });
  // Criteria are the user's own: the form asks for them, the answer brings none.
  await expect(page.getByTestId('profile-min-rate')).toHaveValue('');
  await save(page).click();
  expect((await lastSave(page)).after.name).toBe('Carla Exempel');
});

test('from a CV for the stored profile: the answer updates it for review', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-update-cv').click();
  await expect(page.getByTestId('profile-paste')).toContainText('Aus Lebenslauf aktualisieren');
  await page.getByTestId('paste-answer').fill(ANSWER);
  await page.getByTestId('paste-take').click();
  await expect(page.getByTestId('profile-name')).toHaveText(
    'Profil mit dem Lebenslauf aktualisiert',
  );
  await expect(page.getByTestId('profile-review')).toBeVisible();
  // The person and the user's own choices stay; the answer adds and fills.
  await expect(page.getByTestId('profile-name-field')).toHaveValue('Erika Beispiel');
  await expect(page.getByTestId('profile-title')).toHaveValue('Interim CFO');
  await expect(page.getByTestId('competence-name')).toHaveCount(7);
  await expect(page.getByTestId('competence-name').last()).toHaveValue('Treasury');
  await expect(page.getByTestId('profile-min-rate')).toHaveValue('1.100');
  await expect(chips(page.getByTestId('focus'))).toHaveText([
    'Controlling',
    'Konzernrechnungslegung nach IFRS',
  ]);
  // Nothing is saved by itself; saving writes into the stored profile.
  expect(await calls(page, 'save_profile')).toHaveLength(0);
  await save(page).click();
  const sent = await lastSave(page);
  expect(sent.source).toBeNull();
  const controlling = sent.after.competences.find((row) => row.name === 'Controlling')!;
  expect(controlling.years).toBe(28);
  expect(controlling.aliases).toEqual(['Financial Controlling', 'FP&A']);
  expect(controlling.origin).toBe(1);
  expect(sent.after.years).toBe(30);
});

test('from a CV: when the prompt could not be copied, the step says so and copies', async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: () => Promise.reject(new Error('denied')) },
    });
  });
  await profile(page, 'no-profile');
  await page
    .getByTestId('profile-empty')
    .getByRole('button', { name: 'Aus Lebenslauf erstellen' })
    .click();
  await expect(page.getByTestId('paste-copied')).toContainText(
    'Der Prompt ließ sich nicht kopieren.',
  );
  await expect(page.getByTestId('paste-copy')).toHaveText('Prompt kopieren');
});

test('a thin profile marks its empty sections, and they follow the form', async ({ page }) => {
  await profile(page, 'profile-thin');
  await expect(badge(page)).toHaveText('Wenig Inhalt');
  // The quality is said once, where it helps: at the competences.
  await expect(page.getByTestId('section-competences')).toContainText(
    'Wenige Kompetenzen, die Passung bleibt grob.',
  );
  await expect(page.getByText('Wenige Kompetenzen, die Passung bleibt grob.')).toHaveCount(1);
  await expect(page.getByText('Das Profil nennt nur wenige Kompetenzen.')).toHaveCount(0);
  const experience = page.getByTestId('section-experience');
  await expect(experience).toContainText('Noch leer');
  await expect(page.getByTestId('section-competences')).not.toContainText('Noch leer');
  // Filled while typing, the section is no longer empty; enough terms and it is complete.
  await page.getByTestId('profile-years').fill('12');
  await expect(experience).not.toContainText('Noch leer');
  const tools = page.getByTestId('profile-tools').locator('input');
  await tools.fill('SAP, Excel');
  await tools.press('Enter');
  await expect(badge(page)).toHaveText('Vollständig');
  await expect(page.getByTestId('section-competences')).not.toContainText(
    'Wenige Kompetenzen, die Passung bleibt grob.',
  );
});

test('a profile edited into broken JSON says so and where, with its folder', async ({ page }) => {
  await profile(page, 'profile-broken');
  const empty = page.getByTestId('profile-empty');
  await expect(empty).toContainText('Profil nicht lesbar');
  await expect(empty).toContainText(
    'Die Datei ist kein gültiges JSON, Zeile 12. Ein neues Profil ersetzt die Datei.',
  );
  await empty.getByTestId('profile-folder').click();
  expect((await calls(page, 'open_target')).at(-1)![1]).toEqual({
    target: { kind: 'profileDir' },
  });
});

test('the head offers the folder, an update from a CV and remove', async ({ page }) => {
  await profile(page);
  const head = page.getByTestId('profile-file');
  await expect(head.getByRole('button')).toHaveText([
    'Andere Datei wählen',
    'Aus Lebenslauf aktualisieren',
    'Ordner öffnen',
    'Entfernen',
  ]);
  await page.getByTestId('profile-folder').click();
  expect((await calls(page, 'open_target')).at(-1)![1]).toEqual({
    target: { kind: 'profileDir' },
  });
  // With changes another file or an answer would replace them: the buttons wait.
  await page.getByTestId('profile-title').fill('CFO');
  await expect(page.getByTestId('profile-update-cv')).toHaveAttribute('aria-disabled', 'true');
});

test('remove asks first, says what happens and can be taken back', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-remove').click();
  const dialog = page.getByTestId('dialog-remove-profile');
  await expect(dialog).toContainText(
    'Die Jobs zeigen danach keine Passung mehr. Die Datei bleibt als Sicherung im Profilordner.',
  );
  await dialog.getByRole('button', { name: 'Entfernen' }).click();
  await expect(page.getByTestId('profile-empty')).toBeVisible();
  await expect(dialog).toHaveCount(0);
  expect(await calls(page, 'remove_profile')).toHaveLength(1);
  // A moment to take it back.
  const toast = page.getByTestId('toast');
  await expect(toast).toContainText('Profil entfernt.');
  await toast.getByRole('button', { name: 'Rückgängig' }).click();
  await expect(page.getByTestId('profile-name')).toHaveText('Erika Beispiel');
  expect(await calls(page, 'restore_profile')).toHaveLength(1);
});

test('every value that does not read is said at its field and can be removed', async ({ page }) => {
  await profile(page, 'profile-unreadable');
  await expect(badge(page)).toHaveText('Etwas prüfen');
  const reasons = await tooltipOf(page, badge(page).locator('.badge'));
  expect(reasons).toContain('„Mindest-Tagessatz“ ist nicht lesbar.');
  expect(reasons).toContain('„Treasury“ steht nicht bei den Kompetenzen.');
  // A key the app does not read at all is named in the head, as it is written in the file.
  await expect(page.getByTestId('profile-warning')).toHaveText(
    'Die App liest „tagessatz_max“ in den Ausschlusskriterien nicht.',
  );
  const form = page.getByTestId('profile-form');
  for (const text of [
    'In der Datei stand „teuer“, das ist keine Zahl.',
    'In der Datei stand „Deutschland“, das kann die App nicht lesen.',
    'In der Datei stand „5“, das kann die App nicht lesen.',
    'In der Datei stand „vielleicht“, das kann die App nicht lesen.',
    'In der Datei stand „senior“, das ist keine Zahl.',
    'In der Datei stand „hoch“, das ist keine Zahl.',
    'In der Datei stand „[]“, das kann die App nicht lesen.',
    'In der Datei stand „bald“, das ist kein Datum.',
    '„Treasury“ steht nicht bei den Kompetenzen.',
    '„Head of“ nennt kein Fachgebiet.',
    'In der Datei stand „egal“, das kann die App nicht lesen.',
    'In der Datei stand „{}“, das kann die App nicht lesen.',
  ]) {
    await expect(form).toContainText(text);
  }
  // One per field: fifteen fields, each with "Wert entfernen".
  const removes = form.getByTestId('value-remove');
  await expect(removes).toHaveCount(15);
  // A Schwerpunkt and a target role that do not count go from their list at once.
  await page.getByTestId('focus-unread').getByTestId('value-remove').click();
  await expect(chips(page.getByTestId('focus'))).toHaveText(['Controlling']);
  await page.getByTestId('roles-unread').getByTestId('value-remove').click();
  await expect(chips(page.getByTestId('profile-roles'))).toHaveText(['Interim CFO']);
  // A new value fixes a field as well.
  await page.getByTestId('profile-min-rate').fill('1000');
  await expect(form).not.toContainText('In der Datei stand „teuer“, das ist keine Zahl.');
  // The others go with "Wert entfernen"; then nothing is left to check.
  while ((await removes.count()) > 0) await removes.first().click();
  await expect(badge(page)).toHaveText('Vollständig');
  await save(page).click();
  const sent = await lastSave(page);
  expect([...sent.clear].sort()).toEqual(
    [
      'available',
      'contracts',
      'countries',
      'minSalary',
      'permanentPlaces',
      'permanentRemoteMin',
      'regions',
      'remote',
      'remoteOutside',
      'targetYears',
      'wishDayRate',
      'wishIndustries',
    ].sort(),
  );
  expect(sent.after.focus).toEqual(['Controlling']);
  expect(sent.after.roles).toEqual(['Interim CFO']);
  expect(sent.after.criteria.minDayRate).toBe(1000);
});

test('the remote switch sits under the countries and needs one', async ({ page }) => {
  await profile(page);
  const countries = page.getByTestId('profile-countries');
  const toggle = page.getByTestId('profile-remote-outside');
  // Directly under the countries, before the contract types.
  const box = async (node: Locator): Promise<{ y: number }> => (await node.boundingBox())!;
  const countriesBox = await box(countries);
  const toggleBox = await box(toggle);
  const anueBox = await box(page.getByTestId('profile-no-anue'));
  expect(toggleBox.y).toBeGreaterThan(countriesBox.y);
  expect(toggleBox.y).toBeLessThan(anueBox.y);
  await expect(page.getByTestId('section-criteria')).toContainText(
    'Ausgeschaltet markiert die App ganz remote Stellen mit Sitz im Ausland zum Prüfen.',
  );
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  // Without countries it has nothing to do: disabled, its tooltip says why.
  for (const name of ['Deutschland', 'Österreich']) {
    await countries.getByRole('button', { name }).click();
  }
  await expect(toggle).toHaveAttribute('aria-disabled', 'true');
  await toggle.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Wähle erst die Einsatzländer.');
});

test('permanent roles can be excluded next to temporary agency work', async ({ page }) => {
  await profile(page);
  const permanent = page.getByTestId('profile-no-permanent');
  await expect(page.getByTestId('section-criteria')).toContainText(
    'Nur bei klarem Wortlaut, sonst markiert die App den Job zum Prüfen.',
  );
  await expect(page.getByTestId('profile-permanent')).toBeVisible();
  await permanent.click();
  await expect(permanent).toHaveAttribute('aria-checked', 'true');
  // Excluded, the rules for permanent roles have nothing left to do.
  await expect(page.getByTestId('profile-permanent')).toHaveCount(0);
  await expect(page.getByTestId('profile-min-salary')).toHaveCount(0);
  await save(page).click();
  const sent = await lastSave(page);
  expect(sent.after.criteria.noPermanent).toBe(true);
  expect(sent.after.criteria.noAnue).toBe(true);
});

test('availability is a block of its own that only marks', async ({ page }) => {
  await profile(page);
  const block = page.getByTestId('section-availability');
  await expect(block).toContainText('Verfügbarkeit');
  await expect(block).toContainText(
    'Beginnt ein Job früher, markiert die App ihn zum Prüfen, sie schließt ihn nicht aus.',
  );
  await expect(block.getByTestId('profile-available')).toBeVisible();
  await expect(page.getByTestId('section-criteria').getByTestId('profile-available')).toHaveCount(
    0,
  );
});

test('how the app reads the profile: closed at first, then the terms and where they come from', async ({
  page,
}) => {
  await profile(page);
  const reading = page.getByTestId('profile-reading');
  await expect(page.getByTestId('reading-terms')).toHaveCount(0);
  await reading.getByRole('button', { name: 'So liest die App dein Profil' }).click();
  await expect(page.getByTestId('reading-terms')).toHaveText('42 Begriffe zählen für die Passung.');
  await expect(page.getByTestId('reading-list')).toContainText('Konzernabschluss nach HGB');
  await expect(page.getByTestId('reading-list')).toContainText('und 30 weitere');
  // Also what only the file holds, which explains the count.
  await expect(page.getByTestId('reading-sources')).toContainText('Kompetenzen 6');
  await expect(page.getByTestId('reading-sources')).toContainText('Stationen 27, nur in der Datei');
  await expect(page.getByTestId('reading-criteria')).toContainText('Tagessatz ab 1.100 €');
  await expect(page.getByTestId('reading-criteria')).toContainText(
    'Einsatzländer Deutschland, Österreich',
  );
  // With changes it says that it reads the saved profile.
  await page.getByTestId('profile-title').fill('CFO');
  await expect(reading).toContainText('Das gilt für den gespeicherten Stand.');
});

test('baseline: profile', async ({ page }) => {
  await profile(page);
  await settle(page);
  await expectShot(page, 'profile');
});

test('baseline: no profile yet', async ({ page }) => {
  await profile(page, 'no-profile');
  await expectShot(page, 'profile-empty');
});

test('baseline: from a CV', async ({ page, browserName }) => {
  if (browserName === 'chromium') {
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  }
  await profile(page, 'no-profile');
  await page
    .getByTestId('profile-empty')
    .getByRole('button', { name: 'Aus Lebenslauf erstellen' })
    .click();
  await expect(page.getByTestId('profile-paste')).toBeVisible();
  await page.getByTestId('paste-answer').fill(ANSWER.slice(0, 120));
  await expectShot(page, 'profile-paste');
});
