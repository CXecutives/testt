// The language switch: Einstellungen > Sprache turns the whole app to English and back at
// once (no reload), the backend stores the choice, and a backend that says English starts
// the app in English. Words, numbers and dates follow; the baselines stay German, two
// English ones show the reader and the settings.

import type { Page } from '@playwright/test';
import { calls, expect, expectShot, open, test } from './fixtures';

const WIN = '?platform=windows';
const EN = `${WIN}&lang=en`;

const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');

async function settings(page: Page, query = WIN): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings')).toBeVisible();
}

async function readFirst(page: Page): Promise<void> {
  await page.getByTestId('nav-jobs').click();
  await rows(page).first().click();
  await expect(page.getByTestId('reader-ring')).toContainText('91');
}

test('Sprache switches the whole app to English and back at once', async ({ page }) => {
  await settings(page);
  const choice = page.getByTestId('language');
  await expect(choice.getByRole('radio', { name: 'Deutsch' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(page.getByTestId('settings-language')).toContainText('Sprache der App');

  await choice.getByRole('radio', { name: 'English' }).click();
  // The page switches before anything reloads: the sidebar, the headings, the document.
  await expect(page.getByTestId('nav-settings')).toContainText('Settings');
  await expect(page.getByTestId('nav-profile')).toContainText('Profile');
  await expect(page.getByTestId('settings-language')).toContainText('App language');
  await expect(page.getByTestId('settings-fetch')).toContainText('Fetch at start');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect(choice.getByRole('radio', { name: 'English' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  expect((await calls(page, 'save_settings')).map(([, args]) => args)).toEqual([
    {
      patch: {
        portals: [],
        autoFetchOnStart: null,
        autoArchiveDays: null,
        autoEmptyTrashDays: null,
        language: 'en',
      },
    },
  ]);

  // The Jobs view in English: the list header, the reader, its numbers and words.
  await readFirst(page);
  await expect(page.getByTestId('list-header')).toContainText('Fetch');
  await expect(page.getByTestId('reader-ring')).toHaveAttribute('aria-label', /^Match 91%/);
  await expect(page.getByTestId('band')).toHaveText('High match');
  await expect(page.getByTestId('prompt')).toContainText('Copy as prompt');
  await expect(page.getByTestId('reader')).not.toContainText('Passung');

  // Back to German, the same way.
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('language').getByRole('radio', { name: 'Deutsch' }).click();
  await expect(page.getByTestId('nav-settings')).toContainText('Einstellungen');
  await expect(page.locator('html')).toHaveAttribute('lang', 'de');
  // The job stays open; its reader speaks German again.
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('band')).toHaveText('Hohe Passung');
  // German puts a narrow no-break space before the percent sign.
  await expect(page.getByTestId('reader-ring')).toHaveAttribute('aria-label', /^Passung 91\s%/);
  await expect(page.getByTestId('list-header')).toContainText('Abrufen');
});

test('the app starts in the language the backend says', async ({ page }) => {
  await open(page, EN);
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect(page.getByTestId('nav-jobs')).toContainText('Jobs');
  await expect(page.getByTestId('nav-settings')).toContainText('Settings');
  // Relative dates and the day overview speak English too.
  await expect(page.getByTestId('day-overview')).not.toContainText('Passung');
  await expect(rows(page).first()).not.toContainText(/gestern|vor \d/);
  await page.getByTestId('nav-settings').click();
  await expect(
    page.getByTestId('language').getByRole('radio', { name: 'English' }),
  ).toHaveAttribute('aria-checked', 'true');
});

test('baseline: jobs with the reader in English', async ({ page }) => {
  await open(page, EN);
  await rows(page).first().click();
  await expect(page.getByTestId('reader-ring')).toContainText('91');
  await expectShot(page, 'jobs-reader-en');
});

test('baseline: settings in English', async ({ page }) => {
  await settings(page, EN);
  await page.getByTestId('settings-language').scrollIntoViewIfNeeded();
  await expectShot(page, 'settings-en');
});
