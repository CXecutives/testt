// The reader, the day overview and the run card after the final round of findings (track
// reader): one column, the search, the action row, the tooltips, the copy, the focus, the
// files and the portal notes.

import type { Page } from '@playwright/test';
import { calls, expect, open, runFinished, test } from './fixtures';

const WIN = '?platform=windows';
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
const row = (page: Page, key: string) => page.getByTestId('job-list').getByTestId(`job-row-${key}`);
const facet = (page: Page, name: string) =>
  page.getByTestId('facet').getByRole('radio', { name: new RegExp(name) });

/** A clipboard that refuses every write (a WebView without the permission). */
async function refusingClipboard(page: Page): Promise<void> {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: () => Promise.reject(new Error('denied')) },
    });
  });
}

test('one column: choosing several jobs keeps the list, its bar acts on them', async ({ page }) => {
  await page.setViewportSize({ width: 683, height: 700 });
  await open(page, WIN);
  await facet(page, 'Alle').click();
  await rows(page)
    .nth(0)
    .click({ modifiers: ['Control'] });
  await rows(page)
    .nth(2)
    .click({ modifiers: ['Shift'] });
  await expect(page.getByTestId('selection-bar')).toContainText('3 ausgewählt');
  const list = page.getByTestId('list-scroll');
  await expect(list).toBeVisible();
  expect((await list.boundingBox())?.width ?? 0).toBeGreaterThan(600);
  await expect(page.getByTestId('selection-pane')).toBeHidden();
  await rows(page)
    .nth(1)
    .click({ modifiers: ['Control'] });
  await expect(page.getByTestId('selection-bar')).toContainText('2 ausgewählt');
  await expect(list).toBeVisible();
  expect(await calls(page, 'job_detail')).toEqual([]);
});

test('one column: the header stays on top while the list scrolls', async ({ page }) => {
  await page.setViewportSize({ width: 780, height: 560 });
  await open(page, WIN);
  await facet(page, 'Alle').click();
  await page
    .getByTestId('list-scroll')
    .evaluate((node) => node.parentElement?.scrollTo({ top: 400 }));
  await expect(page.getByTestId('search')).toBeInViewport();
  await expect(page.getByTestId('fetch')).toBeInViewport();
  const search = await page.getByTestId('search').boundingBox();
  expect(search?.y ?? -1).toBeGreaterThanOrEqual(0);
});

test('a search keeps the open job that is a hit beyond the loaded rows', async ({ page }) => {
  await open(page, `${WIN}&scenario=many`);
  await facet(page, 'Alle').click();
  await page.getByTestId('sort').click();
  await page.evaluate(() => window.__harness.pick(1));
  await expect(page.getByTestId('sort')).toHaveText('Nach Datum');
  await row(page, 'linkedin-100006').click();
  await expect(page.getByTestId('reader-title')).toHaveText('Finance Manager 7');
  await page.getByTestId('sort').click();
  await page.evaluate(() => window.__harness.pick(0));
  await expect(page.getByTestId('sort')).toHaveText('Nach Passung');
  await page.getByTestId('search').fill('Finance Manager');
  await expect(facet(page, 'Alle')).toContainText('500');
  await page.waitForTimeout(400);
  await expect(page.getByTestId('reader-title')).toHaveText('Finance Manager 7');
});

