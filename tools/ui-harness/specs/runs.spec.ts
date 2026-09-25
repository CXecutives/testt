// Runs and their numbers against the stub: every run shows as its kind (also the ones the
// app starts by itself), the run card says what a run brought and what it could not write
// (once), and the list and the reader stay true while a run updates jobs.

import type { Page } from '@playwright/test';
import type { JobView } from '../../../ui/src/lib/ipc/types';
import { calls, expect, open, runFinished, settle, test } from './fixtures';

const WIN = '?platform=windows';
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
const row = (page: Page, key: string) => page.getByTestId('job-list').getByTestId(`job-row-${key}`);

async function segmentCount(page: Page, label: string): Promise<number> {
  const text = await page
    .getByTestId('facet')
    .getByRole('radio', { name: new RegExp(label) })
    .innerText();
  return Number(text.replace(/\D/g, ''));
}

/** Send run events the way the backend does (the page's channel when no run holds one). */
async function emit(page: Page, ...events: unknown[]): Promise<void> {
  await page.evaluate((list) => {
    for (const event of list) window.__harness.emit(event as never);
  }, events);
}

test('a rescore the app starts shows as a rescore and leaves the fetch card alone', async ({
  page,
}) => {
  await open(page, `${WIN}&tick=15`);
  await page.getByTestId('fetch').click();
  await runFinished(page);
  const card = page.getByTestId('run-card');
  await expect(card).toHaveAttribute('data-kind', 'fetch');
  await expect(page.getByTestId('run-finished')).toContainText('Abruf fertig');
  await expect(page.getByTestId('last-new')).toHaveText('2 neu');
  await page.getByTestId('run-history').getByRole('button').first().click();
  const history = await page.getByTestId('run-history').locator('li').count();

  // The profile changed: the app scores every job anew by itself, on the page's channel.
  await page.evaluate(() => {
    window.__harness.holdAfter = 2;
    window.__harness.appRun('rescore');
  });
  const fetch = page.getByTestId('fetch');
  await expect(fetch).toHaveAttribute('aria-disabled', 'true');
  await expect(page.getByTestId('cancel-run')).toHaveCount(0);
  await expect(page.getByTestId('run-running')).toHaveCount(0);
  await expect(card).toHaveAttribute('data-kind', 'fetch');
  await fetch.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Die Jobs werden gerade neu bewertet.');
  await page.evaluate(() => (window.__harness.holdAfter = null));
  await runFinished(page);

  // Afterwards the fetch card is unchanged and Abrufen is back.
  await expect(page.getByTestId('run-finished')).toContainText('Abruf fertig');
  await expect(page.getByTestId('last-new')).toHaveText('2 neu');
  await expect(page.getByTestId('run-history').locator('li')).toHaveCount(history);
  await expect(fetch).not.toHaveAttribute('aria-disabled', 'true');
  expect(await calls(page, 'start_run')).toHaveLength(1);
});

test('the auto fetch the app starts shows as a fetch', async ({ page }) => {
  await open(page, `${WIN}&tick=15`);
  await page.evaluate(() => {
    window.__harness.holdAfter = 3;
    window.__harness.appRun('fetch');
  });
  await expect(page.getByTestId('run-running')).toBeVisible();
  await expect(page.getByTestId('step-scan')).toBeVisible();
  await expect(page.getByTestId('cancel-run')).toBeVisible();
  await page.evaluate(() => (window.__harness.holdAfter = null));
  await runFinished(page);
  await expect(page.getByTestId('run-finished')).toContainText('Abruf fertig');
});

test('a rescore that cannot write the files says so once, with a retry', async ({ page }) => {
  await open(page, `${WIN}&tick=15&export=locked`);
  await page.evaluate(() => window.__harness.appRun('rescore'));
  await runFinished(page);
  const card = page.getByTestId('run-card');
  await expect(card).toHaveAttribute('data-kind', 'rescore');
  await expect(page.getByTestId('run-finished')).toContainText('Neu bewertet');
  const note = page.getByTestId('export-failed');
  await expect(note).toHaveCount(1);
  await expect(note).toContainText(
    'Die Excel-Datei ist in einem anderen Programm geöffnet und blieb unverändert.',
  );
  await expect(page.getByText('blieb unverändert')).toHaveCount(1);
  await note.getByRole('button', { name: 'Erneut versuchen' }).click();
  await runFinished(page);
  const started = await calls(page, 'start_run');
  expect((started.at(-1)?.[1] as { request: unknown }).request).toEqual({ kind: 'rescore' });
});

test('a fetch that cannot write the Excel file says so once, and the toast too', async ({
  page,
}) => {
  await open(page, `${WIN}&tick=15&export=locked`);
  await page.getByTestId('fetch').click();
  // Elsewhere a toast brings the news, not "done" alone.
  await page.getByTestId('nav-settings').click();
  await runFinished(page);
  await expect(page.getByTestId('toast')).toHaveText(
    'Abruf fertig, die Dateien sind nicht aktuell.',
  );
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('run-finished')).toContainText('Abruf fertig');
  await expect(page.getByTestId('export-failed')).toHaveCount(1);
  await expect(page.getByText('blieb unverändert')).toHaveCount(1);
  // The numbers of the run still stand next to it.
  await expect(page.getByTestId('last-new')).toHaveText('2 neu');
  await expect(page.getByTestId('last-top')).toHaveText('1 passt gut');
});

