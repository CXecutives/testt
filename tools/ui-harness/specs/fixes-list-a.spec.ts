// Fixes of the final audit round for the job list, its header and the selection: paging,
// choosing several jobs, the keys, the header row, errors and the empty states.

import type { Page } from '@playwright/test';
import type { Portal } from '../../../ui/src/lib/ipc/types';
import { calls, expect, open, settle, test } from './fixtures';

const WIN = '?platform=windows';
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
const row = (page: Page, key: string) => page.getByTestId('job-list').getByTestId(`job-row-${key}`);
const facet = (page: Page, name: string) =>
  page.getByTestId('facet').getByRole('radio', { name: new RegExp(name) });

/** Keys of the listed rows, in the order of the list. */
function listedKeys(page: Page): Promise<string[]> {
  return page
    .getByTestId('job-list')
    .locator('[data-key]')
    .evaluateAll((items) => items.map((item) => (item as HTMLElement).dataset.key ?? ''));
}

/** Scroll the list to its end until at least `count` rows are mounted. */
async function mountRows(page: Page, count: number): Promise<void> {
  const scroll = page.getByTestId('list-scroll');
  await expect
    .poll(
      async () => {
        await scroll.evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
        return page.getByTestId('job-list').locator('[data-key]').count();
      },
      { timeout: 20_000 },
    )
    .toBeGreaterThanOrEqual(count);
}

/** The sample jobs of the default scenario (stub.ts). */
const SAMPLE = [
  ['freelancermap', '2801'],
  ['linkedin', '4100200301'],
  ['freelance', '900411'],
  ['freelancermap', '2803'],
  ['linkedin', '4100200302'],
  ['freelance', '900412'],
  ['linkedin', '4100200303'],
  ['linkedin', '4100200306'],
  ['linkedin', '4100200304'],
  ['freelancermap', '2805'],
  ['freelancermap', '2802'],
  ['freelancermap', '2804'],
  ['freelance', '900413'],
  ['linkedin', '4100200305'],
  ['freelancermap', '2806'],
] as const;

/** Change jobs of the stub before the page asks for the app state (read, pinned). */
async function atStart(
  page: Page,
  keys: readonly (readonly [Portal, string])[],
  change: { unread?: boolean; pinned?: boolean },
): Promise<void> {
  await page.addInitScript(
    ({ keys, change }) => {
      let harness: Window['__harness'] | undefined;
      Object.defineProperty(window, '__harness', {
        configurable: true,
        get: () => harness,
        set: (value: Window['__harness']) => {
          harness = value;
          // After the stub has built its jobs (the rest of its module), before app_state.
          queueMicrotask(() => {
            for (const [portal, id] of keys) {
              const job = value.job({ portal, id });
              if (job) value.emit({ type: 'jobUpdated', job: { ...job, ...change }, fresh: false });
            }
          });
        },
      });
    },
    { keys, change },
  );
}

/** Every sample job is read before the page asks for the app state. */
function allReadAtStart(page: Page): Promise<void> {
  return atStart(page, SAMPLE, { unread: false });
}

/** Keys of the jobs of `?scenario=many` (stub.ts manyJobs). */
function manyKeys(count: number): [Portal, string][] {
  const portals: Portal[] = ['linkedin', 'freelance', 'freelancermap'];
  return Array.from({ length: count }, (_, i) => [portals[i % 3]!, String(100000 + i)]);
}

test.describe('paging and the remembered tab', () => {
  test('jobs read under Neu keep their row, and the next page skips no job', async ({ page }) => {
    await open(page, `${WIN}&scenario=many`);
    // Read three jobs: Neu keeps their rows (like Mail), the backend no longer lists them.
    for (const index of [0, 1, 2]) await rows(page).nth(index).click();
    await mountRows(page, 240);
    const seen = await listedKeys(page);
    const offsets = (await calls(page, 'list_jobs'))
      .map(([, args]) => (args as { query: { offset: number; limit: number } }).query)
      .filter((query) => query.offset > 0 && query.limit > 0)
      .map((query) => query.offset);
    expect(offsets[0]).toBe(117);
    // Neu entered again: the read jobs leave it, and every job it lists now was listed before.
    await facet(page, 'Alle').click();
    await facet(page, 'Neu').click();
    await mountRows(page, 237);
    const again = (await listedKeys(page)).slice(0, 237);
    expect(again.filter((key) => !seen.includes(key))).toEqual([]);
  });

  test('a start on Alle keeps Alle as the tab Jobs comes back to', async ({ page }) => {
    await allReadAtStart(page);
    await open(page, WIN);
    await expect(facet(page, 'Alle')).toHaveAttribute('aria-checked', 'true');
    await page.getByTestId('nav-archive').click();
    await page.getByTestId('nav-jobs').click();
    await settle(page);
    await expect(facet(page, 'Alle')).toHaveAttribute('aria-checked', 'true');
    await expect(rows(page).first()).toBeVisible();
    await expect(row(page, 'freelancermap-2801')).toBeVisible();
  });
});

