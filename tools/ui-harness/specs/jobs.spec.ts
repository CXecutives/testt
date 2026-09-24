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
  await expect(page.getByTestId('must')).toHaveText('4 von 4 Muss erfüllt');
  await expect(page.getByTestId('contract')).toHaveText('Interim');
  await expect(
    page.getByTestId('criteria').locator('li:not([data-testid="contract"])'),
  ).toHaveCount(4);
  // The click marks the job read; wait until the list and the reader have taken that in (a
  // slow machine would otherwise re-render the reader under the pointer).
  await expect(top.locator('.title')).not.toHaveClass(/unread/);
  await expect(page.locator('.ad mark').first()).toBeAttached();
  await settle(page);

  const reason = page.getByTestId('reasons-met').getByRole('button').first();
  await reason.hover();
  await expect(page.getByRole('tooltip')).toContainText('im Profil');
  await expect(page.locator('mark.active')).toHaveCount(1);
  // A click scrolls the passage into view (smooth; retried until it has arrived).
  await expect(async () => {
    await reason.click();
    await expect(page.locator('mark.active')).toBeInViewport({ timeout: 2000 });
  }).toPass({ timeout: 10_000 });

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
  // One quiet icon button: its tooltip names the current order, a click switches it.
  const sort = page.getByTestId('sort');
  await expect(sort).toHaveAttribute('aria-label', 'Beste Passung zuerst');
  await sort.click();
  await expect(sort).toHaveAttribute('aria-label', 'Neueste zuerst');
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
  // The reason is the subline of the band, said once (no notice, no repeated violation).
  await expect(page.getByTestId('band')).toHaveText('Ausgeschlossen');
  const because = await page.getByTestId('exclusion').innerText();
  await expect(page.getByTestId('reader').getByText(because, { exact: true })).toHaveCount(1);
  await expect(page.getByTestId('criteria').locator('[data-state="violated"]')).toHaveCount(1);
  // ANÜ is one chip: the contract chip steps back behind the criterion of the same name.
  await expect(page.getByTestId('criteria').getByText('ANÜ', { exact: true })).toHaveCount(1);
});

test('the day overview tiles filter the list and match the counts', async ({ page }) => {
  await open(page, WIN);
  const overview = page.getByTestId('day-overview');
  await expect(overview).toBeVisible();
  const tileValue = async (id: string): Promise<number> =>
    Number((await page.getByTestId(id).innerText()).replace(/\D/g, ''));
  await expect.poll(() => tileValue('tile-high')).toBe(2);
  await page.getByTestId('tile-high').click();
  await expect(page.getByTestId('tile-high')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByTestId('filter')).toBeVisible();
  await expect(rows(page)).toHaveCount(await tileValue('tile-high'));
  await page.getByTestId('clear-filter').click();
  await page.getByTestId('tile-no-detail').click();
  await expect(rows(page)).toHaveCount(await tileValue('tile-no-detail'));
  await page.getByTestId('tile-excluded').click();
  await expect(excludedRows(page)).toHaveCount(await tileValue('tile-excluded'));
  // A second click on the active tile takes the filter off again.
  await page.getByTestId('tile-excluded').click();
  await expect(page.getByTestId('filter')).toHaveCount(0);
  await expect(page.getByTestId('issue-freelance-mails')).toBeVisible();

  // The overview does not repeat the list: no job rows, and "neu" per portal adds up to Neu.
  await expect(overview.locator('[data-testid^="job-row-"]')).toHaveCount(0);
  const perPortal = await page
    .getByTestId('new-per-portal')
    .locator('li')
    .evaluateAll((items) => items.map((item) => Number(item.textContent?.match(/\d+/)?.[0])));
  expect(perPortal.reduce((sum, value) => sum + value, 0)).toBe(await segmentCount(page, 'Neu'));
});

test('the reader summary agrees with the listed must requirements', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const keys = await rows(page).evaluateAll((els) => els.map((e) => e.getAttribute('data-testid')));
  let checked = 0;
  for (const key of keys) {
    const row = page.getByTestId(key!);
    await row.click();
    // Wait until the reader shows this job (a slow machine would still show the last one).
    await expect(page.getByTestId('reader-title')).toHaveText(
      await row.locator('.title').innerText(),
    );
    await expect(page.getByTestId('reader')).toHaveCount(1);
    const must = page.getByTestId('must');
    if ((await must.count()) === 0) continue;
    const text = await must.innerText();
    const numbers = text.match(/^(\d+) von (\d+)/);
    if (numbers === null) continue;
    const [met, total] = [Number(numbers[1]), Number(numbers[2])];
    const listed = await page.getByTestId('why').evaluate((why) => {
      // Muss is the default and carries no badge; only Kann does.
      const musts = [...why.querySelectorAll('li[data-weight="must"]')];
      if (musts.some((li) => li.querySelector('.badge') !== null)) return null;
      const kind = (li: Element): string =>
        li.querySelector('[role="img"]')?.getAttribute('aria-label') ?? '';
      return {
        met: musts.filter((li) => kind(li) === 'Erfüllt').length,
        partial: musts.filter((li) => kind(li) === 'Teilweise erfüllt').length,
        all: musts.length,
      };
    });
    if (listed === null) throw new Error(`${key}: a must requirement carries a badge`);
    expect(listed.met, `${key}: ${text}`).toBe(met);
    expect(listed.all, `${key}: ${text}`).toBe(total);
    expect(text.includes('teilweise'), `${key}: ${text}`).toBe(listed.partial > 0);
    checked += 1;
  }
  expect(checked).toBeGreaterThanOrEqual(8);
});

