// Final round, team "switches and profile": only the switch switches (Einstellungen, Profil,
// the gallery's row), the profile form in its logical order with one control height per
// block, the countries as a field that suggests the engine's countries, the day of "Ab
// Datum" only while it is chosen, neutral examples, and the mailbox address to copy.

import type { Locator, Page } from '@playwright/test';
import type { ProfileSave } from '../../../ui/src/lib/ipc/types';
import { calls, expect, open, test } from './fixtures';

const WIN = '?platform=windows';

async function profile(page: Page, query = `${WIN}&scenario=default`): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-profile').click();
  await expect(page.getByTestId('profile-form')).toBeVisible();
}

const chips = (field: Locator): Locator => field.locator('.chip .text');
const countries = (page: Page): Locator => page.getByTestId('profile-countries');
const countryInput = (page: Page): Locator => countries(page).locator('input');
const options = (page: Page): Locator => page.getByTestId('profile-countries-options');

async function lastSave(page: Page): Promise<ProfileSave> {
  const all = await calls(page, 'save_profile');
  return (all.at(-1)![1] as { save: ProfileSave }).save;
}

test('profile switches: only the switch switches, its text names and describes it', async ({
  page,
}) => {
  await profile(page);
  const criteria = page.getByTestId('section-criteria');
  const cases = [
    {
      id: 'profile-remote-outside',
      label: 'Remote-Stellen im Ausland zulassen',
      hint: 'Ausgeschaltet markiert die App ganz remote Stellen mit Sitz im Ausland zum Prüfen.',
    },
    { id: 'profile-no-anue', label: 'Arbeitnehmerüberlassung ausschließen', hint: null },
    {
      id: 'profile-no-permanent',
      label: 'Festanstellung ausschließen',
      hint: 'Nur bei klarem Wortlaut, sonst markiert die App den Job zum Prüfen.',
    },
  ];
  for (const { id, label, hint } of cases) {
    const toggle = page.getByTestId(id);
    const before = await toggle.getAttribute('aria-checked');
    await criteria.getByText(label, { exact: true }).click();
    if (hint) await criteria.getByText(hint, { exact: true }).click();
    await expect(toggle, `${id} from its text`).toHaveAttribute('aria-checked', before!);
    await expect(toggle).toHaveAccessibleName(label);
    if (hint) await expect(toggle).toHaveAccessibleDescription(hint);
    await toggle.click();
    await expect(toggle, `${id} itself`).not.toHaveAttribute('aria-checked', before!);
  }
  // No text of the form is a label of a switch.
  const switchIds = await criteria
    .getByRole('switch')
    .evaluateAll((nodes) => nodes.map((node) => node.id));
  for (const id of switchIds) await expect(page.locator(`label[for="${id}"]`)).toHaveCount(0);
});

test('the profile in its logical order, each block with its sentence', async ({ page }) => {
  await profile(page);
  const sections = await page
    .locator('[data-testid^="section-"]')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-testid')));
  expect(sections).toEqual([
    'section-person',
    'section-competences',
    'section-experience',
    'section-languages',
    'section-wishes',
    'section-criteria',
    'section-availability',
    'section-understood',
  ]);
  const sentences: [string, string][] = [
    ['section-person', 'Die Rolle zählt für die Passung.'],
    ['section-competences', 'Nur dieser Block ist nötig, danach bewertet die App jeden Job.'],
    ['section-experience', 'Damit prüft die App, was eine Anzeige verlangt.'],
    ['section-languages', 'Die App vergleicht sie mit den Sprachen einer Anzeige.'],
    ['section-wishes', 'Wünsche verschieben die Bewertung leicht, sie schließen nichts aus.'],
    ['section-criteria', 'Ein Job, der hier nicht passt, gilt als ausgeschlossen.'],
  ];
  for (const [id, sentence] of sentences) {
    await expect(page.getByTestId(id).locator('.hint').first()).toHaveText(sentence);
  }
  // Said once: no other block claims to be needed.
  await expect(page.getByTestId('profile-form').getByText(/nötig/)).toHaveCount(1);
  // The languages have a block of their own, out of the experience.
  await expect(page.getByTestId('section-languages').getByTestId('languages')).toHaveCount(1);
  await expect(page.getByTestId('section-experience').getByTestId('languages')).toHaveCount(0);
});