/** The rows the list highlights, by key. */
function highlighted(page: Page): Promise<string[]> {
  return page
    .getByTestId('job-list')
    .locator('[data-key]:has(.row.selected)')
    .evaluateAll((items) => items.map((item) => (item as HTMLElement).dataset.key ?? ''));
}

/** After a move the list ignores clicks for a moment (the row under the pointer changed). */
async function settleMoves(page: Page): Promise<void> {
  await page.waitForTimeout(550);
}

test.describe('choosing several jobs', () => {
  test('the highlight shows exactly the chosen rows; Ctrl+click takes a highlighted row out', async ({
    page,
  }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    const bar = page.getByTestId('selection-bar');
    // A range from the anchor replaces the choice: the open job is no longer part of it.
    await row(page, 'freelancermap-2801').click();
    await row(page, 'linkedin-4100200301').click({ modifiers: ['Control'] });
    await row(page, 'freelancermap-2802').click({ modifiers: ['Shift'] });
    await expect(bar).toContainText('3 ausgewählt');
    expect(await highlighted(page)).toEqual([
      'linkedin:4100200301',
      'freelance:900411',
      'freelancermap:2802',
    ]);
    // Ctrl+click on the open job, not highlighted: it joins and shows.
    await row(page, 'freelancermap-2801').click({ modifiers: ['Control'] });
    await expect(bar).toContainText('4 ausgewählt');
    expect(await highlighted(page)).toHaveLength(4);
    // Ctrl+click on a highlighted row always takes it out.
    await row(page, 'freelance-900411').click({ modifiers: ['Control'] });
    await expect(bar).toContainText('3 ausgewählt');
    expect(await highlighted(page)).not.toContain('freelance:900411');
    // Without a choice, a Ctrl+click on the open job closes it, like in a mail app.
    await page.keyboard.press('Escape');
    await row(page, 'freelancermap-2803').click();
    await expect(page.getByTestId('reader')).toBeVisible();
    await row(page, 'freelancermap-2803').click({ modifiers: ['Control'] });
    await expect(page.getByTestId('reader')).toHaveCount(0);
    expect(await highlighted(page)).toEqual([]);
  });

  test('a range starts from the job the app opened, not from a row clicked before', async ({
    page,
  }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    // Archive the open job from the reader: the app opens the next one.
    await row(page, 'freelancermap-2802').click();
    await page.getByTestId('reader-archive').click();
    await expect(page.getByTestId('reader-title')).toHaveText(/Kaufmännische Leitung/);
    await settleMoves(page);
    await row(page, 'linkedin-4100200304').click({ modifiers: ['Shift'] });
    await expect(page.getByTestId('selection-bar')).toContainText('4 ausgewählt');
    expect(await highlighted(page)).toEqual([
      'freelancermap:2803',
      'linkedin:4100200303',
      'freelancermap:2804',
      'linkedin:4100200304',
    ]);
    // A job opened from the day overview starts the range too, not the row clicked before.
    await page.keyboard.press('Escape');
    await row(page, 'freelancermap-2806').click();
    await page.getByTestId('reader-close').click();
    await page.getByTestId('best').locator('[data-testid^="best-"]').first().click();
    await expect.poll(async () => (await highlighted(page)).length).toBe(1);
    const opened = (await highlighted(page))[0] ?? '';
    expect(opened).not.toBe('freelancermap:2806');
    const keys = await listedKeys(page);
    const at = keys.indexOf(opened);
    await rows(page)
      .nth(at + 1)
      .click({ modifiers: ['Shift'] });
    expect(await highlighted(page)).toEqual(keys.slice(at, at + 2));
  });

  test('one chosen row left by a move opens; the highlight and the reader agree', async ({
    page,
  }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await row(page, 'freelancermap-2801').click();
    await row(page, 'freelance-900411').click({ modifiers: ['Control'] });
    await row(page, 'freelancermap-2802').click({ modifiers: ['Shift'] });
    await expect(page.getByTestId('selection-bar')).toContainText('2 ausgewählt');
    await settleMoves(page);
    await row(page, 'freelancermap-2802').hover();
    await page.getByTestId('archive-freelancermap-2802').click();
    await expect(page.getByTestId('selection-bar')).toHaveCount(0);
    await expect(page.getByTestId('reader-title')).toHaveText('SAP S/4HANA Finance Projektleitung');
    // (The archived row keeps its look while it folds away.)
    await expect.poll(() => highlighted(page)).toEqual(['freelance:900411']);
  });

  test('in one column a Ctrl or Shift click chooses and keeps the list', async ({ page }) => {
    await page.setViewportSize({ width: 820, height: 640 });
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await row(page, 'freelancermap-2801').click({ modifiers: ['Control'] });
    await expect(page.getByTestId('list-scroll')).toBeVisible();
    await expect(page.getByTestId('reader')).toHaveCount(0);
    expect(await highlighted(page)).toEqual(['freelancermap:2801']);
    expect(await calls(page, 'job_detail')).toEqual([]);
    await row(page, 'freelance-900411').click({ modifiers: ['Control'] });
    await expect(page.getByTestId('selection-bar')).toContainText('2 ausgewählt');
    expect(await calls(page, 'job_detail')).toEqual([]);
  });
});

