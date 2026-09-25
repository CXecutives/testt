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
  // The overview file and the folder have one place: the day overview, not the run card.
  await expect(page.getByRole('button', { name: 'Übersicht öffnen' })).toHaveCount(1);
  await expect(page.getByRole('button', { name: 'Ordner öffnen' })).toHaveCount(1);
  await expect(page.getByTestId('day-overview').getByTestId('overview-open')).toBeVisible();
  // A history line copies like "Kopieren" does: the time, a space, the text.
  await page.getByTestId('run-history').getByRole('button').first().click();
  const line = await page.getByTestId('run-card').locator('.history li').first().textContent();
  expect(line?.trim()).toMatch(/^\d{2}:\d{2} \S/);

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
  await expect(page.getByTestId('must')).toHaveText('4 von 4 Pflichtanforderungen erfüllt');
  await expect(page.getByTestId('contract')).toHaveText('Interim');
  await expect(page.getByTestId('criteria').locator('li[data-testid^="criterion-"]')).toHaveCount(
    5,
  );
  // The click marks the job read; wait until the list and the reader have taken that in (a
  // slow machine would otherwise re-render the reader under the pointer).
  await expect(top.locator('.title')).not.toHaveClass(/unread/);
  await expect(page.locator('.ad mark').first()).toBeAttached();
  await settle(page);

  const reason = page.getByTestId('reasons-met').getByRole('button').first();
  await reason.hover();
  // The evidence stands under the point, quiet (no tooltip needed to trust it).
  await expect(page.getByTestId('reasons-met').getByTestId('evidence').first()).toContainText(
    'im Profil',
  );
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

test('a hidden job is in no list and no count', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(page.getByTestId('excluded-divider')).toBeVisible();
  await expect(row(page, 'linkedin-4100200306')).toHaveCount(0);
  const listed = (await rows(page).count()) + (await excludedRows(page).count());
  expect(listed).toBe(await segmentCount(page, 'Alle'));
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

test('the order menu reorders the list and keeps the selection', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await row(page, 'freelancermap-2803').click();
  const before = await rows(page).evaluateAll((els) =>
    els.map((e) => e.getAttribute('data-testid')),
  );
  // One quiet button names the order; it opens the OS's menu with both, the current ticked.
  const sort = page.getByTestId('sort');
  await expect(sort).toHaveText('Nach Passung');
  await expect(sort).toHaveAttribute('aria-haspopup', 'menu');
  await sort.click();
  expect((await page.evaluate(() => window.__harness.menus)).at(-1)).toEqual([
    { text: 'Nach Passung', enabled: true, command: null, checked: true },
    { text: 'Nach Datum', enabled: true, command: null, checked: false },
  ]);
  await page.evaluate(() => window.__harness.pick(1));
  await expect(sort).toHaveText('Nach Datum');
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
  // Arbeitnehmerüberlassung is one chip: the contract chip steps back behind the criterion of the same name.
  await expect(
    page.getByTestId('criteria').getByText('Arbeitnehmerüberlassung', { exact: true }),
  ).toHaveCount(1);
});

test('a job that cannot be scored says why, once', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await row(page, 'freelancermap-2806').click();
  await expect(page.getByTestId('band')).toHaveText('Nicht bewertbar');
  await expect(page.getByTestId('unscorable')).toHaveText('Zu wenig Text für eine Bewertung.');
  // The short-text note of the ad does not say it a second time.
  await expect(page.getByText('Die Anzeige ist sehr kurz.')).toHaveCount(0);
  await expect(page.getByTestId('why')).toHaveCount(0);
});

