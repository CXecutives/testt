// Fixes of the closing round for the job list: one Tab stop, rows that keep their shape,
// toasts and their undo, dates that move on, the trash date, errors that are said.

import type { Page } from '@playwright/test';
import { calls, expect, NOW, open, settle, test } from './fixtures';

const WIN = '?platform=windows';
const list = (page: Page) => page.getByTestId('job-list');
const row = (page: Page, key: string) => list(page).getByTestId(`job-row-${key}`);
const facet = (page: Page, name: string) =>
  page.getByTestId('facet').getByRole('radio', { name: new RegExp(name) });

/** After a move the list ignores clicks for a moment (the row under the pointer changed). */
async function settleMoves(page: Page): Promise<void> {
  await page.waitForTimeout(550);
}

/** A row's tool: it exists while the pointer is on the row. */
async function tool(page: Page, id: string, key: string): Promise<void> {
  await row(page, key).hover();
  await page.getByTestId(`${id}-${key}`).click();
}

/** The next call of `command` fails with a database error. */
async function failNext(page: Page, command: string): Promise<void> {
  await page.evaluate((command) => {
    const calls = window.__harness.calls;
    const push = calls.push.bind(calls);
    calls.push = (...items: [string, unknown][]) => {
      if (items.some(([name]) => name === command)) {
        calls.push = push;
        push(...items);
        throw { kind: 'db', params: {} };
      }
      return push(...items);
    };
  }, command);
}

/** Is the focus inside the job list? */
function focusInList(page: Page): Promise<boolean> {
  return page.evaluate(() => document.activeElement?.closest('[data-testid="job-list"]') != null);
}

test.describe('the list is one Tab stop', () => {
  test('Tab leaves the list from its open row, Shift+Tab comes back to it', async ({ page }) => {
    await open(page, WIN);
    await row(page, 'linkedin-4100200301').click();
    // One row takes Tab; the others and every tool are reached otherwise.
    await expect(list(page).locator('.row:not([tabindex="-1"])')).toHaveCount(1);
    await row(page, 'linkedin-4100200301').focus();
    await page.keyboard.press('Tab');
    expect(await focusInList(page)).toBe(false);
    await page.keyboard.press('Shift+Tab');
    await expect(row(page, 'linkedin-4100200301')).toBeFocused();
    // The arrows move within it, and the Tab stop goes along.
    await page.keyboard.press('ArrowDown');
    await expect(row(page, 'freelance-900411')).toBeFocused();
    await expect(row(page, 'freelance-900411')).not.toHaveAttribute('tabindex', '-1');
    await expect(row(page, 'linkedin-4100200301')).toHaveAttribute('tabindex', '-1');
  });
});

test.describe('rows keep their shape', () => {
  test('reading a job keeps its title as wide as before, so it never wraps anew', async ({
    page,
  }) => {
    await open(page, WIN);
    const title = row(page, 'linkedin-4100200301').locator('.title');
    const width = () =>
      title.evaluate((node) => {
        const range = document.createRange();
        range.selectNodeContents(node);
        return range.getBoundingClientRect().width;
      });
    await expect(title).toHaveClass(/unread/);
    const unread = await width();
    await row(page, 'linkedin-4100200301').click();
    await expect(title).not.toHaveClass(/unread/);
    expect(Math.abs((await width()) - unread)).toBeLessThan(0.01);
  });

  test('the company gives way before the place', async ({ page }) => {
    await page.setViewportSize({ width: 1100, height: 800 });
    await open(page, WIN);
    await facet(page, 'Alle').click();
    const place = row(page, 'linkedin-4100200303').locator('.place');
    await expect(place).toBeVisible();
    const cut = await place.evaluate((node) => node.scrollWidth > node.clientWidth);
    expect(cut).toBe(false);
  });

  test('without a profile a row without a badge is only as high as its ring', async ({ page }) => {
    await open(page, `${WIN}&scenario=no-profile`);
    const bare = list(page).locator('.job.bare .row').first();
    await expect(bare).toBeVisible();
    expect((await bare.boundingBox())!.height).toBe(64);
  });

  test('the badge of a job that cannot be scored says why', async ({ page }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await row(page, 'freelancermap-2806').getByText('Nicht bewertbar').hover();
    await expect(page.getByRole('tooltip')).toHaveText('Zu wenig Text für eine Bewertung.');
  });
});

