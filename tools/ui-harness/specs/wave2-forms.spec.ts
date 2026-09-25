// Wave 2, group forms: the Profil form and its controls (the country suggestions, the
// references of fields and switches, the focus after a button goes, the day, the keys, the
// one-choice buttons, the narrow layout, a new form for a file that does not read).

import type { Page } from '@playwright/test';
import { calls, expect, open, test } from './fixtures';

const WIN = '?platform=windows';

async function profile(page: Page, query = `${WIN}&scenario=default`): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-profile').click();
}

const saves = async (page: Page): Promise<number> => (await calls(page, 'save_profile')).length;

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