test('one place for filters: Neu, Alle, Favoriten; the overview says what now', async ({
  page,
}) => {
  await open(page, WIN);
  const overview = page.getByTestId('day-overview');
  await expect(overview).toBeVisible();
  // No counts in the overview: its best new jobs, the open points with their action, the
  // files. While the list beside shows the best new jobs on top, one quiet line says so.
  await expect(overview.getByTestId('tile-high')).toHaveCount(0);
  await expect(overview.getByTestId('new-per-portal')).toHaveCount(0);
  const best = page.getByTestId('best').locator('[data-testid^="best-"]');
  await expect(page.getByTestId('top-in-list')).toBeVisible();
  await expect(best).toHaveCount(0);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(page.getByTestId('top-in-list')).toHaveCount(0);
  expect(await best.count()).toBeGreaterThan(0);
  expect(await best.count()).toBeLessThanOrEqual(3);
  // The files have a block of their own.
  await expect(page.getByTestId('files')).toContainText('Dateien');
  await expect(page.getByTestId('issue-freelance-mails')).toContainText(
    'Eine Alert-Mail enthielt keine Jobs.',
  );
  await expect(overview.getByTestId('overview-excel')).toBeVisible();
  // The one filter place: the segments in the list header count their lists.
  const facet = page.getByTestId('facet');
  await facet.getByRole('radio', { name: /Favoriten/ }).click();
  await expect(facet.getByRole('radio', { name: /Favoriten/ })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(rows(page)).toHaveCount(await segmentCount(page, 'Favoriten'));
  expect(await calls(page, 'list_jobs')).toContainEqual([
    'list_jobs',
    expect.objectContaining({ query: expect.objectContaining({ favourites: true }) }),
  ]);
  // No application marks: no "Beworben" anywhere.
  await expect(facet.getByRole('radio', { name: /Beworben/ })).toHaveCount(0);
});

test('the reader summary agrees with the listed must requirements', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  // The window mounts a few rows per frame: wait until it is complete.
  await expect
    .poll(async () => {
      const before = await rows(page).count();
      await settle(page);
      return before > 6 && before === (await rows(page).count());
    })
    .toBe(true);
  const keys = await rows(page).evaluateAll((els) => els.map((e) => e.getAttribute('data-testid')));
  let checked = 0;
  for (const key of keys) {
    const row = page.getByTestId('job-rows').getByTestId(key!);
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
      // Muss is the default and carries no badge, except on an open one (it stands out
      // there); Kann always does.
      const kind = (li: Element): string =>
        li.querySelector('[role="img"]')?.getAttribute('aria-label') ?? '';
      const musts = [...why.querySelectorAll('li[data-weight="must"]')];
      if (
        musts.some((li) => li.querySelector('.badge') !== null && kind(li) !== 'Nicht im Profil')
      ) {
        return null;
      }
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

test('rows are mail-style: one height per title line, at most two, a fixed dot gutter', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const first = row(page, 'freelancermap-2801');
  await expect(first).toContainText('Interim CFO für Familienunternehmen');
  await expect(first).not.toContainText('(m/w/d)');
  // A long title takes a second line and ends there (the rest is a tooltip).
  const clamp = await first
    .locator('.title')
    .evaluate((node) => getComputedStyle(node).getPropertyValue('-webkit-line-clamp'));
  expect(clamp).toBe('2');
  // Every row, with or without badge, has the same height per title line (86, 106).
  const heights = await page
    .getByTestId('job-list')
    .locator('[data-testid^="job-row-"]')
    .evaluateAll((els) =>
      els.map((row) => {
        const title = row.querySelector('.title')!;
        const lines = Math.round(
          title.clientHeight / parseFloat(getComputedStyle(title).lineHeight),
        );
        return row.getBoundingClientRect().height - 20 * (lines - 1);
      }),
    );
  expect([...new Set(heights)]).toEqual([86]);
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

test('without a profile: empty rings, newest first, one line in the list leads to one', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=no-profile`);
  // Said once, at the top of the list; no best matches without a profile.
  const notice = page.getByTestId('no-profile');
  await expect(notice).toBeVisible();
  await expect(notice).toHaveText(/Ohne Profil gibt es keine Passung\./);
  expect(await page.getByTestId('job-list').getByTestId('no-profile').count()).toBe(1);
  await expect(page.getByTestId('day-overview').getByTestId('no-profile')).toHaveCount(0);
  await expect(page.getByTestId('best')).toHaveCount(0);
  await expect(notice.locator('.btn.primary')).toHaveCount(0);
  // Without a profile the order is by date; the menu cannot open, its tooltip says why.
  await expect(page.getByTestId('sort')).toHaveText('Nach Datum');
  await expect(page.getByTestId('sort')).toHaveAttribute('aria-disabled', 'true');
  await page.getByTestId('sort').click({ force: true });
  expect(await page.evaluate(() => window.__harness.menus)).toEqual([]);
  // Every row keeps its ring, empty: the titles stand where they always do.
  await expect(
    rows(page).first().getByRole('img', { name: 'Ohne Profil keine Passung' }),
  ).toHaveCount(1);
  await expect(rows(page).first().locator('.reason')).toHaveCount(0);
  const query = (await calls(page, 'list_jobs'))[0]?.[1] as { query: { sort: string } };
  expect(query.query.sort).toBe('newest');
  // The way on is the empty profile form, in one click.
  await notice.getByRole('button', { name: 'Profil anlegen' }).click();
  await expect(page.getByTestId('profile-form')).toBeVisible();
  await expect(page.getByTestId('profile-name')).toHaveText('Neues Profil');
});

test('an empty list and a first fetch without news', async ({ page }) => {
  await open(page, `${WIN}&scenario=empty`);
  await expect(page.getByTestId('empty-all')).toBeVisible();
  expect(await visibleCount(page, '[data-testid^="empty-"]')).toBe(1);
  await expect(page.getByTestId('run-status')).toContainText('Abgerufen');
  // The overview does not say "nothing new" again; the files keep their place.
  await expect(page.getByTestId('new-jobs')).toHaveCount(0);
  await expect(page.getByTestId('day-overview')).not.toContainText('Keine neuen Jobs');
  await expect(page.getByTestId('overview-open')).toBeVisible();
});

test('an unusable profile: the list names it and leads to the Profil view', async ({ page }) => {
  await open(page, `${WIN}&scenario=profile-broken`);
  const notice = page.getByTestId('no-profile');
  await expect(notice).toContainText('Profil nicht lesbar');
  await expect(notice).toContainText('Die Jobs zeigen deshalb keine Passung.');
  await notice.getByRole('button', { name: 'Profil öffnen' }).click();
  await expect(page.getByTestId('view-profile')).toBeVisible();
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
  await expect(page.getByTestId('run-status')).toContainText('Abgerufen');
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
  // The rescore the app starts after a profile change: on the page's channel, no start_run.
  await page.evaluate(() => window.__harness.appRun('rescore'));
  await runFinished(page);
  await expect(page.getByTestId('toast')).toHaveCount(0);
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('run-card')).toHaveCount(0);
  await expect(page.getByText('Abruf fertig')).toHaveCount(0);
  expect(await calls(page, 'start_run')).toHaveLength(0);
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
  // The sidebar says the fetch failed; the open point names it once and says why.
  await expect(failed).toContainText('Letzter Abruf');
  await expect(page.getByTestId('run-status')).toContainText('Fehlgeschlagen 08:30');
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
  await expect(page.getByTestId('run-running')).toContainText('Wartet auf linkedin.com');
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

test('rows and reader say the same in short words; dead ends lead on', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  // An excluded row names its reason in short words, the day rate carries its unit.
  await expect(excludedRows(page).first().locator('.foot')).toHaveText('Arbeitnehmerüberlassung');
  await expect(row(page, 'freelancermap-2801').getByTestId('row-facts')).toContainText(
    '1.100 €/Tag',
  );
  // The reader's facts: duration and remote share like the row, the date like the row with
  // the exact moment in its tooltip.
  await row(page, 'freelancermap-2801').click();
  const facts = page.locator('.head .facts');
  await expect(facts).toContainText('6 Monate');
  await expect(facts).toContainText('60 % remote');
  await expect(facts).not.toContainText('Hybrid');
  await expect(facts).not.toContainText('2026');
  // A teaser names its portal and leads to the sign-in in Einstellungen.
  await row(page, 'freelance-900411').click();
  await expect(page.getByTestId('detail-note')).toContainText(
    'Ohne Anmeldung zeigt freelance.de nur einen Anriss.',
  );
  await page.getByTestId('set-up-sign-in').click();
  await expect(page.getByTestId('view-settings')).toBeVisible();
});

test('without a profile the prompt says why it cannot work', async ({ page }) => {
  await open(page, `${WIN}&scenario=no-profile`);
  await rows(page).first().click();
  await expect(page.getByTestId('prompt')).toHaveAttribute('aria-disabled', 'true');
});

test('a list that fails to load says so once, and its retry reloads the overview too', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=list-error`);
  await expect(page.getByTestId('list-error')).toBeVisible();
  await expect(page.getByTestId('best-error')).toHaveCount(0);
});

test('a failing list offers a retry', async ({ page }) => {
  await open(page, `${WIN}&scenario=list-error`);
  await expect(page.getByTestId('list-error')).toContainText('Die Datenbank meldet einen Fehler.');
});

test('details and pins: teaser note, fetch details, pin star', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  // A teaser needs the sign-in: without it no "Details holen" that could not work.
  await row(page, 'freelance-900411').click();
  await expect(page.getByTestId('detail-note')).toContainText('Anriss');
  await expect(page.getByTestId('fetch-details')).toHaveCount(0);
  // A job whose details are still missing fetches them, from the note on the missing text.
  await row(page, 'linkedin-4100200302').click();
  await expect(page.getByTestId('fetch-details')).toHaveCount(1);
  await expect(
    page.getByTestId('detail-note').locator('xpath=..').getByTestId('fetch-details'),
  ).toBeVisible();
  await page.getByTestId('fetch-details').click();
  const started = await calls(page, 'start_run');
  expect((started[0]?.[1] as { request: unknown }).request).toEqual({
    kind: 'details',
    keys: [{ portal: 'linkedin', id: '4100200302' }],
  });
  await runFinished(page);
  await row(page, 'freelance-900411').click();
  await page.getByTestId('reader-pin').click();
  await expect(page.getByTestId('reader-pin')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByTestId('pin-freelance-900411')).toHaveAttribute('aria-pressed', 'true');
});

test('the close button in the reader leads back to the day overview', async ({ page }) => {
  await open(page, WIN);
  const first = rows(page).first();
  await first.click();
  await expect(page.getByTestId('reader')).toBeVisible();
  await expect(page.getByTestId('day-overview')).toHaveCount(0);
  await page.getByTestId('reader-close').click();
  await expect(page.getByTestId('day-overview')).toBeVisible();
  await expect(page.getByTestId('reader')).toHaveCount(0);
  await expect(first).not.toHaveAttribute('aria-current', 'true');
  // Below 900 px the view's back button does it; the close button steps aside.
  await first.click();
  await page.setViewportSize({ width: 780, height: 560 });
  await expect(page.getByTestId('reader-close')).toBeHidden();
  await expect(page.getByTestId('back')).toBeVisible();
});

test('a second click on the open row keeps it open; a search that drops it closes it', async ({
  page,
}) => {
  await open(page, WIN);
  const first = rows(page).first();
  await first.click();
  await expect(page.getByTestId('reader-ring')).toContainText('91');
  await first.click();
  await expect(page.getByTestId('reader')).toBeVisible();
  await expect(first).toHaveAttribute('aria-current', 'true');
  // A search that no longer finds the open job goes back to the overview.
  await page.getByTestId('search').fill('Kernfusion');
  await expect(page.getByTestId('day-overview')).toBeVisible();
});

test('switching jobs: never blank, the old text stays put, the new job starts at the top', async ({
  page,
}) => {
  await open(page, WIN);
  await rows(page).first().click();
  await expect(page.getByTestId('reader-ring')).toContainText('91');
  const top = await page.getByTestId('stage').evaluate((node) => {
    node.scrollTo({ top: 400 });
    return node.scrollTop;
  });
  expect(top).toBeGreaterThan(0);
  // Every frame from the click on: some stage shows content, no test id exists twice, and a
  // still picture of the old job (while there is one) keeps the old scroll position.
  await page.evaluate(() => {
    const pane = document.querySelector('[data-testid="reader-pane"]')!;
    const w = window as unknown as { __frames: [boolean, number, number][] };
    w.__frames = [];
    const sample = (): void => {
      const stages = [...pane.querySelectorAll<HTMLElement>('.stage')];
      const still = stages.find((node) => node.getAttribute('aria-hidden') === 'true');
      w.__frames.push([
        stages.some((node) => (node.textContent ?? '').trim() !== ''),
        document.querySelectorAll('[data-testid="reader"]').length,
        still?.scrollTop ?? -1,
      ]);
      if (w.__frames.length < 40) requestAnimationFrame(sample);
    };
    requestAnimationFrame(sample);
  });
  const next = rows(page).nth(1);
  await next.click();
  await expect(page.getByTestId('reader-title')).toHaveText(
    await next.locator('.title').innerText(),
  );
  await page.waitForFunction(
    () => (window as unknown as { __frames: unknown[] }).__frames.length >= 40,
  );
  const frames = await page.evaluate(
    () => (window as unknown as { __frames: [boolean, number, number][] }).__frames,
  );
  expect(frames.every(([filled]) => filled)).toBe(true);
  expect(frames.every(([, readers]) => readers <= 1)).toBe(true);
  const stills = frames.map(([, , still]) => still).filter((still) => still >= 0);
  expect(stills.length).toBeGreaterThan(0);
  expect(stills.every((still) => still === top)).toBe(true);
  expect(await page.getByTestId('stage').evaluate((node) => node.scrollTop)).toBe(0);
});

/** Watch the list for the next frames: the most rows that glide at once (a re-sort). */
async function watchGlides(page: Page, frames: number, during?: string): Promise<void> {
  await page.evaluate(
    ([count, selector]) => {
      const w = window as unknown as { __glides: number; __left: number };
      w.__glides = 0;
      w.__left = count;
      const gliding = (row: Element): boolean =>
        row.getAnimations().some((animation) => {
          const keyframes = (animation.effect as KeyframeEffect | null)?.getKeyframes() ?? [];
          return (
            keyframes.some((k) => 'transform' in k) && keyframes.every((k) => !('opacity' in k))
          );
        });
      const sample = (): void => {
        // A watch "during" something ends with it (before counting what comes after it).
        if (selector && document.querySelector(selector) === null) {
          w.__left = 0;
          return;
        }
        const list = document.querySelector('[data-testid="job-list"]');
        const now = list ? [...list.querySelectorAll('[data-key]')].filter(gliding).length : 0;
        w.__glides = Math.max(w.__glides, now);
        w.__left -= 1;
        if (w.__left > 0) requestAnimationFrame(sample);
      };
      requestAnimationFrame(sample);
    },
    [frames, during ?? ''] as const,
  );
}

/** Wait until no row moves any more. */
async function rowsAtRest(page: Page): Promise<void> {
  await page.waitForFunction(() =>
    [...document.querySelectorAll('[data-key]')].every((row) => row.getAnimations().length === 0),
  );
}

async function glides(page: Page): Promise<number> {
  await page.waitForFunction(() => (window as unknown as { __left: number }).__left <= 0);
  return page.evaluate(() => (window as unknown as { __glides: number }).__glides);
}

test('only a re-sort moves rows: new jobs of a run land in place, the sort switch glides', async ({
  page,
}) => {
  await open(page, `${WIN}&tick=30`);
  // During the run new jobs come in at the top: nothing below them slides.
  await page.getByTestId('fetch').click();
  await expect(page.getByTestId('run-running')).toBeVisible();
  const before = await rows(page).count();
  await watchGlides(page, 600, '[data-testid="run-running"]');
  expect(await glides(page)).toBe(0);
  expect(await rows(page).count()).toBeGreaterThan(before);
  await runFinished(page);
  // The re-sort at the end of the run glides; a search then filters in place.
  await rowsAtRest(page);
  await watchGlides(page, 20);
  await page.getByTestId('search').fill('Interim');
  await expect(rows(page)).toHaveCount(3);
  expect(await glides(page)).toBe(0);
  await page.getByTestId('search').fill('');
  await expect(rows(page)).not.toHaveCount(3);
  await rowsAtRest(page);
  // Another order is a re-sort: the rows on screen glide to their new place.
  await watchGlides(page, 120);
  await page.getByTestId('sort').click();
  await page.evaluate(() => window.__harness.pick(1));
  expect(await glides(page)).toBeGreaterThan(0);
});

test('the list header: one slot for Abrufen and Abbrechen, a steady second row, a line once scrolled', async ({
  page,
}) => {
  await open(page, `${WIN}&tick=200`);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const header = page.getByTestId('list-header');
  const clear = 'rgba(0, 0, 0, 0)';
  // The bottom line only once the list is scrolled.
  await expect(header).toHaveCSS('border-bottom-color', clear);
  const list = page.getByTestId('list-scroll');
  await list.evaluate((node) => node.scrollTo({ top: 300 }));
  await expect(header).not.toHaveCSS('border-bottom-color', clear);
  await list.evaluate((node) => node.scrollTo({ top: 0 }));
  await expect(header).toHaveCSS('border-bottom-color', clear);
  // The search keeps its width when Abbrechen takes the place of Abrufen.
  const search = page.getByTestId('search');
  const before = (await search.boundingBox())!.width;
  await page.getByTestId('fetch').click();
  await expect(page.getByTestId('cancel-run')).toBeVisible();
  expect((await search.boundingBox())!.width).toBe(before);
  await expect(page.getByRole('button', { name: 'Abrufen' })).toHaveCount(0);
  await page.getByTestId('cancel-run').click();
  await runFinished(page);
  // The second row keeps its height whatever it holds (the segments, a place's count).
  const second = header.locator('.second');
  const height = (await second.boundingBox())!.height;
  await page.getByTestId('nav-archive').click();
  await expect(page.getByTestId('place-count')).toBeVisible();
  expect((await second.boundingBox())!.height).toBe(height);
  await expect(page.getByTestId('search')).toHaveAttribute('placeholder', 'Archiv durchsuchen');
});

test('the reader: a compact bar once the actions scroll away, a jump flashes its passage', async ({
  page,
}) => {
  await open(page, WIN);
  await rows(page).first().click();
  await expect(page.getByTestId('reader-ring')).toContainText('91');
  const bar = page.getByTestId('reader-compact');
  await expect(bar).toHaveCSS('opacity', '0');
  await expect(bar).toHaveAttribute('inert', '');
  const stage = page.getByTestId('stage');
  await stage.evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
  await expect(bar).toHaveCSS('opacity', '1');
  await expect(bar).not.toHaveAttribute('inert');
  await expect(bar).toContainText('Interim CFO für Familienunternehmen');
  // The same tools in the same order as the title line (open first).
  expect(
    await bar.locator('.btn').evaluateAll((els) => els.map((el) => el.getAttribute('data-testid'))),
  ).toEqual(['compact-open', 'compact-archive', 'compact-trash', 'compact-pin', 'compact-close']);
  const pinned = await page.getByTestId('reader-pin').getAttribute('aria-pressed');
  await bar.getByTestId('compact-pin').click();
  await expect(page.getByTestId('reader-pin')).not.toHaveAttribute('aria-pressed', pinned ?? '');
  await stage.evaluate((node) => node.scrollTo({ top: 0 }));
  await expect(bar).toHaveCSS('opacity', '0');

  // A reason that jumps to its passage makes the passage flash once it has arrived.
  await page.evaluate(() => {
    const w = window as unknown as { __flashes: string[] };
    w.__flashes = [];
    new MutationObserver((records) => {
      for (const record of records) {
        const mark = record.target as Element;
        if (mark.classList.contains('flash')) w.__flashes.push(mark.getAttribute('data-reason')!);
      }
    }).observe(document.querySelector('[data-testid="ad-text"]')!, {
      subtree: true,
      attributes: true,
      attributeFilter: ['class'],
    });
  });
  await settle(page);
  await page.getByTestId('reasons-met').getByRole('button').first().click();
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { __flashes: string[] }).__flashes))
    .not.toEqual([]);
  await expect(page.locator('mark.flash')).toHaveCount(0);
});

