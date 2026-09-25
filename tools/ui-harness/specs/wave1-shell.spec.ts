// Wave 1, track "shell": the scrollbar's room in the Jobs view, the keys of lists, panes and
// radio groups, the sidebar in the first run and its one-line run status, the closing note
// per activity, the shared components (switch, buttons, notices, badges, toasts, chips) and
// the demo data that agrees with itself.

import type { Locator, Page } from '@playwright/test';
import { calls, expect, NOW, open, runFinished, settle, test } from './fixtures';

const WIN = '?platform=windows';
const MAC = '?platform=macos';
const DAY = 24 * 60 * 60 * 1000;

const rows = (page: Page): Locator =>
  page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
const row = (page: Page, key: string): Locator =>
  page.getByTestId('job-list').getByTestId(`job-row-${key}`);

async function settings(page: Page, query = WIN): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('view-settings')).toBeVisible();
  await settle(page);
}

/** A colour token as the page computes it (rgb()). */
async function tokenColour(page: Page, name: string): Promise<string> {
  return page.evaluate((token) => {
    const probe = document.createElement('span');
    probe.style.color = `var(${token})`;
    document.body.append(probe);
    const colour = getComputedStyle(probe).color;
    probe.remove();
    return colour;
  }, name);
}

test('with scrollbars shown the list and the reader keep their room: edges line up, nothing jumps', async ({
  playwright,
  browserName,
  baseURL,
}) => {
  test.skip(browserName !== 'chromium', 'Chromium hides scrollbars by a switch left out here');
  // A browser of its own that shows the scrollbars (the harness hides them by default).
  const browser = await playwright.chromium.launch({ ignoreDefaultArgs: ['--hide-scrollbars'] });
  try {
    const context = await browser.newContext({
      baseURL,
      viewport: { width: 1360, height: 900 },
      deviceScaleFactor: 1,
      locale: 'de-DE',
      timezoneId: 'Europe/Berlin',
    });
    const page = await context.newPage();
    await open(page, WIN);
    await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
    const list = page.getByTestId('list-scroll');
    // The room of the Windows scrollbar is there (8 px), whether the list scrolls or not.
    const gutter = await list.evaluate(
      (node) => (node as HTMLElement).offsetWidth - node.clientWidth,
    );
    expect(gutter).toBe(8);
    // The header ends where the rows end (their text shares one edge).
    const edges = await page.evaluate(() => {
      const scroller = document.querySelector('[data-testid="list-scroll"]')!;
      const header = scroller.parentElement!.querySelector(':scope > .head > *')!;
      const first = scroller.querySelector('[data-testid^="job-row-"]')!;
      return {
        header: header.getBoundingClientRect().right,
        row: first.getBoundingClientRect().right,
      };
    });
    expect(Math.abs(edges.header - edges.row)).toBeLessThanOrEqual(0.5);
    // A long and a short job: the reader's close button stands at one place.
    const closeAt = async (key: string): Promise<number> => {
      await row(page, key).click();
      await expect(page.getByTestId('reader-close')).toBeVisible();
      await settle(page);
      return (await page.getByTestId('reader-close').boundingBox())!.x;
    };
    const long = await closeAt('freelancermap-2801');
    const short = await closeAt('freelance-900411');
    expect(short).toBe(long);
  } finally {
    await browser.close();
  }
});

test('keyboard focus stays clear of the macOS toolbar band and the reader bar', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1100, height: 600 });
  // The centre of the focused control is the control itself, not the band over it.
  const covered = async (): Promise<string | null> =>
    page.evaluate(() => {
      const node = document.activeElement;
      if (!(node instanceof HTMLElement) || node === document.body) return null;
      const box = node.getBoundingClientRect();
      const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
      return hit !== null && (node === hit || node.contains(hit))
        ? null
        : (node.getAttribute('data-testid') ?? node.tagName);
    });
  await settings(page, MAC);
  await page.getByTestId('reset').focus();
  const cut: string[] = [];
  for (let stop = 0; stop < 16; stop += 1) {
    await page.keyboard.press('Shift+Tab');
    await page.waitForTimeout(80);
    const hidden = await covered();
    if (hidden !== null) cut.push(hidden);
  }
  // The reader: from its end upward, below the compact bar.
  await page.getByTestId('nav-jobs').click();
  await row(page, 'freelancermap-2801').click();
  await expect(page.getByTestId('reader-close')).toBeVisible();
  await page.getByTestId('stage').evaluate((node) => (node.scrollTop = node.scrollHeight));
  await page.getByTestId('reader-close').focus();
  await page.getByTestId('stage').evaluate((node) => (node.scrollTop = node.scrollHeight));
  for (let stop = 0; stop < 12; stop += 1) {
    await page.keyboard.press('Shift+Tab');
    await page.waitForTimeout(80);
    const inReader = await page.evaluate(
      () =>
        document.querySelector('[data-testid="reader-pane"]')?.contains(document.activeElement) ??
        false,
    );
    const hidden = inReader ? await covered() : null;
    if (hidden !== null) cut.push(`reader ${hidden}`);
  }
  expect(cut).toEqual([]);
});

