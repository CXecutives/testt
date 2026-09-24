// The Profil view against the stub.

import type { Page } from '@playwright/test';
import { calls, expect, expectShot, open, test } from './fixtures';

async function profile(page: Page, scenario = 'default'): Promise<void> {
  await open(page, `?platform=windows&scenario=${scenario}`);
  await page.getByTestId('nav-profile').click();
  await expect(page.getByTestId('profile')).toBeVisible();
}

test('the profile shows what the app understood, in plain words', async ({ page }) => {
  await profile(page);
  await expect(page.getByTestId('profile-name')).toHaveText('profil-interim-finance.json');
  await expect(page.getByTestId('profile-file')).toContainText('18 KB · 21.09.2026');
  await expect(page.getByTestId('profile-file')).toContainText('Gut lesbar');
  // Label | value rows; what the profile leaves open says "nicht gesetzt", in the subtle tone.
  const list = page.getByTestId('criteria-list');
  await expect(list.locator('dt')).toHaveText([
    'Tagessatz',
    'Einsatzland',
    'Arbeitnehmerüberlassung',
    'Verfügbarkeit',
    'Mindestgehalt',
    'Region',
    'Seniorität',
  ]);
  await expect(list.locator('dd')).toHaveText([
    'ab 1.100 €',
    'DE, AT',
    'ausgeschlossen',
    'nicht gesetzt',
    'nicht gesetzt',
    'nicht gesetzt',
    'ab 15 Jahren Erfahrung',
  ]);
  await expect(list.locator('dd.unset')).toHaveCount(3);
  await expect(list.locator('svg')).toHaveCount(0);
  await expect(page.getByTestId('background')).toHaveText(
    '28 Jahre Berufserfahrung · Diplom-Kauffrau',
  );
  await expect(page.getByTestId('packs')).toHaveText('Finanzen · SAP');
  await expect(page.getByTestId('competences')).toContainText('+30');
  // The core sends no warning for keys it does not evaluate: no sentence about them.
  await expect(page.getByTestId('profile-understood')).toContainText('Erkannt');
  await expect(page.getByTestId('ignored-keys')).toHaveCount(0);
  await expect(page.getByTestId('profile-understood').getByRole('alert')).toHaveCount(0);
});

test('remove asks first; choosing a profile again rescores', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-remove').click();
  await page
    .getByTestId('dialog-remove-profile')
    .getByRole('button', { name: 'Entfernen' })
    .click();
  await expect(page.getByTestId('profile-empty')).toBeVisible();
  expect(await calls(page, 'remove_profile')).toHaveLength(1);
  await page.getByTestId('profile-empty').getByRole('button', { name: 'Profil wählen' }).click();
  await expect(page.getByTestId('profile-rescored')).toHaveText('Neu bewertet.');
});

test('saving the template confirms with a toast', async ({ page }) => {
  await profile(page);
  await page.getByTestId('profile-template').click();
  await expect(page.getByTestId('toast')).toHaveText('Die Vorlage ist gespeichert.');
});

test('a profile edited into broken JSON says so and where', async ({ page }) => {
  await profile(page, 'profile-broken');
  await expect(page.getByTestId('profile-parse-error')).toContainText(
    'Die Datei ist kein gültiges JSON, Zeile 12.',
  );
});

test('baseline: profile', async ({ page }) => {
  await profile(page);
  await expectShot(page, 'profile');
});

test('baseline: no profile yet', async ({ page }) => {
  await profile(page, 'no-profile');
  await expectShot(page, 'profile-empty');
});