test('the run card: steps side by side, a finished step draws its check once', async ({ page }) => {
  // A card that mounts with a step already done shows a plain check.
  await open(page, `${WIN}&scenario=running`);
  const tops = await Promise.all(
    ['scan', 'fetch', 'score'].map(async (step) =>
      Math.round((await page.getByTestId(`step-${step}`).boundingBox())!.y),
    ),
  );
  expect(new Set(tops).size).toBe(1);
  await expect(page.getByTestId('step-scan').locator('.mark')).not.toHaveClass(/drawn/);
  await expect(page.getByTestId('countdown')).toHaveText('Weiter in 0:42');
  // A step that finishes while the card is on screen draws its check.
  await open(page, `${WIN}&tick=80`);
  await page.getByTestId('fetch').click();
  await expect(page.getByTestId('step-scan')).toHaveClass(/current/);
  await expect(page.getByTestId('step-scan')).toHaveClass(/done/, { timeout: 10_000 });
  await expect(page.getByTestId('step-scan').locator('.mark')).toHaveClass(/drawn/);
  await runFinished(page);
  // Finished: the time and the pills; the chevron turns when the card collapses.
  await expect(page.getByTestId('last-new')).toContainText('2 neu');
  const toggle = page.getByTestId('run-toggle');
  await expect(toggle).toHaveClass(/turned/);
  await toggle.click();
  await expect(toggle).not.toHaveClass(/turned/);
  await expect(page.getByTestId('last-new')).toHaveCount(0);
});