test('the run card counts the run: new and not excluded, high among those', async ({ page }) => {
  await open(page, `${WIN}&tick=15`);
  await page.getByTestId('fetch').click();
  await runFinished(page);
  // Three new jobs came in, one of them excluded: two new, one fits well.
  await expect(page.getByTestId('last-new')).toHaveText('2 neu');
  await expect(page.getByTestId('last-top')).toHaveText('1 passt gut');
  await expect(page.getByTestId('nothing-new')).toHaveCount(0);
});

test('Details holen reports the details, not a fetch', async ({ page }) => {
  await open(page, `${WIN}&tick=40`);
  await row(page, 'linkedin-4100200302').click();
  await expect(page.getByTestId('detail-note')).toBeVisible();
  await page.evaluate(() => (window.__harness.holdAfter = 3));
  await page.getByTestId('fetch-details').click();
  const card = page.getByTestId('run-card');
  await expect(card).toHaveAttribute('data-kind', 'details');
  await expect(page.getByTestId('step-fetch')).toBeVisible();
  await expect(page.getByTestId('step-score')).toBeVisible();
  await expect(page.getByTestId('step-scan')).toHaveCount(0);
  await page.evaluate(() => (window.__harness.holdAfter = null));
  await runFinished(page);
  await expect(page.getByTestId('run-finished')).toContainText('Details geholt');
  await expect(card).not.toContainText('Abruf');
  await expect(page.getByTestId('last-new')).toHaveCount(0);
  await expect(page.getByTestId('nothing-new')).toHaveCount(0);
  // The reader has the details now; the last fetch is still the last fetch.
  await expect(page.getByTestId('detail-note')).toHaveCount(0);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('run-status')).toContainText('Abgerufen 08:30');
});

test('a failed first fetch does not claim the alert mails were empty', async ({ page }) => {
  await open(page, `${WIN}&scenario=mailbox-only&mail=offline&tick=15`);
  await expect(page.getByTestId('view-first-run')).toBeVisible();
  await page.getByTestId('first-fetch').click();
  await runFinished(page);
  await expect(page.getByTestId('run-failed')).toContainText('Gmail ist nicht erreichbar.');
  await expect(page.getByTestId('empty-all')).toContainText(
    'Nach dem ersten Abruf stehen die Jobs hier.',
  );
  await expect(page.getByText('enthielten bisher keine Jobs')).toHaveCount(0);
});

test('a start that fails keeps the last result and says why', async ({ page }) => {
  await open(page, `${WIN}&tick=15`);
  await page.getByTestId('fetch').click();
  await runFinished(page);
  await expect(page.getByTestId('last-new')).toHaveText('2 neu');
  // A run the page has not heard of yet holds the slot: start_run answers "busy".
  await page.evaluate(() => {
    window.__harness.holdAfter = 0;
    window.__harness.appRun('rescore');
  });
  await page.getByTestId('fetch').click();
  await expect(page.getByTestId('start-error')).toHaveText('Gerade läuft schon ein Abruf.');
  await expect(page.getByTestId('run-finished')).toContainText('Abruf fertig');
  await expect(page.getByTestId('last-new')).toHaveText('2 neu');
  await page.evaluate(() => (window.__harness.holdAfter = null));
  await runFinished(page);
});

test('the reader stays with the selected job while a run updates the one before', async ({
  page,
}) => {
  await open(page, WIN);
  await row(page, 'linkedin-4100200301').click();
  await expect(page.getByTestId('reader-title')).toHaveText('Head of Controlling Transformation');
  await page.evaluate(() => (window.__harness.detailDelay = 400));
  const before = (await calls(page, 'job_detail')).length;
  const a = await jobOf(page, 'linkedin', '4100200301');
  await row(page, 'freelancermap-2802').click();
  // While B loads, a run update of A arrives.
  await emit(page, {
    type: 'jobUpdated',
    job: a,
    fresh: false,
  });
  await expect(page.getByTestId('reader-title')).toHaveText('Interim Head of Finance');
  await page.waitForTimeout(900);
  await expect(page.getByTestId('reader-title')).toHaveText('Interim Head of Finance');
  const asked = (await calls(page, 'job_detail')).slice(before).map(([, args]) => args);
  expect(asked).toEqual([{ key: { portal: 'freelancermap', id: '2802' } }]);
});