test('every control of a block has the height of a field', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-available').getByRole('button', { name: 'Ab Datum' }).click();
  const heights = await page.locator('[data-testid^="section-"]').evaluateAll((sections) =>
    Object.fromEntries(
      sections.map((section) => {
        // The box of every field (a chip field by its one-line height: it grows with its
        // chips) and every toggle button of a choice.
        const fields = [...section.querySelectorAll<HTMLElement>('input[type="text"]')].map(
          (input) => {
            const box = input.closest<HTMLElement>('.field')!;
            return box.matches('.entry')
              ? parseFloat(getComputedStyle(box).minHeight)
              : Math.round(box.getBoundingClientRect().height);
          },
        );
        const choices = [...section.querySelectorAll<HTMLElement>('[role="group"] .btn')].map(
          (button) => Math.round(button.getBoundingClientRect().height),
        );
        return [section.getAttribute('data-testid'), [...new Set([...fields, ...choices])]];
      }),
    ),
  );
  for (const [section, set] of Object.entries(heights)) {
    if (section === 'section-understood') continue;
    expect(set, section).toEqual([36]);
  }
  // The choice buttons take the small type of chips and segments, not the larger button type.
  const types = await page
    .locator('[data-testid^="section-"] [role="group"] .btn')
    .evaluateAll((buttons) => [...new Set(buttons.map((b) => getComputedStyle(b).fontSize))]);
  expect(types).toEqual(['13px']);
  // Every number field has one width; the day of "Ab Datum" too.
  const widths = await Promise.all(
    [
      'profile-years',
      'profile-wish-rate',
      'profile-min-rate',
      'profile-target-years',
      'profile-min-salary',
      'profile-remote-min',
      'profile-date',
    ].map(async (id) => Math.round((await page.getByTestId(id).boundingBox())!.width)),
  );
  expect(new Set(widths).size, widths.join(' ')).toBe(1);
});

test('the countries: search in both languages, Enter or a click, chips, DACH in one click', async ({
  page,
}) => {
  await profile(page);
  await expect(chips(countries(page))).toHaveText(['Deutschland', 'Österreich']);
  const input = countryInput(page);
  // German names, and any word of them: the chosen ones are not offered again.
  await input.fill('schw');
  await expect(options(page).getByRole('option')).toHaveText(['Schweden', 'Schweiz']);
  await expect(input).toHaveAttribute('aria-expanded', 'true');
  // Enter takes the first one.
  await input.press('Enter');
  await expect(chips(countries(page))).toHaveText(['Deutschland', 'Österreich', 'Schweden']);
  await expect(input).toHaveValue('');
  await expect(options(page)).toBeHidden();
  // English names find the country too, named in the app's language; a click takes it.
  await input.fill('nether');
  await expect(options(page).getByRole('option')).toHaveText(['Niederlande']);
  await options(page).getByRole('option', { name: 'Niederlande' }).click();
  await expect(input).toBeFocused();
  // Accents do not matter.
  await input.fill('dane');
  await expect(options(page).getByRole('option')).toHaveText(['Dänemark']);
  await input.press('Escape');
  await expect(input).toHaveValue('');
  // A chip goes with its x; DACH brings back what is missing of the three.
  await countries(page).getByRole('button', { name: 'Österreich entfernen' }).click();
  await expect(chips(countries(page))).toHaveText(['Deutschland', 'Schweden', 'Niederlande']);
  await page.getByTestId('profile-dach').click();
  await expect(chips(countries(page))).toHaveText([
    'Deutschland',
    'Schweden',
    'Niederlande',
    'Österreich',
    'Schweiz',
  ]);
  // All three chosen: the quick pick has nothing left to do.
  await expect(page.getByTestId('profile-dach')).toHaveCount(0);
  // The remote switch stands directly under the countries.
  const field = (await countries(page).boundingBox())!;
  const toggle = (await page.getByTestId('profile-remote-outside').boundingBox())!;
  const anue = (await page.getByTestId('profile-no-anue').boundingBox())!;
  expect(toggle.y).toBeGreaterThan(field.y + field.height);
  expect(toggle.y).toBeLessThan(anue.y);
  await page.getByTestId('profile-save').click();
  expect((await lastSave(page)).after.criteria.countries).toEqual(['DE', 'SE', 'NL', 'AT', 'CH']);
});

test('the countries: text that names no country says so and is not taken', async ({ page }) => {
  await profile(page);
  const input = countryInput(page);
  await input.fill('Atlantis');
  await expect(page.getByTestId('profile-countries-none')).toHaveText(
    'Kein Land mit diesem Namen.',
  );
  await expect(options(page)).toBeHidden();
  await input.press('Enter');
  await page.getByTestId('profile-name-field').focus();
  await expect(chips(countries(page))).toHaveText(['Deutschland', 'Österreich']);
  await expect(input).toHaveValue('Atlantis');
  // Leaving the field with a single match takes it.
  await input.fill('Ital');
  await page.getByTestId('profile-name-field').focus();
  await expect(chips(countries(page))).toHaveText(['Deutschland', 'Österreich', 'Italien']);
  await expect(page.getByTestId('profile-countries-none')).toHaveCount(0);
});