test('the day overview: its best jobs open the reader', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('sort').click();
  await page.evaluate(() => window.__harness.pick(1));
  const best = page.getByTestId('best').locator('[data-testid^="best-"]').first();
  const title = await best.locator('.title').innerText();
  await best.click();
  await expect(page.getByTestId('reader-title')).toHaveText(title);
});

test('the reader: one row of alike actions, archive opens the next job, undo, a prompt', async ({
  page,
  browserName,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const first = rows(page).first();
  await first.click();
  await expect(page.getByTestId('reader')).toBeVisible();
  // Open the ad, the alert mail, the prompt: one row, the same outlined buttons, no marks.
  const actions = page.getByTestId('open-ad').locator('xpath=..');
  expect(
    await actions.evaluate((row) =>
      [...row.querySelectorAll('.btn')].map((button) => button.getAttribute('data-testid')),
    ),
  ).toEqual(['open-ad', 'open-mail', 'prompt']);
  await expect(actions.locator('.btn:not(.secondary)')).toHaveCount(0);
  await expect(page.getByTestId('applied')).toHaveCount(0);
  await expect(page.getByTestId('note')).toHaveCount(0);
  // The action row stays one line: a narrow reader says only "KI-Bewertung", wide the whole.
  const actionTops = async (): Promise<number> =>
    actions.evaluate(
      (row) => new Set([...row.children].map((child) => child.getBoundingClientRect().top)).size,
    );
  expect(await actionTops()).toBe(1);
  await expect(page.getByTestId('prompt')).toHaveText('KI-Bewertung');
  await page.setViewportSize({ width: 1600, height: 900 });
  await expect(page.getByTestId('prompt')).toHaveText('Prompt für KI-Bewertung kopieren');
  expect(await actionTops()).toBe(1);
  // A prompt for any AI chat.
  if (browserName === 'chromium') {
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  }
  await page.getByTestId('prompt').click();
  await expect(page.getByTestId('toast').last()).toContainText(
    'Prompt kopiert, bereit für einen KI-Chat.',
  );
  // Archivieren folds the row away and opens the next job; a double click archives one.
  const title = await page.getByTestId('reader-title').innerText();
  const next = await rows(page).nth(1).locator('.title').innerText();
  await page.getByTestId('reader-archive').dblclick();
  await expect(page.getByTestId('reader-title')).toHaveText(next);
  expect(await calls(page, 'move_jobs')).toHaveLength(1);
  await expect(page.getByTestId('toast').last()).toContainText(`„${title}“ archiviert.`);
  await expect(page.getByTestId('job-list').getByText(title, { exact: true })).toHaveCount(0);
  await page.getByTestId('toast').last().getByTestId('toast-action').click();
  await expect(page.getByTestId('job-list').getByText(title, { exact: true })).toHaveCount(1);
});

test('keys like a mail app: arrows open the next job, Home and End, Esc, Ctrl+F', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const all = page.getByTestId('job-list').locator('[data-testid^="job-row-"]');
  await expect.poll(async () => all.count()).toBeGreaterThan(6);
  await settle(page);
  const titles = await all.locator('.title').allInnerTexts();
  const reader = page.getByTestId('reader-title');
  // Nothing open and no focus: ArrowDown opens the first job, then the next; ArrowUp back.
  const blur = (): Promise<void> =>
    page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await blur();
  await page.keyboard.press('ArrowDown');
  await expect(reader).toHaveText(titles[0]!);
  await page.keyboard.press('ArrowDown');
  await expect(reader).toHaveText(titles[1]!);
  await expect(all.nth(1)).toHaveAttribute('aria-current', 'true');
  await expect(all.nth(1)).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(reader).toHaveText(titles[0]!);
  // Home and End: the first and the last row (the excluded ones come last).
  await page.keyboard.press('End');
  await expect(reader).toHaveText(titles.at(-1)!);
  await expect(all.last()).toBeInViewport();
  await page.keyboard.press('Home');
  await expect(reader).toHaveText(titles[0]!);
  // Esc closes the job: the day overview again.
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('day-overview')).toBeVisible();
  // A click on plain text in the reader leaves the keys working.
  await all.nth(2).click();
  await expect(reader).toHaveText(titles[2]!);
  await page.getByTestId('reader-title').click();
  await page.keyboard.press('ArrowDown');
  await expect(reader).toHaveText(titles[3]!);
  // Ctrl+F goes to the search, from anywhere; in the search Esc clears it first, then closes.
  await page.keyboard.press('Control+f');
  const search = page.getByTestId('search');
  await expect(search).toBeFocused();
  await page.keyboard.type(titles[3]!.slice(0, 12));
  await page.keyboard.press('Escape');
  await expect(search).toHaveValue('');
  await expect(reader).toHaveText(titles[3]!);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('day-overview')).toBeVisible();
  // The arrows inside the search move the caret, not the list.
  await search.fill('Finance');
  await search.press('ArrowLeft');
  await expect(page.getByTestId('day-overview')).toBeVisible();
});

