// Wave 1, backend track: what the page shows of the backend's files, texts and settings,
// against the stub (which mirrors the backend's contract).

import type { Page } from '@playwright/test';
import type { ExportSummary, RunEvent } from '../../../ui/src/lib/ipc/types';
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

/** A fetch that finished as the backend reports it, with what its export says. */
function finished(files: Partial<ExportSummary>): RunEvent {
  return {
    type: 'finished',
    summary: {
      run: 42,
      kind: 'fetch',
      outcome: { kind: 'completed' },
      dryRun: false,
      startedAt: '2026-09-24T07:29:00Z',
      finishedAt: '2026-09-24T07:30:00Z',
      scan: null,
      perPortal: [],
      newJobs: { count: 0, high: 0 },
      score: null,
      export: {
        overviewXlsx: null,
        overviewHtml: null,
        backup: null,
        txtWritten: 0,
        txtFailed: 0,
        error: null,
        ...files,
      },
      emptyAlerts: [],
    },
  };
}

async function finish(page: Page, files: Partial<ExportSummary>): Promise<void> {
  await page.evaluate((event) => {
    window.__harness.emit({ type: 'started', kind: 'fetch' });
    window.__harness.emit(event);
  }, finished(files));
}

test('an unreachable work folder is said as such in the run card, with a retry', async ({
  page,
}) => {
  await open(page, WIN);
  await finish(page, { error: { kind: 'io', params: { target: 'workspace' } } });
  const failed = page.getByTestId('export-failed');
  await expect(failed).toContainText('Der Arbeitsordner ist nicht erreichbar.');
  await expect(failed).not.toContainText('Textdateien');
  await expect(failed.getByRole('button')).toHaveText('Erneut versuchen');
});

test('the old Excel file the export renamed is named once and shows in its folder', async ({
  page,
}) => {
  await open(page, WIN);
  const name = 'JobAlerts.alt-20260924-093000.xlsx';
  await finish(page, {
    backup: `C:\\Users\\demo\\Documents\\Job-Alerts\\auswertung\\${name}`,
  });
  const note = page.getByTestId('excel-renamed');
  await expect(note).toHaveCount(1);
  await expect(note).toContainText(`Die alte Excel-Datei heißt jetzt ${name}.`);
  await note.getByRole('button', { name: 'Im Explorer zeigen' }).click();
  expect(await lastOpened(page)).toEqual({ target: { kind: 'excelBackupInFolder', name } });
});
