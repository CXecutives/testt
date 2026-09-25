// The words of the final text audit: what a text promises is what the app does, one name
// for one thing, numbers formatted like every other count, rows in short words.

import type { Page } from '@playwright/test';
import { expect, open, runFinished, test } from './fixtures';

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

test('reset names everything it deletes before it asks', async ({ page }) => {
  await settings(page);
  await expect(page.getByTestId('settings-reset')).toContainText(
    'Löscht Jobs, Einstellungen, Profil, App-Passwort und Anmeldungen.',
  );
  await page.getByTestId('reset').click();
  // The files in the user's own work folder go too (core reset: `app_files`).
  await expect(page.getByTestId('dialog-reset')).toContainText(
    'Die App startet neu und löscht auch Excel-Datei, Übersicht und Textdateien im Arbeitsordner.',
  );
  // What stays behind need not be a file (the app password, a sign-in).
  await open(page, `${WIN}&scenario=reset`);
  await expect(page.getByTestId('first-reset-report')).not.toContainText('Datei');
});

test('the English reader counts the must-have requirements, as the German one does', async ({
  page,
}) => {
  await open(page, `${WIN}&lang=en`);
  await page.getByTestId('job-rows').locator('[data-testid^="job-row-"]').first().click();
  // German counts Pflichtanforderungen; the English AI prompt says "must-have requirements".
  await expect(page.getByTestId('must')).toHaveText(/^\d+ of \d+ must-have requirements met/);
});

test('an excluded row names a missing degree or licence in short words, never a sentence', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const excluded = page.getByTestId('excluded-rows');
  // A country outside the profile says what does not fit, like its neighbours.
  await expect(excluded.getByTestId('job-row-linkedin-4100200305').locator('.foot')).toHaveText(
    'Einsatzland passt nicht',
  );
  const key = { portal: 'freelance', id: '900412' } as const;
  const row = excluded.getByTestId('job-row-freelance-900412');
  await expect(row.locator('.foot')).toHaveText('Arbeitnehmerüberlassung');
  const job = await page.evaluate((k) => window.__harness.job(k), key);
  // The engine excludes on a degree or licence the ad makes mandatory (`formalOpen`).
  const exclude = (code: string, params: Record<string, string | boolean>) =>
    page.evaluate(
      ([base, note]) => {
        window.__harness.emit({
          type: 'jobUpdated',
          job: { ...base, match: { ...base.match!, note } },
          fresh: false,
        });
      },
      [job!, { code, params }] as const,
    );
  await exclude('formalOpen', { class: 'degree', mandatory: true });
  await expect(row.locator('.foot')).toHaveText('Abschluss fehlt');
  await exclude('formalOpen', { class: 'licence', mandatory: true });
  await expect(row.locator('.foot')).toHaveText('Zulassung fehlt');
  // A code of a newer core: the plain word, not a raw code and not a cut sentence.
  await exclude('somethingNew', {});
  await expect(row.locator('.foot')).toHaveText('Ausgeschlossen');
});

test('reading the whole mailbox has one name: in the list, the settings and the run', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=empty`);
  await expect(page.getByTestId('read-older')).toContainText('Ganzes Postfach lesen');
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings-care')).toContainText('Ganzes Postfach lesen');
  // The run card names the run by its kind until the first status comes.
  await page.evaluate(() => (window.__harness.holdAfter = 1));
  await page.getByTestId('full-mailbox').click();
  await page
    .getByTestId('dialog-full-mailbox')
    .getByRole('button', { name: 'Postfach lesen' })
    .click();
  await expect(page.getByTestId('run-running')).toContainText('Ganzes Postfach lesen');
  await page.evaluate(() => (window.__harness.holdAfter = null));
  await runFinished(page);
});

test('the selection bar counts with a thousands separator, like the pane beside it', async ({
  page,
}) => {
  // A thousand rows come in windows of 60: more time than a usual test.
  test.setTimeout(60_000);
  await open(page, `${WIN}&scenario=many`);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const rows = page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
  await rows.first().click();
  // The list shows its rows in windows: scroll until more than a thousand are there.
  const list = page.getByTestId('list-scroll');
  await expect
    .poll(
      async () => {
        await list.evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
        return rows.count();
      },
      { intervals: [50], timeout: 40_000 },
    )
    .toBeGreaterThan(1000);
  await rows.nth(1000).click({ modifiers: ['Shift'] });
  await expect(page.getByTestId('selection-count')).toHaveText('1.001 ausgewählt');
  await expect(page.getByTestId('reader-pane')).toContainText('1.001 Jobs ausgewählt');
});

test('the first run names the portals whose alerts belong in the mailbox', async ({ page }) => {
  await open(page, `${WIN}&scenario=first-run`);
  // The portals in the app's one order (the settings'), by their web address.
  await expect(page.getByTestId('step-mailbox')).toContainText(
    'Die Alert-Mails von linkedin.com, freelance.de und freelancermap.de gehören hierher.',
  );
  await open(page, `${WIN}&scenario=first-run&lang=en`);
  await expect(page.getByTestId('step-mailbox')).toContainText(
    'The alert emails from linkedin.com, freelance.de and freelancermap.de belong here.',
  );
  // "in Gmail" never breaks apart: no line of the intro ends with the preposition.
  const intro = await page.getByTestId('first-run').locator('.benefit').textContent();
  expect(intro).toContain('in Gmail');
});

test('an empty list during a fetch says the jobs come in as it goes, not at its end', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=empty`);
  await page.evaluate(() => (window.__harness.holdAfter = 1));
  await page.getByTestId('fetch').click();
  await expect(page.getByTestId('empty-all')).toHaveText('Die Jobs erscheinen hier nach und nach.');
  await page.evaluate(() => (window.__harness.holdAfter = null));
  await runFinished(page);
});