test('the countries in English: English names, German ones still find them', async ({ page }) => {
  await profile(page, `${WIN}&scenario=default&lang=en`);
  await expect(chips(countries(page))).toHaveText(['Germany', 'Austria']);
  await countryInput(page).fill('schweiz');
  await expect(options(page).getByRole('option')).toHaveText(['Switzerland']);
});

test('a country of a file the app does not know stays and shows as it is', async ({ page }) => {
  await open(page, `${WIN}&scenario=no-profile`);
  await page.getByTestId('nav-profile').click();
  await page
    .getByTestId('profile-empty')
    .getByRole('button', { name: 'Aus Lebenslauf erstellen' })
    .click();
  const answer = JSON.stringify({
    name: 'Carla Exempel',
    kernkompetenzen: [{ kompetenz: 'Controlling', jahre: 20 }],
    harte_kriterien: { laender: ['DE', 'XK'] },
  });
  await page.getByTestId('paste-answer').fill(answer);
  await page.getByTestId('paste-take').click();
  await expect(chips(countries(page))).toHaveText(['Deutschland', 'XK']);
  await page.getByTestId('profile-save').click();
  expect((await lastSave(page)).after.criteria.countries).toEqual(['DE', 'XK']);
});

test('availability: the day exists only for "Ab Datum" and gets the caret', async ({ page }) => {
  await profile(page);
  const choices = page.getByTestId('profile-available');
  const date = page.getByTestId('profile-date');
  await choices.getByRole('button', { name: 'Sofort' }).click();
  await expect(date).toHaveCount(0);
  await choices.getByRole('button', { name: 'Ab Datum' }).click();
  await expect(date).toBeVisible();
  await expect(date).toBeFocused();
  await date.fill('1.11.2026');
  await choices.getByRole('button', { name: 'Sofort' }).click();
  await expect(date).toHaveCount(0);
  await page.getByTestId('profile-save').click();
  expect((await lastSave(page)).after.criteria.available).toEqual({ kind: 'now' });
});

test('neutral examples that fit any consultant, in both languages', async ({ page }) => {
  await open(page, `${WIN}&scenario=no-profile`);
  await page.getByTestId('nav-profile').click();
  await page.getByTestId('profile-empty').getByRole('button', { name: 'Profil anlegen' }).click();
  const placeholder = (id: string, input = false): Locator =>
    input ? page.getByTestId(id).locator('input') : page.getByTestId(id);
  const expected: [string, boolean, string][] = [
    ['profile-name-field', false, 'Vor- und Nachname'],
    ['profile-title', false, 'z. B. Interim Manager'],
    ['competence-name', false, 'z. B. Projektleitung'],
    ['competence-aliases', true, 'Synonyme'],
    ['profile-keywords', true, 'z. B. Transformation'],
    ['profile-tools', true, 'z. B. Scrum'],
    ['profile-certificates', true, 'z. B. PMP'],
    ['profile-industries', true, 'z. B. Handel'],
    ['profile-regions', true, 'z. B. München'],
    ['profile-countries', true, 'Land suchen'],
  ];
  for (const [id, input, text] of expected) {
    await expect(placeholder(id, input), id).toHaveAttribute('placeholder', text);
  }
  await open(page, `${WIN}&scenario=no-profile&lang=en`);
  await page.getByTestId('nav-profile').click();
  await page.getByTestId('profile-empty').getByRole('button').first().click();
  await expect(page.getByTestId('profile-name-field')).toHaveAttribute(
    'placeholder',
    'First and last name',
  );
  await expect(page.getByTestId('competence-aliases').locator('input')).toHaveAttribute(
    'placeholder',
    'Synonyms',
  );
});

test('the connected mailbox address is text to copy', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-settings').click();
  const address = page.getByTestId('settings-mailbox').locator('[data-copy]').first();
  await expect(address).toHaveText('alerts.demo@gmail.com');
  expect(await address.evaluate((node) => getComputedStyle(node).userSelect)).not.toBe('none');
});

test('the countries: the arrow keys move through the suggestions, Enter takes the highlighted one', async ({
  page,
}) => {
  await profile(page);
  const input = countryInput(page);
  await input.fill('schw');
  await expect(options(page).getByRole('option')).toHaveText(['Schweden', 'Schweiz']);
  await input.press('ArrowDown');
  await input.press('Enter');
  await expect(chips(countries(page))).toHaveText(['Deutschland', 'Österreich', 'Schweiz']);
  await input.fill('schw');
  await input.press('ArrowDown');
  await input.press('ArrowUp');
  await input.press('Enter');
  await expect(chips(countries(page))).toHaveText([
    'Deutschland',
    'Österreich',
    'Schweiz',
    'Schweden',
  ]);
});