test('a run update of a job beyond the loaded page is no new row', async ({ page }) => {
  await open(page, `${WIN}&scenario=many`);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(page.getByTestId('facet').getByRole('radio', { name: /Alle/ })).toContainText(
    '2.000',
  );
  const first = await rows(page).first().getAttribute('data-testid');
  const newBefore = await segmentCount(page, 'Neu');
  // Job 100000 scores 0 and sorts far beyond the first page of 500.
  const far = await jobOf(page, 'linkedin', '100000');
  await emit(page, {
    type: 'jobUpdated',
    job: { ...far, match: { ...far.match!, score: 35, band: 'low' } },
    fresh: false,
  });
  await page.waitForTimeout(600);
  await settle(page);
  await expect(rows(page).first()).toHaveAttribute('data-testid', first!);
  await expect(page.getByTestId('job-row-linkedin-100000')).toHaveCount(0);
  expect(await segmentCount(page, 'Alle')).toBe(2000);
  expect(await segmentCount(page, 'Neu')).toBe(newBefore);

  // A job new in the run comes in at the top, and the counts follow the backend.
  await emit(page, {
    type: 'jobUpdated',
    job: { ...far, key: { portal: 'linkedin', id: '999999' }, title: 'Neu im Lauf' },
    fresh: true,
  });
  await expect(rows(page).first()).toHaveAttribute('data-testid', 'job-row-linkedin-999999');
  await expect.poll(() => segmentCount(page, 'Alle')).toBe(2001);
});

test('a page that fails while scrolling says so and loads on retry', async ({ page }) => {
  await open(page, `${WIN}&scenario=many`);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(page.getByTestId('facet').getByRole('radio', { name: /Alle/ })).toContainText(
    '2.000',
  );
  await expect(rows(page).first()).toBeVisible();
  await page.evaluate(() => (window.__harness.failPages = 1));
  // Scroll window by window to the end of the first page of 500; the next page fails.
  const scroller = page.getByTestId('list-scroll');
  const error = page.getByTestId('page-error');
  for (let i = 0; i < 40 && (await error.count()) === 0; i += 1) {
    await scroller.evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
    await page.waitForTimeout(250);
  }
  await expect(page.getByTestId('page-error')).toContainText(
    'Weitere Jobs ließen sich nicht laden.',
  );
  const mounted = await page.locator('[data-testid^="job-row-"]').count();
  await page.getByTestId('page-error').getByRole('button', { name: 'Erneut versuchen' }).click();
  await expect(page.getByTestId('page-error')).toHaveCount(0);
  await expect
    .poll(() => page.locator('[data-testid^="job-row-"]').count())
    .toBeGreaterThan(mounted);
});

test('the divider under Neu names no number; under Alle the one of every excluded job', async ({
  page,
}) => {
  await open(page, WIN);
  await expect(page.getByTestId('facet').getByRole('radio', { name: /Neu/ })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(page.getByTestId('excluded-divider')).toHaveText('Ausgeschlossen');
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(page.getByTestId('excluded-count')).toBeVisible();
  await expect(page.getByTestId('excluded-divider')).toHaveText(
    `Ausgeschlossen ${await page.getByTestId('excluded-rows').locator('[data-testid^="job-row-"]').count()}`,
  );
});

test('the portals follow the one order of the app', async ({ page }) => {
  await open(page, `${WIN}&scenario=empty`);
  const order = await page
    .locator('[data-testid^="alert-"]')
    .evaluateAll((items) => items.map((item) => item.getAttribute('data-testid')));
  expect(order).toEqual(['alert-linkedin', 'alert-freelance', 'alert-freelancermap']);
  await page.getByTestId('nav-settings').click();
  const cards = await page
    .locator('[data-testid^="portal-"]')
    .evaluateAll((items) => items.map((item) => item.getAttribute('data-testid')));
  expect(cards.filter((id) => /^portal-[a-z]+$/.test(id ?? ''))).toEqual([
    'portal-linkedin',
    'portal-freelance',
    'portal-freelancermap',
  ]);
});

/** A job as the stub holds it. */
async function jobOf(page: Page, portal: JobView['portal'], id: string): Promise<JobView> {
  const job = await page.evaluate((key) => window.__harness.job(key), { portal, id });
  expect(job, `${portal}-${id}`).not.toBeNull();
  return job!;
}

test('an archived job leaves the list and every count but the archive', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const all = await segmentCount(page, 'Alle');
  const link = page.getByTestId('show-archive');
  const archived = Number((await link.innerText()).replace(/\D/g, ''));
  await row(page, 'linkedin-4100200301').click();
  await page.getByTestId('hide').click();
  await expect(row(page, 'linkedin-4100200301')).toHaveCount(0);
  await expect.poll(() => segmentCount(page, 'Alle')).toBe(all - 1);
  await expect(link).toContainText(String(archived + 1));
  expect((await calls(page, 'move_jobs')).map(([, args]) => args)).toEqual([
    { keys: [{ portal: 'linkedin', id: '4100200301' }], to: 'archive' },
  ]);
  expect((await jobOf(page, 'linkedin', '4100200301')).place).toBe('archive');
  // The archive lists it.
  await link.click();
  await expect(row(page, 'linkedin-4100200301')).toHaveCount(1);
});
