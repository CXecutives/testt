// Wave 2, group forms: the Profil form and its controls (the country suggestions, the
// references of fields and switches, the focus after a button goes, the day, the keys, the
// one-choice buttons, the narrow layout, a new form for a file that does not read).

import type { Locator, Page } from '@playwright/test';
import { calls, expect, open, test } from './fixtures';

const WIN = '?platform=windows';

async function profile(page: Page, query = `${WIN}&scenario=default`): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-profile').click();
}

/** A new form from the empty state. */
async function create(page: Page, scenario = 'no-profile'): Promise<void> {
  await profile(page, `${WIN}&scenario=${scenario}`);
  await page.getByTestId('profile-create').click();
  await expect(page.getByTestId('profile-form')).toBeVisible();
}

const chips = (field: Locator): Locator => field.locator('.chip .text');
const countries = (page: Page): Locator => page.getByTestId('profile-countries');
const countryInput = (page: Page): Locator => countries(page).locator('input');
const options = (page: Page): Locator => page.getByTestId('profile-countries-options');
const saves = async (page: Page): Promise<number> => (await calls(page, 'save_profile')).length;

/** Every id an aria-describedby, aria-labelledby or aria-controls inside `scope` names that
 *  is not in the page. */
async function dangling(scope: Locator): Promise<string[]> {
  return scope.evaluate((root) => {
    const missing: string[] = [];
    const attributes = ['aria-describedby', 'aria-labelledby', 'aria-controls'];
    for (const node of root.querySelectorAll(attributes.map((a) => `[${a}]`).join(', '))) {
      for (const attribute of attributes) {
        for (const id of (node.getAttribute(attribute) ?? '').split(/\s+/).filter(Boolean)) {
          if (document.getElementById(id) === null) {
            missing.push(`${node.getAttribute('data-testid') ?? node.tagName} ${attribute}=${id}`);
          }
        }
      }
    }
    return missing;
  });
}