test('a switch darkens a step on hover and one more while pressed, off and on', async ({
  page,
}) => {
  await settings(page);
  const steps = async (id: string): Promise<string[]> => {
    const toggle = page.getByTestId(id);
    const track = toggle.locator('.track');
    const colour = (): Promise<string> =>
      track.evaluate((node) => getComputedStyle(node).backgroundColor);
    await page.mouse.move(0, 0);
    await page.waitForTimeout(250);
    const rest = await colour();
    const box = (await toggle.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.waitForTimeout(250);
    const hover = await colour();
    await page.mouse.down();
    await page.waitForTimeout(250);
    const pressed = await colour();
    await page.mouse.up();
    return [rest, hover, pressed];
  };
  for (const id of ['toggle-login-freelance', 'toggle-auto-fetch']) {
    const [rest, hover, pressed] = await steps(id);
    expect(new Set([rest, hover, pressed]).size, `${id}: ${rest} ${hover} ${pressed}`).toBe(3);
  }
});

test('only what loses something for good warns: the trash does not, delete for good does', async ({
  page,
}) => {
  await open(page, WIN);
  const danger = await tokenColour(page, '--danger-strong');
  await rows(page).first().click();
  await page.keyboard.press('Shift+ArrowDown');
  const trash = page.getByTestId('selection-trash');
  await expect(trash).toBeVisible();
  await trash.hover();
  // The trash can be undone: it looks like every other icon on hover.
  await expect(trash).not.toHaveCSS('color', danger);
  // In the trash the bar's "Endgültig löschen" loses the jobs for good: it warns.
  await trash.click();
  await page.getByTestId('nav-trash').click();
  await rows(page).first().click();
  await page.keyboard.press('Shift+ArrowDown');
  const purge = page.getByTestId('selection-purge');
  await purge.hover();
  await expect(purge).toHaveCSS('color', danger);
});

test('a dialog confirms with the bare verb of its heading', async ({ page }) => {
  await settings(page);
  await page.getByTestId('full-mailbox').click();
  await expect(page.getByTestId('dialog-full-mailbox').getByTestId('dialog-confirm')).toHaveText(
    'Lesen',
  );
  await page.keyboard.press('Escape');
  await page.getByTestId('nav-jobs').click();
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await row(page, 'freelancermap-2803').hover();
  await page.getByTestId('trash-freelancermap-2803').click();
  await page.getByTestId('nav-trash').click();
  await page.getByTestId('empty-trash').click();
  await expect(page.getByTestId('dialog-empty-trash').getByTestId('dialog-confirm')).toHaveText(
    'Leeren',
  );
  await page.keyboard.press('Escape');
  await rows(page).first().hover();
  await page.getByTestId('purge-freelancermap-2803').click();
  await expect(page.getByTestId('dialog-purge').getByTestId('dialog-confirm')).toHaveText(
    'Löschen',
  );
});

test('first run: Einstellungen and Profil open, Jobs and its places lead to the setup', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=first-run`);
  const nav = page.getByTestId('sidebar').locator('nav');
  await expect(nav.locator('xpath=ancestor-or-self::*[@inert]')).toHaveCount(0);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('view-settings')).toBeVisible();
  // The language can be chosen before anything is set up.
  await page.getByTestId('language').getByRole('radio', { name: 'English' }).click();
  await expect(page.getByTestId('nav-settings')).toContainText('Settings');
  await page.getByTestId('language').getByRole('radio', { name: 'Deutsch' }).click();
  await expect(page.getByTestId('nav-settings')).toContainText('Einstellungen');
  await page.getByTestId('nav-profile').click();
  await expect(page.getByTestId('view-profile')).toBeVisible();
  // The trash leads to the setup page, which Jobs marks as current like any view.
  await page.getByTestId('nav-trash').click();
  await expect(page.getByTestId('view-first-run')).toBeVisible();
  await expect(page.locator('[data-testid^="nav-"][aria-current="page"]')).toHaveCount(1);
  await expect(page.getByTestId('nav-jobs')).toHaveAttribute('aria-current', 'page');
});

test('first run: a place clicked before the first fetch leaves the list in the inbox', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=mailbox-only`);
  await page.getByTestId('first-profile').click();
  await expect(page.getByTestId('view-profile')).toBeVisible();
  await page.getByTestId('nav-trash').click();
  await expect(page.getByTestId('view-first-run')).toBeVisible();
  await page.getByTestId('first-fetch').click();
  await runFinished(page);
  await expect(page.getByTestId('nav-jobs')).toHaveAttribute('aria-current', 'page');
  await expect(rows(page).first()).toBeVisible();
  // Without a profile nothing is excluded.
  await expect(page.getByTestId('excluded-rows')).toHaveCount(0);
});

