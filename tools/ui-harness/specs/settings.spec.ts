// Einstellungen and the first-run page against the stub.

import type { Page } from '@playwright/test';
import { calls, expect, expectShot, open, test, visibleCount } from './fixtures';

const WIN = '?platform=windows';

async function settings(page: Page, query = WIN): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings')).toBeVisible();
}

test('first run: three steps that tick themselves, fetch locked until a mailbox', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=first-run`);
  await expect(page.getByTestId('first-run')).toBeVisible();
  expect(await visibleCount(page, '.btn.primary')).toBe(1);

  const fetch = page.getByTestId('first-fetch');
  await expect(fetch).toHaveAttribute('aria-disabled', 'true');
  await fetch.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Erst ein Postfach verbinden.');

  await page.getByTestId('mailbox-user').fill('alerts.demo');
  await page.getByTestId('mailbox-password').fill('abcdabcdabcdabcd');
  await page.getByTestId('mailbox-save').click();
  await expect(page.getByTestId('mailbox-form')).toContainText('Die Adresse ist unvollständig.');
  await page.getByTestId('mailbox-user').fill('alerts.demo@gmail.com');
  await page.getByTestId('mailbox-password').fill('kurz');
  await page.getByTestId('mailbox-password').press('Enter');
  await expect(page.getByTestId('mailbox-form')).toContainText('16 Buchstaben');
  await page.getByTestId('mailbox-password').fill('abcd efgh ijkl mnop');
  await page.getByTestId('mailbox-password').press('Enter');
  await expect(page.getByTestId('step-mailbox')).toHaveAttribute('data-done', 'true');
  await expect(fetch).not.toHaveAttribute('aria-disabled', 'true');

  // The profile is made in the Profil view; back on the first-run page its step is done.
  await page.getByTestId('first-profile').click();
  await expect(page.getByTestId('view-profile')).toBeVisible();
  await page.getByTestId('profile-empty').getByRole('button', { name: 'Profil anlegen' }).click();
  await page.getByTestId('competence-add').click();
  await page.getByTestId('competence-name').fill('Controlling');
  await page.getByTestId('profile-save').click();
  await expect(page.getByTestId('profile-name')).toHaveText('beraterprofil.json');
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('step-profile')).toHaveAttribute('data-done', 'true');
  expect(await visibleCount(page, '.btn.primary')).toBe(1);

  await fetch.click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await expect(page.getByTestId('run-running')).toBeVisible();
});

test('settings: sections, no primary while nothing asks for one', async ({ page }) => {
  await settings(page);
  for (const id of ['mailbox', 'fetch', 'portals', 'files', 'care']) {
    await expect(page.getByTestId(`settings-${id}`)).toBeVisible();
  }
  // "Abrufen" lives in the list of the Jobs view: nothing here asks for a primary.
  expect(await visibleCount(page, '.btn.primary')).toBe(0);
  await expect(page.getByTestId('settings-mailbox')).toContainText('alerts.demo@gmail.com');
});

test('changing the mailbox: form with save and cancel, Esc cancels', async ({ page }) => {
  await settings(page);
  await page.getByTestId('mailbox-change').click();
  await expect(page.getByTestId('mailbox-form')).toBeVisible();
  expect(await visibleCount(page, '.btn.primary')).toBe(1);
  await expect(page.getByTestId('mailbox-save')).toHaveClass(/primary/);
  await page.getByTestId('mailbox-password').press('Escape');
  await expect(page.getByTestId('mailbox-form')).toHaveCount(0);
});

test('removing the mailbox asks first', async ({ page }) => {
  await settings(page);
  await page.getByTestId('mailbox-remove').click();
  const dialog = page.getByTestId('dialog-remove-mailbox');
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Entfernen' }).click();
  await expect(page.getByTestId('mailbox-form')).toBeVisible();
  expect(await calls(page, 'remove_mailbox')).toHaveLength(1);
});

test('auto fetch and portal switches save at once', async ({ page }) => {
  await settings(page);
  await page.getByTestId('toggle-auto-fetch').click();
  await expect(page.getByTestId('toggle-auto-fetch')).toHaveAttribute('aria-checked', 'false');
  await page.getByTestId('toggle-enabled-linkedin').click();
  await expect(page.getByTestId('toggle-enabled-linkedin')).toHaveAttribute(
    'aria-checked',
    'false',
  );
  // "Aktiv" sits in the portal's header row; an inactive portal hides "Details holen".
  await expect(
    page.getByTestId('portal-linkedin').locator('.head').getByTestId('toggle-enabled-linkedin'),
  ).toBeVisible();
  await expect(page.getByTestId('toggle-details-linkedin')).toHaveCount(0);
  await expect(page.getByTestId('toggle-details-freelancermap')).toBeVisible();
  const saved = (await calls(page, 'save_settings')).map(([, args]) => args);
  expect(saved).toEqual([
    { patch: { portals: [], autoFetchOnStart: false } },
    {
      patch: {
        portals: [{ portal: 'linkedin', enabled: false, fetchDetails: null, loginEnabled: null }],
        autoFetchOnStart: null,
      },
    },
  ]);
});

test('freelance.de sign-in: account risk, sign in and out', async ({ page }) => {
  await settings(page);
  const card = page.getByTestId('portal-freelance');
  await page.getByTestId('toggle-login-freelance').click();
  await expect(card).toContainText('Kontorisiko');
  await page.getByTestId('sign-in-freelance').click();
  await expect(card).toContainText('Angemeldet');
  await page.getByTestId('sign-out-freelance').click();
  await expect(card).toContainText('Nicht angemeldet');
  expect(await calls(page, 'portal_logout')).toHaveLength(1);
});

test('quota only from 80 %, pauses with reason and end', async ({ page }) => {
  await settings(page);
  await expect(page.getByTestId('quota-freelancermap')).toContainText('Heute 86 von 100 Seiten');
  await expect(page.getByTestId('quota-linkedin')).toHaveCount(0);
  await settings(page, `${WIN}&scenario=paused`);
  await expect(page.getByTestId('health-linkedin')).toContainText(
    'Das Portal bremst die Anfragen. Pause bis 11:05.',
  );
  await expect(page.getByTestId('health-freelance')).toContainText(
    '2 Alert-Mails ohne erkannte Jobs',
  );
});

test('files: rewrite and delete the text files where they are', async ({ page }) => {
  await settings(page);
  await page.getByTestId('txt-rewrite').click();
  await expect(page.getByTestId('toast').last()).toHaveText('38 Dateien geschrieben.');
  await page.getByTestId('txt-clear').click();
  await page.getByTestId('dialog-clear').getByRole('button', { name: 'Löschen' }).click();
  await expect(page.getByTestId('toast').last()).toHaveText('38 Dateien gelöscht.');
  await expect(page.getByTestId('txt-clear')).toHaveAttribute('aria-disabled', 'true');
});

test('reading the whole mailbox asks first, then shows the run', async ({ page }) => {
  await settings(page);
  await page.getByTestId('full-mailbox').click();
  await page
    .getByTestId('dialog-full-mailbox')
    .getByRole('button', { name: 'Postfach lesen' })
    .click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  const started = await calls(page, 'start_run');
  expect((started[0]?.[1] as { request: unknown }).request).toEqual({ kind: 'fullMailbox' });
});

test('reset asks with a danger dialog; the report shows after the restart', async ({ page }) => {
  await settings(page);
  await page.getByTestId('reset').click();
  const dialog = page.getByTestId('dialog-reset');
  await expect(dialog.getByRole('button', { name: 'Abbrechen' })).toBeFocused();
  await dialog.getByRole('button', { name: 'Zurücksetzen' }).click();
  expect(await calls(page, 'reset_all')).toHaveLength(1);
  await settings(page, `${WIN}&scenario=reset`);
  await expect(page.getByTestId('reset-report')).toHaveText(
    'Die App wurde zurückgesetzt. 1 Datei ließ sich nicht löschen.',
  );
});

test('locked buttons explain themselves', async ({ page }) => {
  // A workspace without files yet has no Excel file to show.
  await settings(page, `${WIN}&scenario=no-files`);
  const excel = page.getByTestId('excel-show');
  await excel.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Die Excel-Datei entsteht beim ersten Abruf.');
  // Without a mailbox reading the whole mailbox is locked.
  await page.getByTestId('mailbox-remove').click();
  await page
    .getByTestId('dialog-remove-mailbox')
    .getByRole('button', { name: 'Entfernen' })
    .click();
  await expect(page.getByTestId('mailbox-form')).toBeVisible();
  await page.getByTestId('full-mailbox').hover();
  await expect(page.getByRole('tooltip')).toHaveText('Erst ein Postfach verbinden.');
});

test('first run: the sidebar waits until the setup is done', async ({ page }) => {
  await open(page, `${WIN}&scenario=first-run`);
  const nav = page.getByTestId('nav-settings').locator('xpath=../..');
  await expect(nav).toHaveAttribute('inert', '');
  await page
    .getByTestId('nav-settings')
    .click({ force: true, timeout: 2000 })
    .catch(() => {});
  await expect(page.getByTestId('first-run')).toBeVisible();
  // The helper line of the password carries the way to create one.
  await expect(
    page.getByTestId('mailbox-form').locator('.help').getByTestId('create-password'),
  ).toBeVisible();
});

test('baseline: first run', async ({ page }) => {
  await open(page, `${WIN}&scenario=first-run`);
  await expectShot(page, 'first-run');
});

test('baseline: settings', async ({ page }) => {
  await settings(page);
  await expectShot(page, 'settings');
});

test('baseline: settings with portals', async ({ page }) => {
  await settings(page, `${WIN}&scenario=paused`);
  await page.getByTestId('settings-portals').scrollIntoViewIfNeeded();
  await expectShot(page, 'settings-portals');
});