test.describe('the keys over the whole list', () => {
  /** The open job's row is the last row of the list, mounted: every page has loaded. */
  async function lastIsOpen(page: Page): Promise<boolean> {
    const keys = await listedKeys(page);
    return keys.length > 120 && (await highlighted(page))[0] === keys.at(-1);
  }

  test('End opens the last job of the whole list and gives its row the focus', async ({ page }) => {
    await open(page, `${WIN}&scenario=many`);
    await page.keyboard.press('End');
    await expect.poll(() => lastIsOpen(page), { timeout: 20_000 }).toBe(true);
    const last = (await listedKeys(page)).at(-1) ?? '';
    const focused = page.getByTestId(`job-row-${last.replace(':', '-')}`);
    await expect(focused).toBeFocused();
    await expect(focused).toBeInViewport();
    await expect(page.getByTestId('job-list').locator('.sentinel')).toHaveCount(0);
    // End again stays on it.
    await page.keyboard.press('End');
    await expect.poll(() => lastIsOpen(page)).toBe(true);
  });

  test('ArrowUp with nothing open opens the last job of the whole list', async ({ page }) => {
    await open(page, `${WIN}&scenario=many`);
    await page.keyboard.press('ArrowUp');
    await expect.poll(() => lastIsOpen(page), { timeout: 20_000 }).toBe(true);
  });

  test('a re-sort keeps the open job in view, and the arrows go on from it', async ({ page }) => {
    await open(page, `${WIN}&scenario=many`);
    await facet(page, 'Alle').click();
    await rows(page).nth(39).click();
    const key = (await highlighted(page))[0] ?? '';
    await page.getByTestId('sort').click();
    await page.evaluate(() => window.__harness.pick(1));
    await expect(page.getByTestId('sort')).toHaveText('Nach Datum');
    const target = page.getByTestId('job-list').locator(`[data-key="${key}"] .row`);
    await expect(target).toHaveClass(/selected/);
    await expect(target).toBeInViewport();
    const keys = await listedKeys(page);
    const next = keys[keys.indexOf(key) + 1];
    await page.keyboard.press('ArrowDown');
    await expect.poll(() => highlighted(page)).toEqual([next]);
  });
});

/** The right edge of an element (px from the left of the window). */
async function rightOf(page: Page, testid: string): Promise<number> {
  const box = await page.getByTestId(testid).boundingBox();
  return Math.round((box?.x ?? 0) + (box?.width ?? 0));
}

/** Archive two sample jobs with their row tools. */
async function archiveTwo(page: Page): Promise<void> {
  await facet(page, 'Alle').click();
  for (const key of ['freelancermap-2802', 'freelancermap-2804']) {
    await row(page, key).hover();
    await page.getByTestId(`archive-${key}`).click();
    await settleMoves(page);
  }
}