test('one column: a running fetch shows in the sidebar while a job is open', async ({ page }) => {
  await page.setViewportSize({ width: 820, height: 640 });
  await open(page, WIN);
  await page.evaluate(() => (window.__harness.holdAfter = 6));
  await page.getByTestId('fetch').click();
  await expect(page.getByTestId('run-card')).toBeVisible();
  await expect(page.getByTestId('run-status')).toHaveCount(0);
  await rows(page).first().click();
  await expect(page.getByTestId('list-scroll')).toBeHidden();
  const status = page.getByTestId('run-status');
  await expect(status).toBeVisible();
  await status.click();
  await expect(page.getByTestId('list-scroll')).toBeVisible();
  await expect(page.getByTestId('run-card')).toBeVisible();
  await expect(page.getByTestId('cancel-run')).toBeVisible();
  await page.evaluate(() => (window.__harness.holdAfter = null));
});

test('the run status is one line as high as a nav entry, today and on another day', async ({
  page,
}) => {
  const check = async (text: string): Promise<void> => {
    const status = page.getByTestId('run-status');
    await expect(status).toContainText(text);
    const line = await status.evaluate((node) => {
      const text = node.querySelector('.text')!;
      return {
        lines: Math.round(text.getBoundingClientRect().height / 18),
        height: node.getBoundingClientRect().height,
      };
    });
    const nav = (await page.getByTestId('nav-jobs').boundingBox())!.height;
    expect(line.lines, text).toBe(1);
    expect(line.height, text).toBe(nav);
  };
  const nextDay = async (): Promise<void> => {
    await page.clock.setFixedTime(new Date(NOW.getTime() + DAY));
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  };
  for (const [lang, fetched, failed, cancelled] of [
    ['de', 'Abgerufen', 'Fehler', 'Abgebrochen'],
    ['en', 'Fetched', 'Failed', 'Cancelled'],
  ] as const) {
    await settings(page, `${WIN}&lang=${lang}`);
    await check(`${fetched} 08:30`);
    await nextDay();
    await check(`${fetched} ${lang === 'de' ? '24.09.' : '24/09'}`);
    await settings(page, `${WIN}&lang=${lang}&scenario=offline`);
    await check(`${failed} 08:30`);
    await nextDay();
    await check(`${failed} ${lang === 'de' ? '24.09.' : '24/09'}`);
    // A fetch cancelled: the status says so, not that it fetched.
    await open(page, `${WIN}&lang=${lang}`);
    await page.evaluate(() => (window.__harness.holdAfter = 4));
    await page.getByTestId('fetch').click();
    await page.getByTestId('cancel-run').click();
    await page.evaluate(() => (window.__harness.holdAfter = null));
    await page.getByTestId('nav-settings').click();
    await check(cancelled);
    await nextDay();
    await check(`${cancelled} ${lang === 'de' ? '24.09.' : '24/09'}`);
  }
});