test.describe('toasts and their undo', () => {
  test('a toast names the whole title', async ({ page }) => {
    await open(page, WIN);
    await tool(page, 'trash', 'freelancermap-2803');
    await expect(page.getByTestId('toast-text').last()).toContainText(
      'Kaufmännische Leitung Projektgeschäft',
    );
  });

  test('an undo opens no job its move did not take away', async ({ page }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await row(page, 'linkedin-4100200301').click();
    await page.getByTestId('reader-archive').click();
    await settleMoves(page);
    // Back from the archive with the row tool, nothing open.
    await page.getByTestId('nav-archive').click();
    await settle(page);
    await tool(page, 'toInbox', 'linkedin-4100200301');
    await page.getByTestId('nav-jobs').click();
    await settle(page);
    await expect(page.getByTestId('reader-title')).toHaveCount(0);
    // Another job moved and taken back: nothing opens.
    await tool(page, 'archive', 'freelancermap-2803');
    await expect(row(page, 'freelancermap-2803')).toHaveCount(0);
    await page.keyboard.press('Control+z');
    await expect(row(page, 'freelancermap-2803')).toBeVisible();
    await page.waitForTimeout(300);
    await expect(page.getByTestId('reader-title')).toHaveCount(0);
  });

  test('in Neu the undo brings the open job back and opens it again', async ({ page }) => {
    await open(page, WIN);
    await expect(facet(page, 'Neu')).toHaveAttribute('aria-checked', 'true');
    await row(page, 'freelancermap-2801').click();
    await row(page, 'linkedin-4100200301').click();
    await expect(page.getByTestId('reader-title')).toContainText(
      'Head of Controlling Transformation',
    );
    await page.getByTestId('reader-trash').click();
    await expect(row(page, 'linkedin-4100200301')).toHaveCount(0);
    await page.keyboard.press('Control+z');
    await expect(row(page, 'linkedin-4100200301')).toBeVisible();
    await expect(page.getByTestId('reader-title')).toContainText(
      'Head of Controlling Transformation',
    );
    // The job read before it stays listed too (Neu keeps the jobs read in this visit).
    await expect(row(page, 'freelancermap-2801')).toBeVisible();
    // One call per move, back to where it came from.
    const back = (await calls(page, 'move_jobs')).at(-1)?.[1] as { to: string };
    expect(back.to).toBe('inbox');
  });

  test('Ctrl+Z takes back the newest move, also when it joined an older toast', async ({
    page,
  }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await tool(page, 'archive', 'freelancermap-2803');
    await settleMoves(page);
    await tool(page, 'trash', 'freelancermap-2802');
    await settleMoves(page);
    await tool(page, 'archive', 'linkedin-4100200302');
    await expect(page.getByTestId('toast-text').filter({ hasText: '2 Jobs' })).toBeVisible();
    await page.keyboard.press('Control+z');
    await expect(row(page, 'linkedin-4100200302')).toBeVisible();
    await expect(row(page, 'freelancermap-2803')).toBeVisible();
    // The trash toast came later, but its move was not the newest: it stays.
    await expect(row(page, 'freelancermap-2802')).toHaveCount(0);
    await expect(page.getByTestId('toast-action')).toHaveCount(1);
  });

  test('an undo that fails says so in the list header until the next list', async ({ page }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await tool(page, 'archive', 'freelancermap-2803');
    await failNext(page, 'move_jobs');
    await page.getByTestId('toast-action').click();
    await expect(page.getByTestId('header-error')).toHaveText('Die Datenbank meldet einen Fehler.');
    await facet(page, 'Neu').click();
    await expect(page.getByTestId('header-error')).toHaveCount(0);
  });

  test('a star that fails says so', async ({ page }) => {
    await open(page, WIN);
    await failNext(page, 'set_pinned');
    await tool(page, 'pin', 'linkedin-4100200301');
    await expect(page.getByTestId('header-error')).toHaveText('Die Datenbank meldet einen Fehler.');
  });

  test('deleting a job for good keeps the undo of another one', async ({ page }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await tool(page, 'archive', 'freelancermap-2803');
    await settleMoves(page);
    await tool(page, 'trash', 'freelancermap-2802');
    await page.getByTestId('nav-trash').click();
    await settle(page);
    await tool(page, 'purge', 'freelancermap-2802');
    await page.getByTestId('dialog-purge').getByTestId('dialog-confirm').click();
    await expect(page.getByTestId('toast-text').filter({ hasText: 'gelöscht' })).toBeVisible();
    // The archive's undo is still there and works (once the dialog is gone).
    await expect(page.getByTestId('toast-action')).toHaveCount(1);
    await expect(page.getByTestId('dialog-purge')).toHaveCount(0);
    await page.keyboard.press('Control+z');
    await expect(page.getByTestId('toast-action')).toHaveCount(0);
    await page.getByTestId('nav-jobs').click();
    await settle(page);
    await expect(row(page, 'freelancermap-2803')).toBeVisible();
  });
});

