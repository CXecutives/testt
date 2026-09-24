// The Jobs view against the stub: the core workflow, list and counts, the reader, the day
// overview and every state of the list.

import type { Page } from '@playwright/test';
import {
  calls,
  expect,
  expectShot,
  open,
  runFinished,
  settle,
  test,
  visibleCount,
} from './fixtures';

const WIN = '?platform=windows';
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
const excludedRows = (page: Page) =>
  page.getByTestId('excluded-rows').locator('[data-testid^="job-row-"]');

const row = (page: Page, key: string) => page.getByTestId('job-list').getByTestId(`job-row-${key}`);

async function segmentCount(page: Page, label: string): Promise<number> {
  const text = await page
    .getByTestId('facet')
    .getByRole('radio', { name: new RegExp(label) })
    .innerText();
  return Number(text.replace(/\D/g, ''));
}

test('core workflow: fetch, rings fill, open the best job, reasons light the ad', async ({
  page,
}) => {
  await open(page, `${WIN}&tick=30`);
  await page.getByTestId('fetch').click();
  await expect(page.getByTestId('run-running')).toBeVisible();
  await expect(page.getByTestId('cancel-run')).toBeVisible();
  await runFinished(page);
  await expect(page.getByTestId('run-finished')).toBeVisible();
  await expect(page.getByTestId('fetch')).toBeVisible();

  // The new job with the best score is scored live and sorted to the top once finished.
  const top = rows(page).first();
  await expect(top).toHaveAttribute('data-testid', 'job-row-freelancermap-2801');
  await expect(row(page, 'linkedin-4100200399').getByRole('img').first()).toHaveAttribute(
    'aria-label',
    /Passung 88/,
  );

  await top.click();
  await expect(page.getByTestId('reader')).toBeVisible();
  await expect(page.getByTestId('band')).toHaveText('Hohe Passung');
  await expect(page.getByTestId('must')).toHaveText('4 von 4 Muss-Anforderungen erfüllt');
  await expect(page.getByTestId('criteria').locator('li')).toHaveCount(4);

  const reason = page.getByTestId('reasons-met').getByRole('button').first();
  await reason.hover();
  await expect(page.getByRole('tooltip')).toContainText('im Profil');
  await expect(page.locator('mark.active')).toHaveCount(1);
  await reason.click();
  await expect(page.locator('mark.active')).toBeInViewport();

  await page.getByTestId('open-ad').click();
  const opened = await calls(page, 'open_target');
  expect(opened.at(-1)?.[1]).toEqual({
    target: { kind: 'jobUrl', key: { portal: 'freelancermap', id: '2801' } },
  });
});

test('counts equal the list, with and without search', async ({ page }) => {
  await open(page, WIN);
  expect(await rows(page).count()).toBe(await segmentCount(page, 'Neu'));
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(page.getByTestId('excluded-divider')).toBeVisible();
  const all = (await rows(page).count()) + (await excludedRows(page).count());
  expect(all).toBe(await segmentCount(page, 'Alle'));
  await expect(page.getByTestId('excluded-divider')).toHaveText(
    `Ausgeschlossen ${await excludedRows(page).count()}`,
  );

  await page.getByTestId('search').fill('Interim');
  await expect.poll(() => segmentCount(page, 'Alle')).toBe(3);
  await expect(rows(page)).toHaveCount(3);
  await page.getByTestId('facet').getByRole('radio', { name: /Neu/ }).click();
  await expect
    .poll(async () => (await rows(page).count()) === (await segmentCount(page, 'Neu')))
    .toBe(true);
});

test('mark_read only on a real click, and only once', async ({ page }) => {
  await open(page, WIN);
  expect(await calls(page, 'mark_read')).toHaveLength(0);
  const first = row(page, 'linkedin-4100200301');
  await first.click();
  await expect(page.getByTestId('reader')).toBeVisible();
  await row(page, 'freelancermap-2802').click();
  await first.click();
  await expect(page.getByTestId('reader-title')).toHaveText('Head of Controlling Transformation');
  const marked = await calls(page, 'mark_read');
  expect(marked.map(([, args]) => args)).toEqual([
    { key: { portal: 'linkedin', id: '4100200301' } },
    { key: { portal: 'freelancermap', id: '2802' } },
  ]);
  // Neu counts down, the row stays until the list reloads.
  expect(await segmentCount(page, 'Neu')).toBe(4);
  await expect(first).toBeVisible();
});