test('rows are mail-style: one height, one title line, a fixed dot gutter', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const first = row(page, 'freelancermap-2801');
  await expect(first).toContainText('Interim CFO für Familienunternehmen');
  await expect(first).not.toContainText('(m/w/d)');
  const title = await first.locator('.title').evaluate((node) => {
    const style = getComputedStyle(node);
    return [style.whiteSpace, style.textOverflow];
  });
  expect(title).toEqual(['nowrap', 'ellipsis']);
  // Every row, with or without badge, has the same height.
  const heights = await page
    .getByTestId('job-list')
    .locator('[data-testid^="job-row-"]')
    .evaluateAll((els) => [...new Set(els.map((e) => e.getBoundingClientRect().height))]);
  expect(heights).toEqual([86]);
  // Read and unread titles start at the same x: the dot lives in its own gutter.
  const lefts = await page
    .getByTestId('job-list')
    .locator('.title')
    .evaluateAll((els) => [...new Set(els.map((e) => Math.round(e.getBoundingClientRect().left)))]);
  expect(lefts).toHaveLength(1);
  await first.click();
  await expect(page.getByTestId('reader-title')).toHaveText('Interim CFO für Familienunternehmen');
});

test('the star pins from the list without opening the job', async ({ page }) => {
  await open(page, WIN);
  const key = 'linkedin-4100200301';
  const pin = page.getByTestId(`pin-${key}`);
  // The star shows on hover only (its wrapper fades), unless the job is pinned.
  const star = pin.locator('xpath=..');
  await expect(star).toHaveCSS('opacity', '0');
  await row(page, key).hover();
  await expect(star).toHaveCSS('opacity', '1');
  await pin.click();
  await expect(pin).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByTestId('reader')).toHaveCount(0);
  expect((await calls(page, 'set_pinned')).map(([, args]) => args)).toEqual([
    { key: { portal: 'linkedin', id: '4100200301' }, on: true },
  ]);
  await page.mouse.move(0, 0);
  await expect(star).toHaveCSS('opacity', '1');
});

test('no search hit: one empty state with a way back', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('search').fill('Kernfusion');
  await expect(page.getByTestId('empty-search')).toContainText('Keine Jobs zu „Kernfusion“.');
  expect(await visibleCount(page, '[data-testid^="empty-"]')).toBe(1);
  await page.getByTestId('empty-search').getByRole('button').click();
  await expect(rows(page).first()).toBeVisible();
});

test('without a profile: no rings, newest first, the overview leads to one', async ({ page }) => {
  await open(page, `${WIN}&scenario=no-profile`);
  // Said once, in the overview: the tiles Neu and Ohne Details and a calm card.
  await expect(page.getByTestId('no-profile')).toBeVisible();
  await expect
    .poll(async () => Number((await page.getByTestId('tile-new').innerText()).replace(/\D/g, '')))
    .toBe(await segmentCount(page, 'Neu'));
  await expect(page.getByTestId('tile-no-detail')).toBeVisible();
  await expect(page.getByTestId('tile-high')).toHaveCount(0);
  await expect(page.getByTestId('no-profile').locator('.btn.primary')).toHaveCount(0);
  await expect(page.getByTestId('sort')).toHaveCount(0);
  await expect(rows(page).first().locator('[role="img"][aria-label^="Passung"]')).toHaveCount(0);
  // Without a ring the reasons of an old match are not shown either, and the unread dot
  // keeps its gutter before the title.
  await expect(rows(page).first().locator('.reason')).toHaveCount(0);
  const gap = await rows(page)
    .first()
    .locator('xpath=..')
    .evaluate((row) => {
      const dot = row.querySelector('.dot')!.getBoundingClientRect();
      const title = row.querySelector('.title')!.getBoundingClientRect();
      return title.left - dot.right;
    });
  expect(gap).toBeGreaterThanOrEqual(10);
  const query = (await calls(page, 'list_jobs'))[0]?.[1] as { query: { sort: string } };
  expect(query.query.sort).toBe('newest');
  // The way on is the Profil view with its three ways in.
  await page.getByTestId('no-profile').getByRole('button').click();
  await expect(page.getByTestId('profile-empty')).toBeVisible();
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
  await expect(page.getByTestId('run-finished')).toContainText('Abruf abgebrochen');
});