test('ui-core-03: the country list has one mark, and the pointer moves it', async ({ page }) => {
  await profile(page);
  await countryInput(page).fill('s');
  const all = options(page).getByRole('option');
  await expect(all.first()).toHaveAttribute('aria-selected', 'true');
  const third = all.nth(2);
  const name = (await third.textContent())!.trim();
  const box = (await third.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  // The option under the pointer is the one mark, no second wash stays on the first.
  await expect(third).toHaveAttribute('aria-selected', 'true');
  await expect(options(page).locator('[aria-selected="true"]')).toHaveCount(1);
  const washes = await all.evaluateAll((nodes) =>
    nodes.map((node) => getComputedStyle(node).backgroundColor),
  );
  expect(new Set(washes.filter((_, index) => index !== 2)).size).toBe(1);
  expect(washes[2]).not.toBe(washes[0]);
  // Enter takes the marked one, which is the one under the pointer.
  await countryInput(page).press('Enter');
  await expect(chips(countries(page)).last()).toHaveText(name);
});

test('ui-forms-07: a chosen country says nothing; other names find their country', async ({
  page,
}) => {
  await profile(page);
  await expect(chips(countries(page))).toHaveText(['Deutschland', 'Österreich']);
  const none = page.getByTestId('profile-countries-none');
  for (const text of ['Deutsch', 'Deutschland', 'Germany', 'Öster']) {
    await countryInput(page).fill(text);
    await expect(none, text).toHaveCount(0);
    await expect(options(page), text).toBeHidden();
  }
  for (const [text, name] of [
    ['UK', 'Großbritannien'],
    ['England', 'Großbritannien'],
    ['Britain', 'Großbritannien'],
    ['Holland', 'Niederlande'],
    ['United States', 'USA'],
    ['Vereinigte Staaten', 'USA'],
    ['Amerika', 'USA'],
    ['Czech Republic', 'Tschechien'],
  ] as const) {
    await countryInput(page).fill(text);
    await expect(options(page).getByRole('option'), text).toHaveText([name]);
    await expect(none, text).toHaveCount(0);
  }
  await countryInput(page).press('Enter');
  await expect(chips(countries(page))).toHaveText(['Deutschland', 'Österreich', 'Tschechien']);
  // A name that is no country still says so.
  await countryInput(page).fill('Atlantis');
  await expect(none).toHaveText('Kein Land mit diesem Namen.');
});

test('ui-core-04: every reference of a field or switch names a text that is there', async ({
  page,
}) => {
  await profile(page);
  const view = page.getByTestId('profile');
  expect(await dangling(view)).toEqual([]);
  // A field with a hint is described by it, one without none; a switch only by a hint.
  await expect(page.getByTestId('profile-name-field')).not.toHaveAttribute('aria-describedby', /./);
  await expect(page.getByTestId('profile-strengths').locator('input')).toHaveAccessibleDescription(
    'Sie stützen die Passung, belegen aber keine Anforderung.',
  );
  await expect(page.getByTestId('profile-no-anue')).not.toHaveAttribute('aria-describedby', /./);
  await expect(page.getByTestId('profile-remote-outside')).toHaveAccessibleDescription(
    'Ausgeschaltet markiert die App ganz remote Jobs mit Sitz im Ausland zum Prüfen.',
  );
  // An error takes the place of the missing hint, and is its description.
  await page.getByTestId('profile-name-field').fill('x'.repeat(10));
  await page.getByTestId('profile-min-rate').fill('250000');
  await page.getByTestId('profile-save').click();
  await expect(page.getByTestId('profile-min-rate')).toHaveAccessibleDescription(
    /Höchstens 100\.000\./,
  );
  expect(await dangling(view)).toEqual([]);
  // The day, a new form and the values the app could not read.
  await page.getByTestId('profile-available').getByRole('radio', { name: 'Ab Datum' }).click();
  await page.getByTestId('profile-date').fill('1.13.2026');
  await page.getByTestId('profile-name-field').focus();
  await expect(page.getByTestId('profile-date')).toHaveAccessibleDescription(
    'Diesen Tag gibt es nicht.',
  );
  expect(await dangling(view)).toEqual([]);
  await create(page);
  expect(await dangling(page.getByTestId('profile'))).toEqual([]);
  await profile(page, `${WIN}&scenario=profile-unreadable`);
  expect(await dangling(page.getByTestId('profile'))).toEqual([]);
  // Einstellungen: a portal switch is described only while its "off" note shows.
  await open(page, WIN);
  await page.getByTestId('nav-settings').click();
  const toggle = page.getByTestId('toggle-enabled-linkedin');
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await expect(toggle).not.toHaveAttribute('aria-describedby', /./);
  expect(await dangling(page.getByTestId('view-settings'))).toEqual([]);
  await toggle.click();
  await expect(page.getByTestId('portal-off-linkedin')).toBeVisible();
  await expect(toggle).toHaveAttribute('aria-describedby', 'switch-enabled-linkedin-hint');
  expect(await dangling(page.getByTestId('view-settings'))).toEqual([]);
});

test('live-forms-12: the day is judged when it is left or saved, never while typed', async ({
  page,
}) => {
  await profile(page);
  await page.getByTestId('profile-available').getByRole('radio', { name: 'Ab Datum' }).click();
  const date = page.getByTestId('profile-date');
  const error = page.getByTestId('profile-date-error');
  await expect(date).toBeFocused();
  for (const character of '1.11.2026') {
    await page.keyboard.type(character);
    await expect(error).toHaveCount(0);
  }
  await page.keyboard.press('Tab');
  await expect(error).toHaveCount(0);
  // A day in the wrong form, said when the field is left.
  await date.fill('1.11.202');
  await expect(error).toHaveCount(0);
  await page.getByTestId('profile-name-field').focus();
  await expect(error).toHaveText('Gib das Datum im Format 01.11.2026 ein.');
  await expect(date).toHaveAttribute('aria-invalid', 'true');
  // Typing again waits for the next judgement.
  await date.focus();
  await page.keyboard.type('6');
  await expect(error).toHaveCount(0);
  // A day in the right form that the calendar does not have says so.
  await date.fill('31.02.2026');
  await page.getByTestId('profile-name-field').focus();
  await expect(error).toHaveText('Diesen Tag gibt es nicht.');
  // Left empty, it waits for the save.
  await date.fill('');
  await page.getByTestId('profile-name-field').focus();
  await expect(error).toHaveCount(0);
  await page.getByTestId('profile-save').click();
  await expect(error).toHaveText('Gib das Datum im Format 01.11.2026 ein.');
  // After "Verwerfen", the day chosen anew is judged anew.
  await date.fill('31.02.2026');
  await page.getByTestId('profile-name-field').focus();
  await expect(error).toBeVisible();
  await page.getByTestId('profile-discard').click();
  await expect(date).toHaveCount(0);
  await page.getByTestId('profile-available').getByRole('radio', { name: 'Ab Datum' }).click();
  await expect(date).toBeFocused();
  await expect(error).toHaveCount(0);
  // In English.
  await profile(page, `${WIN}&scenario=default&lang=en`);
  await page.getByTestId('profile-available').getByRole('radio', { name: 'From a date' }).click();
  await page.getByTestId('profile-date').fill('31/02/2026');
  await page.getByTestId('profile-name-field').focus();
  await expect(error).toHaveText('This day does not exist.');
});

test('live-forms-13: a wrong day is said once, at its field', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-available').getByRole('radio', { name: 'Ab Datum' }).click();
  await page.getByTestId('profile-date').fill('31.02.2026');
  await page.getByTestId('profile-save').click();
  await expect(page.getByTestId('profile-date')).toBeFocused();
  await expect(page.getByTestId('profile-date-error')).toHaveText('Diesen Tag gibt es nicht.');
  const said = (await page.locator('body').innerText()).split('Diesen Tag gibt es nicht.');
  expect(said).toHaveLength(2);
  await expect(page.getByTestId('profile-save-status')).not.toContainText('Tag');
  expect(await saves(page)).toBe(0);
});