test.describe('dates', () => {
  test('a relative date moves on while the app stays open', async ({ page }) => {
    await open(page, WIN);
    const date = row(page, 'linkedin-4100200301').locator('.date');
    const before = await date.textContent();
    await page.clock.setFixedTime(new Date(NOW.getTime() + 5 * 3_600_000));
    await page.evaluate(() => dispatchEvent(new Event('focus')));
    await expect(date).not.toHaveText(before ?? '');
  });

  test('in the Papierkorb a row shows the day the job went there, and no star', async ({
    page,
  }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await expect(row(page, 'linkedin-4100200303').locator('.date')).toHaveText('gestern');
    await tool(page, 'trash', 'linkedin-4100200303');
    await settleMoves(page);
    await tool(page, 'trash', 'freelancermap-2801');
    await page.getByTestId('nav-trash').click();
    await settle(page);
    await expect(row(page, 'linkedin-4100200303').locator('.date')).toHaveText('jetzt');
    // By date: the day the job went there, which its row shows.
    await page.getByTestId('sort').click();
    const menu = (await page.evaluate(() => window.__harness.menus)).at(-1) ?? [];
    expect(menu.map((entry) => entry.text)).toContain('Nach Datum');
    await page.keyboard.press('Escape');
    // A favourite in the trash is none: no star on its row.
    await expect(row(page, 'freelancermap-2801')).toBeVisible();
    await expect(
      list(page)
        .locator('.job', { has: row(page, 'freelancermap-2801') })
        .locator('.mark'),
    ).toHaveCount(0);
  });
});

test.describe('the empty list and the search', () => {
  test('reading older mails from the empty list asks first, as in Einstellungen', async ({
    page,
  }) => {
    await open(page, `${WIN}&scenario=empty`);
    await page.getByTestId('read-older').click();
    const dialog = page.getByTestId('dialog-read-older');
    await expect(dialog).toBeVisible();
    expect(await calls(page, 'start_run')).toEqual([]);
    await dialog.getByTestId('dialog-confirm').click();
    await expect.poll(async () => (await calls(page, 'start_run')).length).toBe(1);
  });

  test('a portal page that does not open says so', async ({ page }) => {
    await open(page, `${WIN}&scenario=empty`);
    await failNext(page, 'open_target');
    await page.getByTestId('alert-linkedin').click();
    await expect(page.getByTestId('portal-error')).toBeVisible();
  });

  test('the links to other places centre under an empty search', async ({ page }) => {
    await open(page, WIN);
    await page.getByTestId('search').fill('Kreditoren');
    const empty = page.getByTestId('empty-search');
    const also = page.getByTestId('also-in').getByRole('button').first();
    await expect(also).toBeVisible();
    const a = (await empty.boundingBox())!;
    const b = (await also.boundingBox())!;
    expect(Math.abs(a.x + a.width / 2 - (b.x + b.width / 2))).toBeLessThan(2);
  });

  test('deleting for good waits for a run on the row tool too', async ({ page }) => {
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await tool(page, 'trash', 'freelancermap-2803');
    await page.getByTestId('nav-trash').click();
    await settle(page);
    await page.evaluate(() => (window.__harness.holdAfter = 1));
    await page.getByTestId('fetch').click();
    await row(page, 'freelancermap-2803').hover();
    await expect(page.getByTestId('purge-freelancermap-2803')).toHaveAttribute(
      'aria-disabled',
      'true',
    );
    await page.evaluate(() => (window.__harness.holdAfter = null));
  });
});