test('the sort switch reorders the list and keeps the selection', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await row(page, 'freelancermap-2803').click();
  const before = await rows(page).evaluateAll((els) =>
    els.map((e) => e.getAttribute('data-testid')),
  );
  await page.getByTestId('sort').getByRole('radio', { name: 'Neueste' }).click();
  await expect
    .poll(() => rows(page).evaluateAll((els) => els.map((e) => e.getAttribute('data-testid'))))
    .not.toEqual(before);
  await expect(row(page, 'freelancermap-2803')).toHaveAttribute('aria-current', 'true');
  await expect(page.getByTestId('reader-title')).toHaveText(
    'Kaufmännische Leitung Projektgeschäft',
  );
});

test('excluded jobs sit grey behind the divider and explain themselves', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const excludedRow = excludedRows(page).first();
  await expect(excludedRow).toContainText('Arbeitnehmerüberlassung');
  await excludedRow.click();
  await expect(page.getByTestId('exclusion')).toBeVisible();
  await expect(page.getByTestId('band')).toHaveText('Ausgeschlossen');
  await expect(page.getByTestId('criteria').locator('[data-state="violated"]')).toHaveCount(1);
});

test('the day overview tiles filter the list and match the counts', async ({ page }) => {
  await open(page, WIN);
  await expect(page.getByTestId('day-overview')).toBeVisible();
  await page.getByTestId('tile-high').click();
  await expect(page.getByTestId('filter')).toBeVisible();
  await expect(rows(page)).toHaveCount(2);
  await page.getByTestId('clear-filter').click();
  await page.getByTestId('tile-no-detail').click();
  await expect(rows(page)).toHaveCount(3);
  await page.getByTestId('tile-excluded').click();
  await expect(excludedRows(page)).toHaveCount(2);
  await page.getByTestId('clear-filter').click();
  await expect(page.getByTestId('issue-alert')).toBeVisible();
});

test('no search hit: one empty state with a way back', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('search').fill('Kernfusion');
  await expect(page.getByTestId('empty-search')).toContainText('Keine Jobs zu „Kernfusion“.');
  expect(await visibleCount(page, '[data-testid^="empty-"]')).toBe(1);
  await page.getByTestId('empty-search').getByRole('button').click();
  await expect(rows(page).first()).toBeVisible();
});

test('without a profile: no rings, newest first, a note to choose one', async ({ page }) => {
  await open(page, `${WIN}&scenario=no-profile`);
  await expect(page.getByTestId('no-profile')).toBeVisible();
  await expect(page.getByTestId('sort')).toHaveCount(0);
  await expect(rows(page).first().locator('[role="img"][aria-label^="Passung"]')).toHaveCount(0);
  const query = (await calls(page, 'list_jobs'))[0]?.[1] as { query: { sort: string } };
  expect(query.query.sort).toBe('newest');
  await page.getByTestId('no-profile').getByRole('button').click();
  await expect(page.getByTestId('sort')).toBeVisible();
  await expect(page.getByTestId('no-profile')).toHaveCount(0);
});

test('an empty list and a first fetch without news', async ({ page }) => {
  await open(page, `${WIN}&scenario=empty`);
  await expect(page.getByTestId('empty-all')).toBeVisible();
  expect(await visibleCount(page, '[data-testid^="empty-"]')).toBe(1);
  await expect(page.getByTestId('run-status')).toContainText('Zuletzt');
});

test('a run can be cancelled', async ({ page }) => {
  await open(page, `${WIN}&tick=200`);
  await page.getByTestId('fetch').click();
  await page.getByTestId('cancel-run').click();
  await runFinished(page);
  await expect(page.getByTestId('run-finished')).toContainText('Der Abruf wurde abgebrochen.');
});

test('offline: the failed run says why and offers a retry', async ({ page }) => {
  await open(page, `${WIN}&scenario=offline`);
  const failed = page.getByTestId('run-failed');
  await expect(failed).toContainText('Gmail ist nicht erreichbar.');
  await failed.getByRole('button', { name: 'Erneut versuchen' }).click();
  await runFinished(page);
  expect(await calls(page, 'start_run')).toHaveLength(1);
});

test('a run in progress after a reload: steps, portals, countdown and pause', async ({ page }) => {
  await open(page, `${WIN}&scenario=running`);
  await expect(page.getByTestId('step-scan')).toHaveClass(/done/);
  await expect(page.getByTestId('step-fetch')).toHaveClass(/current/);
  await expect(page.getByTestId('run-portal-linkedin')).toContainText('3 von 5');
  await expect(page.getByTestId('countdown')).toHaveText('Weiter in 0:42');
  await expect(page.getByTestId('pause-freelance')).toContainText('Pause bis 09:42.');
});

test('loading takes a moment: skeletons, then the list', async ({ page }) => {
  await open(page, `${WIN}&scenario=slow`);
  await expect(page.getByTestId('list-skeleton')).toBeVisible();
  await expect(rows(page).first()).toBeVisible({ timeout: 5000 });
  await expect(page.getByTestId('list-skeleton')).toHaveCount(0);
});

