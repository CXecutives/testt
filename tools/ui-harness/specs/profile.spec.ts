// The Profil view against the stub: the profile as a form, the three ways in, saving,
// discarding, leaving with unsaved changes, Claude's answer and the chip fields.

import type { Locator, Page } from '@playwright/test';
import { calls, expect, expectShot, open, settle, test } from './fixtures';
import type { ProfileSave } from '../../../ui/src/lib/ipc/types';

async function profile(page: Page, scenario = 'default'): Promise<void> {
  await open(page, `?platform=windows&scenario=${scenario}`);
  await page.getByTestId('nav-profile').click();
  await expect(page.getByTestId('profile')).toBeVisible();
}

const save = (page: Page): Locator => page.getByTestId('profile-save');
const discard = (page: Page): Locator => page.getByTestId('profile-discard');
const chips = (field: Locator): Locator => field.locator('.chip .text');

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

test('the profile is a form, filled from the stored profile', async ({ page }) => {
  await profile(page);
  // The person first, the time of the last save; the file name only as the tooltip.
  await expect(page.getByTestId('profile-name')).toHaveText('Erika Beispiel');
  await expect(page.getByTestId('profile-role')).toHaveText('Interim Managerin Finanzen');
  await expect(page.getByTestId('profile-saved-at')).toHaveText('Gespeichert 21.09.2026, 09:30');
  const head = page.getByTestId('profile-file');
  await expect(head).not.toContainText('KB');
  // Well filled, but something to check: no "Vollständig" next to a value that did not read.
  await expect(head).toContainText('Bitte prüfen');
  await expect(head).not.toContainText('Vollständig');
  // What the app understood: terms for the match, never "Kompetenzen" against 6 rows.
  await expect(page.getByTestId('profile-understood')).toHaveText(
    '42 Begriffe für die Passung · 2 Schwerpunkte · Fachgebiete Finanzen, SAP',
  );
  // The value the app could not read is said at its field, not in the head.
  await expect(page.getByTestId('profile-warning')).toHaveCount(0);
  await expect(page.getByTestId('section-criteria')).toContainText(
    'In der Datei stand „viel“, das ist keine Zahl.',
  );
  for (const section of [
    'person',
    'competences',
    'experience',
    'tools',
    'languages',
    'wishes',
    'criteria',
  ]) {
    await expect(page.getByTestId(`section-${section}`)).toBeVisible();
  }
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
  // Schwerpunkte are starred, and listed under the competences.
  await expect(rows.nth(1).getByTestId('competence-star')).toHaveAttribute('aria-pressed', 'true');
  await expect(rows.nth(0).getByTestId('competence-star')).toHaveAttribute('aria-pressed', 'false');
  await expect(chips(page.getByTestId('focus'))).toHaveText([
    'Controlling',
    'Konzernrechnungslegung nach IFRS',
  ]);
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
  // Nothing changed: nothing to save, and "Speichern" is the only primary of the view.
  await expect(save(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(discard(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(page.locator('.btn.primary')).toHaveCount(1);
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
  expect(sent.before.criteria.minDayRate).toBe(1100);
  expect(sent.after.criteria.minDayRate).toBe(1250);
  expect(sent.after.keywords).toEqual(['IFRS', 'HGB', 'Konzernabschluss', 'Bilanzierung']);
  expect(sent.after.criteria.available).toEqual({ kind: 'from', date: '2026-11-01' });
  // Saved: the form is the stored profile again.
  await expect(save(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(page.getByTestId('profile-saved-at')).toBeVisible();
  await expect(page.getByTestId('profile-date')).toHaveValue('01.11.2026');
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
    'Auch genannt',
  );
});

test('a wrong date is said at the field and nothing is saved', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-available').getByRole('button', { name: 'Ab Datum' }).click();
  await page.getByTestId('profile-date').fill('31.02.2026');
  await expect(page.getByTestId('profile-date-error')).toHaveText(
    'Datum im Format 01.11.2026 eingeben.',
  );
  await save(page).click();
  expect(await calls(page, 'save_profile')).toHaveLength(0);
  // Saving says why in the bar and puts the caret into the day.
  await expect(page.getByTestId('profile-save-status')).toHaveText(
    'Datum im Format 01.11.2026 eingeben.',
  );
  await expect(page.getByTestId('profile-date')).toBeFocused();
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
    'Auch genannt',
  );
  await expect(page.getByTestId('profile-title')).toHaveAttribute('placeholder', 'Projektleitung');
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
  // The other two ways in stay at hand in a new form.
  await expect(page.getByTestId('profile-from-cv')).toBeVisible();
  await expect(page.getByTestId('profile-pick')).toBeVisible();
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

test('create from the empty form and save', async ({ page }) => {
  await profile(page, 'no-profile');
  await page.getByTestId('profile-empty').getByRole('button', { name: 'Profil anlegen' }).click();
  await expect(page.getByTestId('profile-name')).toHaveText('Neues Profil');
  await expect(page.getByTestId('profile-file')).toContainText('Noch nicht gespeichert');
  await expect(save(page)).toHaveAttribute('aria-disabled', 'true');
  await page.getByTestId('profile-name-field').fill('Erika Beispiel');
  await page.getByTestId('competence-name').fill('Controlling');
  await page.getByTestId('competence-years').fill('18');
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
    'Die Angaben prüfen, dann speichern.',
  );
  await expect(page.getByTestId('profile-name-field')).toHaveValue('Jonas Muster');
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
  await expect(card).toContainText('In Claude einfügen und den Lebenslauf anhängen.');
  if (browserName === 'chromium') {
    await expect(page.getByTestId('paste-copied')).toContainText(
      'Die Anfrage für Claude ist kopiert.',
    );
    const copied = await page.evaluate(() => navigator.clipboard.readText());
    expect(copied).toContain('Lebenslauf');
  }
  const take = page.getByTestId('paste-take');
  await expect(take).toHaveAttribute('aria-disabled', 'true');
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

test('a thin profile marks its empty sections', async ({ page }) => {
  await profile(page, 'profile-thin');
  await expect(page.getByTestId('profile-file')).toContainText('Wenig Inhalt');
  // The quality is said once, where it helps: at the competences.
  await expect(page.getByTestId('section-competences')).toContainText(
    'Wenige Kompetenzen, die Passung bleibt grob.',
  );
  await expect(page.getByText('Wenige Kompetenzen, die Passung bleibt grob.')).toHaveCount(1);
  await expect(page.getByText('Das Profil nennt nur wenige Kompetenzen.')).toHaveCount(0);
  for (const section of ['tools', 'languages']) {
    await expect(page.getByTestId(`section-${section}`)).toContainText('Noch leer');
  }
  await expect(page.getByTestId('section-experience')).not.toContainText('Noch leer');
  await expect(page.getByTestId('section-competences')).not.toContainText('Noch leer');
});

test('a profile edited into broken JSON says so and where', async ({ page }) => {
  await profile(page, 'profile-broken');
  const empty = page.getByTestId('profile-empty');
  await expect(empty).toContainText('Profil nicht lesbar');
  await expect(empty).toContainText(
    'Die Datei ist kein gültiges JSON, Zeile 12. Ein neues Profil ersetzt die Datei.',
  );
});

test('remove asks first', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-remove').click();
  await page
    .getByTestId('dialog-remove-profile')
    .getByRole('button', { name: 'Entfernen' })
    .click();
  await expect(page.getByTestId('profile-empty')).toBeVisible();
  await expect(page.getByTestId('dialog-remove-profile')).toHaveCount(0);
  expect(await calls(page, 'remove_profile')).toHaveLength(1);
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