test('live-forms-15: one choice is one Tab stop, and the arrows choose', async ({ page }) => {
  await profile(page);
  const row = page.getByTestId('language-row').first();
  const levels = row.getByTestId('language-level');
  await expect(levels).toHaveAttribute('role', 'radiogroup');
  const chosen = levels.locator('[aria-checked="true"]');
  const name = (await chosen.textContent())!.trim();
  // Name, the chosen level, the x: three stops.
  await row.getByTestId('language-name').focus();
  await page.keyboard.press('Tab');
  await expect(levels.getByRole('radio', { name })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(row.getByTestId('language-remove')).toBeFocused();
  // The arrows choose the next level, Home and End the ends.
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('ArrowRight');
  const next = levels.locator('[aria-checked="true"]');
  await expect(next).toBeFocused();
  expect((await next.textContent())!.trim()).not.toBe(name);
  await page.keyboard.press('Home');
  await expect(levels.getByRole('radio', { name: 'A1' })).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('End');
  await expect(levels.getByRole('radio', { name: 'Muttersprache' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(levels.locator('[tabindex="0"]')).toHaveCount(1);
  // Space on the chosen one clears it; the group keeps one stop, the first.
  await page.keyboard.press('Space');
  await expect(levels.locator('[aria-checked="true"]')).toHaveCount(0);
  await expect(levels.locator('[tabindex="0"]')).toHaveText('A1');
  // The chosen option keeps the pressed look.
  const remote = page.getByTestId('profile-remote');
  await expect(remote).toHaveAttribute('role', 'radiogroup');
  const look = await remote
    .getByRole('radio')
    .evaluateAll((nodes) =>
      nodes.map((node) => [node.getAttribute('aria-checked'), getComputedStyle(node).color]),
    );
  const on = look.filter(([checked]) => checked === 'true').map(([, color]) => color);
  const off = look.filter(([checked]) => checked === 'false').map(([, color]) => color);
  expect(on).toHaveLength(1);
  expect(off).not.toContain(on[0]);
  const sent = async (): Promise<unknown> => {
    await page.getByTestId('profile-save').click();
    const all = await calls(page, 'save_profile');
    return (all.at(-1)![1] as { save: { after: { languages: { level: unknown }[] } } }).save.after
      .languages[0]!.level;
  };
  expect(await sent()).toBeNull();
});

test('live-forms-16: narrow, a Schwerpunkt wraps at its words; a field action ends its line', async ({
  page,
}) => {
  await page.setViewportSize({ width: 480, height: 640 });
  await profile(page);
  const chip = page
    .getByTestId('focus')
    .locator('.chip .text')
    .filter({ hasText: 'Konzernrechnungslegung nach IFRS' });
  await chip.scrollIntoViewIfNeeded();
  // The long word stands on one line.
  const lines = await chip.evaluate((node) => {
    const text = node.firstChild!;
    const range = document.createRange();
    const at = text.textContent!.indexOf('Konzernrechnungslegung');
    range.setStart(text, at);
    range.setEnd(text, at + 'Konzernrechnungslegung'.length);
    return new Set([...range.getClientRects()].map((rect) => Math.round(rect.top))).size;
  });
  expect(lines).toBe(1);
  // Wide, the chips stay beside their label.
  await page.setViewportSize({ width: 1360, height: 900 });
  const label = (await page.getByTestId('focus-count').boundingBox())!;
  const first = (await page.getByTestId('focus').locator('.chip').first().boundingBox())!;
  expect(first.x).toBeGreaterThan(label.x + label.width);
  expect(Math.abs(first.y + first.height / 2 - (label.y + label.height / 2))).toBeLessThan(2);
  // "Wert entfernen" ends the message line of a half-width field as of a full-width one.
  await profile(page, `${WIN}&scenario=profile-unreadable`);
  for (const field of ['minDayRate', 'regions']) {
    const scope = page.locator(`[data-field="${field}"]`);
    const action = (await scope.getByTestId('value-remove').boundingBox())!;
    const message = (await scope.locator('[role="alert"]').boundingBox())!;
    const box = (await scope.boundingBox())!;
    expect(Math.abs(action.x + action.width - (box.x + box.width)), field).toBeLessThan(2);
    expect(action.y, field).toBeLessThan(message.y + message.height / 2);
  }
});