test('a failing list offers a retry', async ({ page }) => {
  await open(page, `${WIN}&scenario=list-error`);
  await expect(page.getByTestId('list-error')).toContainText('Die Datenbank meldet einen Fehler.');
});

test('details and pins: teaser note, fetch details, pin star', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await row(page, 'freelance-900411').click();
  await expect(page.getByTestId('detail-note')).toContainText('Anriss');
  await page.getByTestId('fetch-details').click();
  const started = await calls(page, 'start_run');
  expect((started[0]?.[1] as { request: unknown }).request).toEqual({
    kind: 'details',
    keys: [{ portal: 'freelance', id: '900411' }],
  });
  await runFinished(page);
  await page.getByTestId('pin').click();
  await expect(page.getByTestId('pin')).toHaveAttribute('aria-pressed', 'true');
  await expect(row(page, 'freelance-900411').getByRole('img', { name: 'Gemerkt' })).toBeVisible();
});

test('2000 jobs render in windows without long tasks', async ({ page, browserName }) => {
  await page.addInitScript(() => {
    (window as unknown as { __long: number[][] }).__long = [];
    if (PerformanceObserver.supportedEntryTypes?.includes('longtask')) {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          (window as unknown as { __long: number[][] }).__long.push([
            entry.startTime,
            entry.duration,
          ]);
        }
      }).observe({ type: 'longtask', buffered: true });
    }
  });
  await open(page, `${WIN}&scenario=many`);
  await expect(rows(page).first()).toBeVisible();
  // Only the list work counts, not the start of the app.
  const since = await page.evaluate(() => performance.now());
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(page.getByTestId('facet').getByRole('radio', { name: /Alle/ })).toContainText(
    '2.000',
  );
  const count = await page.locator('[data-testid^="job-row-"]').count();
  expect(count).toBeLessThanOrEqual(70);
  for (let i = 0; i < 4; i += 1) {
    await page
      .getByTestId('list-scroll')
      .evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
    await page.waitForTimeout(150);
  }
  expect(await page.locator('[data-testid^="job-row-"]').count()).toBeGreaterThan(count);
  if (browserName === 'chromium') {
    const long = await page.evaluate(() => (window as unknown as { __long: number[][] }).__long);
    expect(
      long.filter(([start, d]) => start! > since && d! > 50),
      JSON.stringify(long),
    ).toEqual([]);
  }
});

test('below 900 px one column with a back button', async ({ page }) => {
  await page.setViewportSize({ width: 780, height: 560 });
  await open(page, WIN);
  await expect(page.getByTestId('reader-pane')).toBeHidden();
  await rows(page).first().click();
  await expect(page.getByTestId('reader')).toBeVisible();
  await expect(page.getByTestId('list-scroll')).toBeHidden();
  await page.getByTestId('back').click();
  await expect(page.getByTestId('list-scroll')).toBeVisible();
});

test('reduced motion: rings show their value at once', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, WIN);
  await rows(page).first().click();
  await expect(page.getByTestId('reader-ring')).toContainText('91', { timeout: 300 });
});

test('one primary button in every state of the Jobs view', async ({ page }) => {
  for (const scenario of ['default', 'empty', 'no-profile', 'offline']) {
    await open(page, `${WIN}&scenario=${scenario}`);
    expect(await visibleCount(page, '.btn.primary'), scenario).toBeLessThanOrEqual(1);
    await settle(page);
  }
  await open(page, WIN);
  await rows(page).first().click();
  expect(await visibleCount(page, '.btn.primary')).toBeLessThanOrEqual(1);
});

test('baseline: jobs with the day overview', async ({ page }) => {
  await open(page, WIN);
  await expectShot(page, 'jobs-overview');
});

test('baseline: jobs with the reader', async ({ page }) => {
  await open(page, WIN);
  await rows(page).first().click();
  await expect(page.getByTestId('reader-ring')).toContainText('91');
  await expectShot(page, 'jobs-reader');
});

test('baseline: jobs while a run is going', async ({ page }) => {
  await open(page, `${WIN}&scenario=running`);
  await expectShot(page, 'jobs-running');
});

test('baseline: an excluded job', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await excludedRows(page).first().click();
  await expect(page.getByTestId('exclusion')).toBeVisible();
  await expectShot(page, 'jobs-excluded');
});

test('baseline: jobs at 780 x 560', async ({ page }) => {
  await page.setViewportSize({ width: 780, height: 560 });
  await open(page, WIN);
  await expectShot(page, 'jobs-narrow');
});