for (const width of [900, 960, 1000, 1100]) {
  test(`the reader's action row stays one line at ${width} px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await rows(page).first().click();
    const top = async (id: string): Promise<number> =>
      (await page.getByTestId(id).boundingBox())?.y ?? -1;
    await expect.poll(async () => (await top('prompt')) === (await top('open-ad'))).toBe(true);
  });
}

test('the date of a job names the day and the time of its alert mail', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  await row(page, 'freelancermap-2801').click();
  await page.locator('.head .facts .fact:last-child > span:last-child').hover();
  await expect(page.getByRole('tooltip')).toHaveText(/^Alert-Mail vom 24\.09\.2026 um \d\d:\d\d$/);
});

test('the chips of the terms say criterion and state in one phrase', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  await row(page, 'freelancermap-2801').click();
  const tip = async (id: string): Promise<string> => {
    await page.getByTestId(`criterion-${id}`).hover();
    const text = (await page.getByRole('tooltip').textContent()) ?? '';
    await page.mouse.move(0, 0);
    return text;
  };
  // The ad names the rate, only not as an amount: open, not "not mentioned".
  expect(await tip('c:minDayRate')).toBe('Tagessatz offen');
  expect(await tip('c:countries')).toBe('Einsatzländer erfüllt');
});

test('the facts copy as one line with their dots', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  await row(page, 'freelancermap-2801').click();
  const copied = await page.locator('.head .facts').evaluate((line) => {
    const selection = window.getSelection();
    selection?.selectAllChildren(line);
    return selection?.toString() ?? '';
  });
  expect(copied).not.toContain('\n');
  expect(copied).toMatch(/\S · \S/);
});

test('a clipboard that refuses says so in its own words, everywhere', async ({ page }) => {
  await refusingClipboard(page);
  await open(page, WIN);
  await page.getByTestId('prompt-top').click();
  await expect(page.getByTestId('day-overview')).toContainText(
    'Der Prompt ließ sich nicht kopieren.',
  );
  await facet(page, 'Alle').click();
  await rows(page).first().click();
  await page.getByTestId('prompt').click();
  await expect(page.getByTestId('reader')).toContainText('Der Prompt ließ sich nicht kopieren.');
  await expect(page.getByText('Protokoll')).toHaveCount(0);
});

test('the comparison prompt stays while no new match is left', async ({ page }) => {
  await open(page, WIN);
  await expect(page.getByTestId('best').getByTestId('prompt-top')).toBeVisible();
  await page.getByTestId('mark-all-read').click();
  await expect(page.getByTestId('best')).toHaveCount(0);
  await expect(page.getByTestId('compare').getByTestId('prompt-top')).toBeVisible();
});

test('the best rows of the overview have the tools of the list', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  const best = page.getByTestId('best');
  await best.locator('[data-testid^="best-"]').first().hover();
  await expect(best.getByRole('button', { name: 'Archivieren' }).first()).toBeVisible();
  await expect(best.getByRole('button', { name: /Favorit/ }).first()).toBeVisible();
});

test('closing a job from the reader hands the focus to its row', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  await row(page, 'freelancermap-2801').click();
  await page.getByTestId('reader-close').focus();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('day-overview')).toBeVisible();
  await expect(row(page, 'freelancermap-2801')).toBeFocused();
  // Esc on one of the reader's buttons does the same.
  await row(page, 'freelancermap-2801').click();
  await page.getByTestId('open-ad').focus();
  await page.keyboard.press('Escape');
  await expect(row(page, 'freelancermap-2801')).toBeFocused();
});

test('archiving from the reader keeps the focus on Archivieren of the next job', async ({
  page,
}) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  await rows(page).first().click();
  const first = await page.getByTestId('reader-title').textContent();
  await page.getByTestId('reader-archive').focus();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('reader-title')).not.toHaveText(first ?? '');
  await expect(page.getByTestId('reader-archive')).toBeFocused();
});

test('a locked Excel file is written again without reading the mailbox', async ({ page }) => {
  await open(page, `${WIN}&tick=15&export=locked`);
  await page.getByTestId('fetch').click();
  await runFinished(page);
  const note = page.getByTestId('export-failed');
  await expect(note).toHaveCount(1);
  await note.getByRole('button', { name: 'Erneut versuchen' }).click();
  await runFinished(page);
  const started = await calls(page, 'start_run');
  expect((started.at(-1)?.[1] as { request: unknown }).request).toEqual({ kind: 'rescore' });
  // Still locked: the card keeps its fetch and says it once.
  await expect(page.getByTestId('run-finished')).toContainText('Abruf fertig');
  await expect(page.getByTestId('export-failed')).toHaveCount(1);
});

test('files that could not be written are no green success elsewhere', async ({ page }) => {
  await open(page, `${WIN}&tick=15&export=locked`);
  await page.getByTestId('fetch').click();
  await page.getByTestId('nav-settings').click();
  await runFinished(page);
  const toast = page.getByTestId('toast');
  await expect(toast).toHaveText('Abruf fertig, die Dateien sind nicht aktuell.');
  await expect(toast).not.toHaveClass(/success/);
});

test('a pause that resolves itself is a calm note, alert mails without jobs a warning', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=paused`);
  const pause = page.getByTestId('issue-linkedin-health');
  await expect(pause).toHaveClass(/info/);
  await expect(pause).toContainText('von selbst weiter');
  await expect(page.getByTestId('issue-freelance-mails')).toHaveClass(/warning/);
});

test('a pause during a run is a calm note too', async ({ page }) => {
  await open(page, `${WIN}&scenario=running`);
  await expect(page.getByTestId('pause-freelance')).toHaveClass(/info/);
});

test('a portal switched off has no open points', async ({ page }) => {
  await open(page, WIN);
  await expect(page.getByTestId('issue-freelance-mails')).toBeVisible();
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('toggle-enabled-freelance').click();
  await expect(page.getByTestId('toggle-enabled-freelance')).toHaveAttribute(
    'aria-checked',
    'false',
  );
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('day-overview')).toBeVisible();
  await expect(page.getByTestId('issue-freelance-mails')).toHaveCount(0);
});

test('an empty trash shows one empty state, the reader only its sentence', async ({ page }) => {
  await open(page, `${WIN}&scenario=empty`);
  await page.getByTestId('nav-trash').click();
  const note = page.getByTestId('place-reader');
  await expect(note).toContainText('30 Tage');
  await expect(note.locator('svg')).toHaveCount(0);
});