// The stub delivers events like Tauri: every command call has its own channel with message
// indices from 0, and a channel whose Rust side is dropped unregisters on the page. A page
// that reused one channel heard nothing of the second run (user test of the installed app).
test('two runs in a row: both end in the idle state', async ({ page }) => {
  await open(page, `${WIN}&tick=15`);
  for (const round of [1, 2]) {
    await page.getByTestId('fetch').click();
    await expect(page.getByTestId('run-running'), `run ${round}`).toBeVisible();
    await runFinished(page);
    await expect(page.getByTestId('run-running'), `run ${round}`).toHaveCount(0);
    await expect(page.getByTestId('run-finished'), `run ${round}`).toContainText('Abruf fertig');
    await expect(page.getByTestId('fetch'), `run ${round}`).toBeVisible();
    await expect(page.getByTestId('run-status')).toHaveCount(0);
  }
  expect(await calls(page, 'start_run')).toHaveLength(2);
  // The sidebar is idle as well once the run card steps aside.
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('run-status')).toContainText('Zuletzt');
});

test('a removed mailbox keeps the jobs: Abrufen waits and the list says how', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('mailbox-remove').click();
  await page
    .getByTestId('dialog-remove-mailbox')
    .getByRole('button', { name: 'Entfernen' })
    .click();
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await expect(page.getByTestId('view-first-run')).toHaveCount(0);
  await expect(rows(page).first()).toBeVisible();
  await expect(page.getByTestId('fetch')).toHaveAttribute('aria-disabled', 'true');
  await page.getByTestId('no-mailbox').getByRole('button').click();
  await expect(page.getByTestId('view-settings')).toBeVisible();
});

test('a rescore is no fetch: no fetch texts, no run card afterwards', async ({ page }) => {
  await open(page, `${WIN}&tick=15`);
  await page.getByTestId('nav-profile').click();
  await page.evaluate(() => {
    window.__harness.emit({ type: 'status', code: 'scoring', portal: null, until: null });
    window.__harness.emit({
      type: 'finished',
      summary: {
        run: 42,
        kind: 'rescore',
        outcome: { kind: 'completed' },
        dryRun: false,
        startedAt: '2026-09-24T07:29:00Z',
        finishedAt: '2026-09-24T07:30:00Z',
        scan: null,
        perPortal: [],
        score: null,
        export: null,
        emptyAlerts: [],
      },
    });
  });
  await expect(page.getByTestId('toast')).toHaveCount(0);
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('run-card')).toHaveCount(0);
  await expect(page.getByText('Abruf fertig')).toHaveCount(0);
});

test('under reduced motion a run without progress still shows its bar', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, `${WIN}&tick=3000`);
  await page.getByTestId('fetch').click();
  const bar = page.getByTestId('run-running').getByRole('progressbar');
  await expect(bar).toBeVisible();
  const inside = await bar.evaluate((track) => {
    const fill = track.firstElementChild!.getBoundingClientRect();
    const box = track.getBoundingClientRect();
    return fill.width > 0 && fill.left >= box.left - 1 && fill.right <= box.right + 1;
  });
  expect(inside).toBe(true);
  await page.getByTestId('cancel-run').click();
  await runFinished(page);
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
  await expect(page.getByTestId('step-fetch')).toContainText('5 von 7');
  // The status names the portal it is about.
  await expect(page.getByTestId('run-running')).toContainText('Wartet auf LinkedIn');
  await expect(page.getByTestId('countdown')).toHaveText('Weiter in 0:42');
  await expect(page.getByTestId('pause-freelance')).toContainText(
    'Pause bis 09:42, das Portal bremst die Anfragen.',
  );
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
  await expect(page.getByTestId('pin-freelance-900411')).toHaveAttribute('aria-pressed', 'true');
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
  // Only the list work counts, not the start of the app: wait until the first window has
  // mounted completely and the main thread is idle, then measure the interactions.
  await expect
    .poll(async () => {
      const before = await rows(page).count();
      await settle(page);
      return before > 0 && before === (await rows(page).count());
    })
    .toBe(true);
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        'requestIdleCallback' in window
          ? requestIdleCallback(() => resolve(), { timeout: 2000 })
          : setTimeout(resolve, 200),
      ),
  );
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

test('baseline: jobs without a profile', async ({ page }) => {
  await open(page, `${WIN}&scenario=no-profile`);
  await expect(page.getByTestId('no-profile')).toBeVisible();
  await expectShot(page, 'jobs-no-profile');
});

test('baseline: jobs at the minimum size 480 x 360', async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 360 });
  await open(page, WIN);
  await expectShot(page, 'jobs-min');
});

test('baseline: a job at the minimum size 480 x 360', async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 360 });
  await open(page, WIN);
  await rows(page).first().click();
  await expect(page.getByTestId('reader-ring')).toContainText('91');
  await expectShot(page, 'jobs-min-reader');
});
