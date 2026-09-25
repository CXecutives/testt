// The words of the final text audit: what a text promises is what the app does, one name
// for one thing, numbers formatted like every other count, rows in short words.

import type { Page } from '@playwright/test';
import { expect, open, test } from './fixtures';

const WIN = '?platform=windows';

async function settings(page: Page, query = WIN): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings')).toBeVisible();
}

test('the text files say what they are for, and that only Rewrite brings them back', async ({
  page,
}) => {
  await settings(page);
  const files = page.getByTestId('settings-files');
  await expect(files).toContainText('38 Anzeigen als Text für eine KI-Bewertung');
  // A fetch never writes a deleted text file again (core: the file keeps its mark).
  await page.getByTestId('txt-clear').click();
  const dialog = page.getByTestId('dialog-clear');
  await expect(dialog).toContainText('Nur „Neu schreiben“ holt sie zurück.');
  await expect(dialog).not.toContainText('Abruf');
  await dialog.getByRole('button', { name: 'Löschen' }).click();
  await expect(page.getByTestId('files-note')).toHaveText('38 Dateien gelöscht.');
  await expect(files).toContainText('0 Anzeigen als Text für eine KI-Bewertung');
  // Nothing promises that they come back by themselves.
  await page.getByTestId('txt-clear').hover();
  await expect(page.getByRole('tooltip')).toHaveText('Es gibt keine Textdateien.');
});