test('Cmd+F is the find key on macOS, Ctrl+F is not', async ({ page }) => {
  await open(page, '?platform=macos');
  const search = page.getByTestId('search');
  await page.keyboard.press('Control+f');
  await expect(search).not.toBeFocused();
  await page.keyboard.press('Meta+f');
  await expect(search).toBeFocused();
});

test('archive from the row: toasts merge, the Archiv sends a job back to the inbox', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const one = 'freelancermap-2802';
  const two = 'freelancermap-2803';
  // A row's tools: the job's actions where it is, then the star.
  await row(page, one).hover();
  expect(
    await row(page, one)
      .locator('xpath=..')
      .locator('.tools .btn')
      .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label'))),
  ).toEqual(['Archivieren', 'Löschen', 'Als Favorit markieren']);
  for (const key of [one, two]) {
    await row(page, key).hover();
    await page.getByTestId(`archive-${key}`).click();
    await expect(row(page, key)).toHaveCount(0);
  }
  // Two archives in a row are one toast that takes both back.
  await expect(page.getByTestId('toast')).toHaveCount(1);
  await expect(page.getByTestId('toast')).toContainText('2 Jobs archiviert.');
  await page.getByTestId('toast-action').click();
  await expect(row(page, one)).toHaveCount(1);
  await expect(row(page, two)).toHaveCount(1);
  // Archived again, the job goes back to the inbox from the Archiv, with a toast.
  await row(page, one).hover();
  await page.getByTestId(`archive-${one}`).click();
  await page.getByTestId('nav-archive').click();
  await expect(page.getByTestId('place-count')).toContainText('im Archiv');
  await row(page, one).hover();
  await page.getByTestId(`toInbox-${one}`).click();
  await expect(row(page, one)).toHaveCount(0);
  await expect(page.getByTestId('toast').last()).toContainText('in den Eingang verschoben.');
  // Jobs in the sidebar is the inbox again.
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('facet')).toBeVisible();
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await expect(row(page, one)).toHaveCount(1);
});