test('the status opens the run only once an unsaved Profil lets the view go', async ({ page }) => {
  await open(page, WIN);
  await row(page, 'freelancermap-2801').click();
  await expect(page.getByTestId('reader-close')).toBeVisible();
  await page.getByTestId('nav-profile').click();
  await page.getByTestId('profile-name-field').fill('Erika Muster');
  await page.getByTestId('run-status').click();
  const dialog = page.getByTestId('dialog-leave-profile');
  await dialog.getByRole('button', { name: 'Abbrechen' }).click();
  await expect(page.getByTestId('view-profile')).toBeVisible();
  // Later, Jobs without the status: the open job stays and no run card came up.
  await page.getByTestId('nav-jobs').click();
  await dialog.getByRole('button', { name: 'Verwerfen' }).click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await expect(page.getByTestId('reader-close')).toBeVisible();
  await expect(page.getByTestId('run-card')).toHaveCount(0);
});

test('the closing note names what the window waits for', async ({ page }) => {
  await open(page, WIN);
  await page.evaluate(() => {
    window.__harness.appRun('rescore');
    window.__harness.fire('closing', { activity: 'rescore' });
  });
  await expect(page.getByTestId('closing')).toHaveText(
    'Das Bewerten wird beendet, dann schließt die App.',
  );
  await open(page, WIN);
  await page.evaluate(() => window.__harness.fire('closing', { activity: 'session' }));
  await expect(page.getByTestId('closing')).toHaveText(
    'Die Anmeldung wird beendet, dann schließt die App.',
  );
  // An older backend sends nothing: the fetch, as before.
  await open(page, WIN);
  await page.evaluate(() => window.__harness.fire('closing', null));
  await expect(page.getByTestId('closing')).toHaveText(
    'Der Abruf wird beendet, dann schließt die App.',
  );
});

test('Tab passes disabled buttons and switches, like native ones', async ({ page }) => {
  await settings(page, `${WIN}&scenario=dry-run`);
  await page.getByTestId('nav-settings').focus();
  const landed: string[] = [];
  for (let stop = 0; stop < 40; stop += 1) {
    await page.keyboard.press('Tab');
    const disabled = await page.evaluate(() =>
      document.activeElement?.getAttribute('aria-disabled') === 'true'
        ? (document.activeElement.getAttribute('data-testid') ?? document.activeElement.tagName)
        : null,
    );
    if (disabled !== null) landed.push(disabled);
  }
  expect(landed).toEqual([]);
  // Still hoverable: the reason shows.
  await page.getByTestId('reset').hover();
  await expect(page.getByRole('tooltip')).toBeVisible();
});

test('Shift with the arrows, Home and End chooses jobs from the open one', async ({ page }) => {
  await open(page, WIN);
  const first = rows(page).first();
  await first.click();
  await page.keyboard.press('Shift+ArrowDown');
  await page.keyboard.press('Shift+ArrowDown');
  await expect(page.getByTestId('selection-pane')).toContainText('3');
  await expect(rows(page).and(page.locator('[aria-current="true"]'))).toHaveCount(3);
  // Back up one: two stay chosen.
  await page.keyboard.press('Shift+ArrowUp');
  await expect(rows(page).and(page.locator('[aria-current="true"]'))).toHaveCount(2);
  // Shift+End: from the open job to the last row.
  await page.keyboard.press('Shift+End');
  const all = await rows(page).count();
  await expect(rows(page).and(page.locator('[aria-current="true"]'))).toHaveCount(all);
  // Back to one job: Esc clears the choice.
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('selection-pane')).toHaveCount(0);
});

test('a radio group takes Home and End too; the list does not', async ({ page }) => {
  await open(page, WIN);
  const facet = page.getByTestId('facet');
  const radios = facet.getByRole('radio');
  const checked = facet.locator('[aria-checked="true"]');
  await checked.focus();
  const before = (await calls(page, 'job_detail')).length;
  await page.keyboard.press('End');
  await expect(radios.last()).toHaveAttribute('aria-checked', 'true');
  await expect(radios.last()).toBeFocused();
  await page.keyboard.press('Home');
  await expect(radios.first()).toHaveAttribute('aria-checked', 'true');
  await expect(radios.first()).toBeFocused();
  await expect(page.getByTestId('reader-close')).toHaveCount(0);
  expect((await calls(page, 'job_detail')).length).toBe(before);
});

