// Wave 1, backend track: what the page shows of the backend's files, texts and settings,
// against the stub (which mirrors the backend's contract).

import type { Page } from '@playwright/test';
import { calls, expect, open, test } from './fixtures';

const WIN = '?platform=windows';
const MAC = '?platform=macos';

async function settings(page: Page, query = WIN): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings')).toBeVisible();
}

async function lastOpened(page: Page): Promise<unknown> {
  return (await calls(page, 'open_target')).at(-1)?.[1];
}

test('the Excel file shows itself in its folder, in the words of the OS', async ({ page }) => {
  await settings(page);
  const reveal = page.getByTestId('excel').getByTestId('excel-reveal');
  await expect(reveal).toHaveText('Im Explorer zeigen');
  await reveal.click();
  expect(await lastOpened(page)).toEqual({ target: { kind: 'excelInFolder' } });
  // "Öffnen" stays the last button of the row, like every opener of this card.
  const labels = await page
    .getByTestId('excel')
    .locator('.btn')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-testid')));
  expect(labels).toEqual(['excel-reveal', 'excel-open']);

  await settings(page, MAC);
  await expect(page.getByTestId('excel-reveal')).toHaveText('Im Finder zeigen');
});

test('no Excel file yet: it can neither open nor show itself, and says why', async ({ page }) => {
  await settings(page, `${WIN}&scenario=no-files`);
  const reveal = page.getByTestId('excel-reveal');
  await expect(reveal).toHaveAttribute('aria-disabled', 'true');
  await reveal.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Die Excel-Datei entsteht beim ersten Abruf.');
});

test('"Ordner öffnen" of the day overview shows the Excel file in its folder', async ({ page }) => {
  await open(page, WIN);
  const folder = page.getByTestId('day-overview').getByTestId('overview-folder');
  await expect(folder).toHaveText('Ordner öffnen');
  await folder.click();
  expect(await lastOpened(page)).toEqual({ target: { kind: 'excelInFolder' } });
});