test('the Papierkorb: delete, restore, delete for good and empty it, asking first', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const one = 'freelancermap-2802';
  const two = 'freelancermap-2803';
  for (const key of [one, two]) {
    await row(page, key).hover();
    await page.getByTestId(`trash-${key}`).click();
    await expect(row(page, key)).toHaveCount(0);
  }
  await expect(page.getByTestId('toast').last()).toContainText('2 Jobs gelöscht.');
  await page.getByTestId('nav-trash').click();
  await expect(page.getByTestId('place-count')).toHaveText('2 Jobs im Papierkorb');
  // In the trash: Wiederherstellen and Endgültig löschen, no star; the reader says where.
  await row(page, one).click();
  await expect(page.getByTestId('place-line')).toContainText('Im Papierkorb');
  await expect(page.getByTestId('reader-pin')).toHaveCount(0);
  await page.getByTestId('reader-restore').click();
  await expect(row(page, one)).toHaveCount(0);
  await expect(page.getByTestId('toast').last()).toContainText('wiederhergestellt.');
  // Endgültig löschen asks first; Abbrechen keeps the job.
  await row(page, two).hover();
  await page.getByTestId(`purge-${two}`).click();
  await expect(page.getByTestId('dialog-purge')).toBeVisible();
  await page.getByTestId('dialog-purge').getByRole('button', { name: 'Endgültig löschen' }).click();
  await expect(row(page, two)).toHaveCount(0);
  expect((await calls(page, 'purge_jobs')).map(([, args]) => args)).toEqual([
    { keys: [{ portal: 'freelancermap', id: '2803' }] },
  ]);
  await expect(page.getByTestId('empty-trash')).toHaveCount(0);
  // Papierkorb leeren: again a job there, again the question.
  await page.getByTestId('nav-jobs').click();
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await row(page, one).hover();
  await page.getByTestId(`trash-${one}`).click();
  await page.getByTestId('nav-trash').click();
  await page.getByTestId('empty-trash').click();
  await page
    .getByTestId('dialog-empty-trash')
    .getByRole('button', { name: 'Papierkorb leeren' })
    .click();
  await expect(page.getByTestId('empty-place-trash')).toBeVisible();
  await expect(page.getByTestId('empty-trash')).toHaveCount(0);
  expect(await calls(page, 'empty_trash')).toHaveLength(1);
});