test('with the focus nowhere the arrows, Home and End scroll Einstellungen', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 600 });
  await settings(page);
  const view = page.getByTestId('view-settings');
  const top = (): Promise<number> => view.evaluate((node) => node.scrollTop);
  await page.getByTestId('settings-fetch').locator('h2').click();
  await page.keyboard.press('End');
  await expect
    .poll(() => view.evaluate((node) => node.scrollHeight - node.clientHeight - node.scrollTop))
    .toBeLessThanOrEqual(1);
  const bottom = await top();
  await page.keyboard.press('ArrowUp');
  await expect.poll(top).toBe(bottom - 40);
  await page.keyboard.press('Home');
  await expect.poll(top).toBe(0);
  await page.keyboard.press('ArrowDown');
  await expect.poll(top).toBe(40);
});

test('after a click into the reader the arrows scroll it; Space on the open row pages it', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1100, height: 600 });
  await open(page, WIN);
  await row(page, 'freelancermap-2801').click();
  const title = page.getByTestId('reader-title');
  await expect(title).toBeVisible();
  const stage = page.getByTestId('stage');
  const top = (): Promise<number> => stage.evaluate((node) => node.scrollTop);
  // Space on the open row pages through the reader; Shift+Space back.
  await row(page, 'freelancermap-2801').focus();
  await page.keyboard.press('Space');
  await expect.poll(top).toBeGreaterThan(100);
  await page.keyboard.press('Shift+Space');
  await expect.poll(top).toBe(0);
  const opened = await title.innerText();
  // A click on the ad's text: the arrows scroll the reader, the job stays.
  await title.click();
  await page.keyboard.press('ArrowDown');
  await expect.poll(top).toBe(40);
  await page.keyboard.press('End');
  await expect
    .poll(() => stage.evaluate((node) => node.scrollHeight - node.clientHeight - node.scrollTop))
    .toBeLessThanOrEqual(1);
  await expect(title).toHaveText(opened);
  // A click in the list gives the arrows back to it.
  await row(page, 'freelancermap-2801').click();
  await page.keyboard.press('ArrowDown');
  await expect(title).not.toHaveText(opened);
});

test('a chip value copies; a double click still edits it; its x has a tooltip', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('nav-profile').click();
  const keywords = page.getByTestId('profile-keywords');
  const chip = keywords.locator('[data-chip]').first();
  const text = chip.locator('.text');
  await text.scrollIntoViewIfNeeded();
  const value = await text.innerText();
  // A drag over the value selects it.
  const box = (await text.boundingBox())!;
  await page.mouse.move(box.x + 1, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 1, box.y + box.height / 2, { steps: 4 });
  await page.mouse.up();
  const selected = await page.evaluate(() => getSelection()?.toString() ?? '');
  expect(value.startsWith(selected.trim())).toBe(true);
  expect(selected.trim().length).toBeGreaterThan(1);
  // The x names what it removes.
  await chip.locator('.remove').hover();
  await expect(page.getByRole('tooltip')).toHaveText(`${value} entfernen`);
  // A double click takes the value back into the field, with no word selected.
  await text.dblclick();
  await expect(keywords.locator('input')).toHaveValue(value);
  expect(await page.evaluate(() => getSelection()?.toString() ?? '')).toBe('');
});

test('a neutral badge stands off the wash of the selected row', async ({ page }) => {
  await open(page, WIN);
  const teaser = row(page, 'freelance-900411');
  await teaser.click();
  const badge = teaser.locator('.badge.neutral').first();
  await expect(badge).toBeVisible();
  const white = await tokenColour(page, '--surface');
  await expect(badge).toHaveCSS('background-color', white);
});

test('a cut title in a toast: the closing quote follows the ellipsis', async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 360 });
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await row(page, 'freelancermap-2801').hover();
  await page.getByTestId('archive-freelancermap-2801').click();
  const name = page.getByTestId('toast-text').locator('.name');
  await expect(name).toHaveText(/…$/);
  const gap = await name.evaluate((node) => {
    const range = document.createRange();
    range.selectNodeContents(node);
    return node.getBoundingClientRect().right - range.getBoundingClientRect().right;
  });
  expect(gap).toBeLessThanOrEqual(1);
  // The whole title in the tooltip.
  await name.hover();
  await expect(page.getByRole('tooltip')).toContainText('Interim CFO');
});

test('ghost buttons at the end of a row end on the edge of the switches', async ({ page }) => {
  await settings(page);
  const edge = async (locator: Locator): Promise<number> =>
    locator.evaluate((node) => node.getBoundingClientRect().right);
  const toggle = await edge(page.getByTestId('toggle-auto-fetch'));
  for (const id of ['mailbox-remove', 'excel-open', 'logs-open']) {
    const label = await edge(page.getByTestId(id).locator('.label'));
    expect(Math.abs(label - toggle), id).toBeLessThanOrEqual(0.5);
  }
});