test.describe('the list header', () => {
  test('Neu, Alle and Favoriten keep their whole labels with thousands of jobs', async ({
    page,
  }) => {
    await atStart(page, manyKeys(12), { pinned: true });
    const labelsCut = (): Promise<boolean> =>
      page
        .getByTestId('facet')
        .locator('.label')
        .evaluateAll((labels) => labels.some((label) => label.scrollWidth > label.clientWidth));
    for (const width of [1100, 1300, 1366, 1920]) {
      await page.setViewportSize({ width, height: 900 });
      await open(page, `${WIN}&scenario=many`);
      await expect(facet(page, 'Favoriten')).toContainText('12');
      expect(await labelsCut(), `${width} px`).toBe(false);
    }
    // A wide column holds the segments and the tools on one line.
    await page.addInitScript(() => localStorage.setItem('jobs-list-width', '560'));
    await open(page, `${WIN}&scenario=many`);
    const second = page.getByTestId('facet').locator('xpath=..');
    expect((await second.boundingBox())?.height).toBe(28);
    expect(await labelsCut()).toBe(false);
  });

  test('the selection bar ends where the order does, on one line and on two', async ({ page }) => {
    for (const width of [1360, 1600]) {
      await page.setViewportSize({ width, height: 900 });
      if (width === 1600) {
        await page.addInitScript(() => localStorage.setItem('jobs-list-width', '560'));
      }
      await open(page, WIN);
      await facet(page, 'Alle').click();
      const end = await rightOf(page, 'sort');
      await row(page, 'freelancermap-2801').click();
      await row(page, 'linkedin-4100200301').click({ modifiers: ['Control'] });
      await expect(page.getByTestId('selection-bar')).toBeVisible();
      expect(await rightOf(page, 'selection-clear'), `${width} px`).toBe(end);
    }
  });

  test('the order keeps the end of the row in the Papierkorb too', async ({ page }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    const end = await rightOf(page, 'sort');
    for (const key of ['freelancermap-2802', 'freelancermap-2804']) {
      await row(page, key).hover();
      await page.getByTestId(`trash-${key}`).click();
      await settleMoves(page);
    }
    await page.getByTestId('nav-trash').click();
    await expect(page.getByTestId('empty-trash')).toBeVisible();
    expect(await rightOf(page, 'sort')).toBe(end);
    const trash = await page.getByTestId('empty-trash').boundingBox();
    const sort = await page.getByTestId('sort').boundingBox();
    expect((trash?.x ?? 0) < (sort?.x ?? 0)).toBe(true);
  });

  test('during a search the place count says what it found there', async ({ page }) => {
    await open(page, WIN);
    await archiveTwo(page);
    await page.getByTestId('nav-archive').click();
    // (The sample data keeps one job in the archive already.)
    await expect(page.getByTestId('place-count')).toHaveText('3 Jobs im Archiv');
    await page.getByTestId('search').fill('Treasury');
    await expect(page.getByTestId('place-count')).toHaveText('1 Job zu „Treasury“ im Archiv');
  });

  test('a double click on Alle als gelesen markieren marks once; its undo brings Neu back', async ({
    page,
  }) => {
    await open(page, WIN);
    const unread = await facet(page, 'Neu').innerText();
    await page.getByTestId('mark-all-read').dblclick();
    await expect(page.getByTestId('toast')).toHaveCount(1);
    expect(await calls(page, 'mark_all_read')).toHaveLength(1);
    await page.keyboard.press('Control+z');
    await expect(facet(page, 'Neu')).toHaveText(unread);
    expect(await calls(page, 'mark_unread')).toHaveLength(1);
  });

  test('Alle als gelesen markieren stays while Neu lists an unread excluded job', async ({
    page,
  }) => {
    await open(page, WIN);
    // Read every counted job of Neu: only the unread excluded one stays unread.
    const counted = await rows(page).evaluateAll((items) =>
      items.map((item) => item.getAttribute('data-testid') ?? ''),
    );
    for (const id of counted) await page.getByTestId('job-list').getByTestId(id).click();
    // Closed, the last one read leaves Neu too once it is entered again.
    await page.keyboard.press('Escape');
    await facet(page, 'Alle').click();
    await facet(page, 'Neu').click();
    await expect(rows(page)).toHaveCount(0);
    await expect(
      page.getByTestId('excluded-rows').locator('[data-testid^="job-row-"]'),
    ).toHaveCount(1);
    await page.getByTestId('mark-all-read').click();
    expect(await calls(page, 'mark_all_read')).toHaveLength(1);
    await expect(page.getByTestId('mark-all-read')).toHaveCount(0);
  });
});