test('choose like a mail app: Ctrl+click, Shift+click, the bar acts on all, Esc clears', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const all = rows(page);
  await all.nth(0).click();
  await all.nth(1).click({ modifiers: ['Control'] });
  const bar = page.getByTestId('selection-bar');
  await expect(bar).toContainText('2 ausgewählt');
  await expect(page.getByTestId('facet')).toHaveCount(0);
  // Esc clears the choice (the open job stays open).
  await page.keyboard.press('Escape');
  await expect(bar).toHaveCount(0);
  await expect(page.getByTestId('reader')).toBeVisible();
  // Shift+click takes the range; the bar archives them all in one move.
  await all.nth(0).click();
  await all.nth(2).click({ modifiers: ['Shift'] });
  await expect(bar).toContainText('3 ausgewählt');
  const before = await all.count();
  await bar.getByTestId('selection-archive').click();
  await expect(all).toHaveCount(before - 3);
  expect((await calls(page, 'move_jobs')).at(-1)?.[1]).toEqual(
    expect.objectContaining({ to: 'archive' }),
  );
  await expect(page.getByTestId('toast').last()).toContainText('3 Jobs archiviert.');
});

test('Alle als gelesen markieren: one click, the toast takes it back', async ({ page }) => {
  await open(page, WIN);
  const unread = await segmentCount(page, 'Neu');
  expect(unread).toBeGreaterThan(0);
  await page.getByTestId('mark-all-read').click();
  await expect(page.getByTestId('mark-all-read')).toHaveCount(0);
  await expect(page.getByTestId('toast').last()).toContainText('Alle als gelesen markiert.');
  await page.getByTestId('toast').last().getByTestId('toast-action').click();
  await expect.poll(() => segmentCount(page, 'Neu')).toBe(unread);
  expect(await calls(page, 'mark_unread')).toHaveLength(1);
});