test('macOS: the first sidebar entry starts on the first line of a view', async ({ page }) => {
  await settings(page, MAC);
  const nav = (await page.getByTestId('nav-jobs').boundingBox())!.y;
  const heading = (await page.getByTestId('settings-mailbox').locator('h2').boundingBox())!.y;
  expect(nav).toBe(heading);
});

test('a notice banner shares the inset of the cards and draws no line of its own', async ({
  page,
}) => {
  await settings(page, `${WIN}&scenario=dry-run`);
  const banner = page.locator('.notice.banner').first();
  const icon = (await banner.locator(':scope > .icon').boundingBox())!.x;
  const labelId = await page.getByTestId('toggle-auto-fetch').getAttribute('aria-labelledby');
  const label = (await page.locator(`[id="${labelId}"]`).boundingBox())!.x;
  expect(icon).toBe(label);
  const same = async (notice: Locator): Promise<boolean> =>
    notice.evaluate((node) => {
      const style = getComputedStyle(node);
      return style.borderTopColor === style.backgroundColor;
    });
  expect(await same(banner)).toBe(true);
  // The warning of the reset report too.
  await open(page, `${WIN}&scenario=reset`);
  expect(await same(page.getByTestId('first-reset-report'))).toBe(true);
});

test('one glyph per action: retries load again, the reset keeps its own', async ({ page }) => {
  await open(page, `${WIN}&scenario=list-error`);
  const retry = page.getByTestId('list-error').getByRole('button');
  await expect(retry.locator('[data-icon]')).toHaveAttribute('data-icon', 'refresh-cw');
  await settings(page);
  await expect(page.getByTestId('reset').locator('[data-icon]')).toHaveAttribute(
    'data-icon',
    'rotate-ccw',
  );
  // Jobs has one glyph: in the sidebar, on "Zurückholen" and on its empty list.
  await page.getByTestId('nav-archive').click();
  await rows(page).first().hover();
  const back = page.locator('[data-testid^="toInbox-"]').first();
  await expect(back.locator('[data-icon]')).toHaveAttribute('data-icon', 'briefcase');
});

test('deleting for good names the job like a move; several by their number', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  for (const key of ['freelancermap-2802', 'freelancermap-2803', 'linkedin-4100200301']) {
    await row(page, key).hover();
    await page.getByTestId(`trash-${key}`).click();
    await expect(row(page, key)).toHaveCount(0);
    // A click right after the list changed is no click (a double click never hits the next).
    await page.waitForTimeout(600);
  }
  await page.getByTestId('nav-trash').click();
  await row(page, 'freelancermap-2802').hover();
  await page.getByTestId('purge-freelancermap-2802').click();
  await page.getByTestId('dialog-purge').getByTestId('dialog-confirm').click();
  await expect(page.getByTestId('toast-text').last()).toHaveText(
    '„Interim Head of Finance“ endgültig gelöscht.',
  );
  // A click right after the list changed is no click (a double click never hits the next).
  await page.waitForTimeout(600);
  await rows(page).first().click();
  await page.keyboard.press('Shift+ArrowDown');
  await page.getByTestId('selection-purge').click();
  await page.getByTestId('dialog-purge-chosen').getByTestId('dialog-confirm').click();
  await expect(page.getByTestId('toast-text').last()).toHaveText('2 Jobs endgültig gelöscht.');
});

test('the demo job says the same in its row, its reader and its prompt', async ({ page }) => {
  await open(page, WIN);
  const facts = row(page, 'freelancermap-2801').getByTestId('row-facts');
  await expect(facts).toContainText('ab sofort');
  await expect(facts).toContainText('1.200 €/Tag');
  await row(page, 'freelancermap-2801').click();
  const reader = page.getByTestId('reader');
  await expect(reader).toContainText('Start ab sofort');
  await expect(reader).toContainText('Tagessatz 1.200 €');
  await expect(reader).not.toContainText('nach Absprache');
  await expect(page.getByTestId('wishes')).toContainText('erreicht den Wunsch von 1.200');
  // Every criterion of the profile is stated and met: one quiet line of the values.
  await expect(page.getByTestId('criteria-clean')).toBeVisible();
  await expect(page.getByTestId('criteria')).toHaveCount(0);
});
