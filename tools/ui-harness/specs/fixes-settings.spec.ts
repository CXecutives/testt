// Fixes of the final audit round for Einstellungen, the first run, the mailbox form and the
// Profil page (track "settings").

import type { Page } from '@playwright/test';
import { expect, open, test } from './fixtures';

const WIN = '?platform=windows';

async function settings(page: Page, query = WIN): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings')).toBeVisible();
}

test('a rescore locks the settings with its own reason, not a fetch', async ({ page }) => {
  await settings(page);
  // The sign-in row exists only with "Mit Anmeldung" on.
  await page.getByTestId('toggle-login-freelance').click();
  await expect(page.getByTestId('sign-in-freelance')).toBeVisible();
  // The profile changed: the app scores every job anew by itself and holds the settings.
  await page.evaluate(() => {
    window.__harness.holdAfter = 2;
    window.__harness.appRun('rescore');
  });
  for (const id of [
    'mailbox-change',
    'txt-rewrite',
    'full-mailbox',
    'reset',
    'sign-in-freelance',
  ]) {
    const button = page.getByTestId(id);
    await expect(button).toHaveAttribute('aria-disabled', 'true');
    await button.hover();
    await expect(page.getByRole('tooltip')).toHaveText('Die Jobs werden gerade neu bewertet.');
  }
  await page.evaluate(() => (window.__harness.holdAfter = null));
});