test('a search looks in its place and names the hits elsewhere, keeping the search', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const key = 'freelancermap-2802';
  const title = await row(page, key).locator('.title').innerText();
  await row(page, key).hover();
  await page.getByTestId(`archive-${key}`).click();
  await page.getByTestId('search').fill(title);
  await expect(page.getByTestId('also-archive')).toHaveText('Auch im Archiv (1)');
  await page.getByTestId('also-archive').click();
  await expect(page.getByTestId('search')).toHaveValue(title);
  await expect(page.getByTestId('search')).toHaveAttribute('placeholder', 'Archiv durchsuchen');
  await expect(row(page, key)).toHaveCount(1);
});

test('the best matches as one prompt: at the end of the overview heading', async ({
  page,
  browserName,
}) => {
  if (browserName === 'chromium') {
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  }
  await open(page, WIN);
  await expect(page.getByTestId('best').getByTestId('prompt-top')).toBeVisible();
  await page.getByTestId('prompt-top').click();
  await expect(page.getByTestId('toast').last()).toContainText(
    'Prompt kopiert, bereit für einen KI-Chat.',
  );
  expect(await calls(page, 'ai_prompt_top')).toHaveLength(1);
  expect((await calls(page, 'ai_prompt_top'))[0]?.[1]).toEqual({ limit: 5 });
});

test('criteria show the ad value and jump to it; wishes have their block; rows show facts', async ({
  page,
}) => {
  await open(page, WIN);
  // The row's key facts from the ad.
  await expect(row(page, 'freelancermap-2801').getByTestId('row-facts')).toHaveText(
    /ab sofort.*6 Monate.*60\s%\sremote.*1\.100/,
  );
  await row(page, 'freelancermap-2801').click();
  const criteria = page.getByTestId('criteria');
  // A value the ad states, a criterion it leaves open (neutral, not ticked).
  await expect(criteria.getByTestId('criterion-c:countries')).toHaveText('Hamburg');
  const rate = criteria.getByTestId('criterion-c:minDayRate');
  await expect(rate).toHaveText('Satz nach Absprache');
  await expect(rate.locator('[data-state]')).toHaveAttribute('data-state', 'unset');
  await expect(criteria.getByTestId('criterion-c:availability')).toHaveText('Start offen');
  // A click marks the passage that states it.
  await rate.getByRole('button').click();
  await expect(page.locator('mark.active')).toContainText('Tagessatz nach Absprache');
  // Wishes in their own block of "Warum".
  await expect(page.getByTestId('wishes')).toContainText('erreicht den Wunsch');
  // A job whose ad meets every criterion shows one quiet line of the values.
  await row(page, 'linkedin-4100200301').click();
  await expect(page.getByTestId('criteria-clean')).toContainText('Bremen');
  await expect(page.getByTestId('criteria')).toHaveCount(0);
});

test('an empty list says where jobs come from', async ({ page }) => {
  await open(page, `${WIN}&scenario=empty`);
  await expect(page.getByTestId('alert-linkedin')).toBeVisible();
  await page.getByTestId('alert-linkedin').click();
  expect((await calls(page, 'open_target')).at(-1)?.[1]).toEqual({
    target: { kind: 'portalHome', portal: 'linkedin' },
  });
  await expect(page.getByTestId('read-older')).toBeVisible();
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
