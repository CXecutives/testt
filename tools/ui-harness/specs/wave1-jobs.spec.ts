// Wave 1, track jobs: the job list, its rows and the reader, and the errors they say (see
// docs/PLAN.md, UI "Jobs").

import type { Locator, Page } from '@playwright/test';
import { calls, expect, NOW, open, runFinished, settle, test } from './fixtures';

const WIN = '?platform=windows';
const DAY = 86_400_000;
const list = (page: Page) => page.getByTestId('job-list');
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
const row = (page: Page, key: string) => list(page).getByTestId(`job-row-${key}`);
const facet = (page: Page, name: string) =>
  page.getByTestId('facet').getByRole('radio', { name: new RegExp(name) });

/** A row's tool: it exists while the pointer is on the row. */
async function tool(page: Page, id: string, key: string): Promise<void> {
  await row(page, key).hover();
  await page.getByTestId(`${id}-${key}`).click();
}

/** After a move the list ignores clicks for a moment (the row under the pointer changed). */
async function settleMoves(page: Page): Promise<void> {
  await page.waitForTimeout(550);
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

/** A token's colour as the engine computes it (for a text or a background). */
async function tokenColour(page: Page, token: string): Promise<string> {
  return page.evaluate((name) => {
    const probe = document.createElement('span');
    probe.style.color = `var(${name})`;
    document.body.append(probe);
    const value = getComputedStyle(probe).color;
    probe.remove();
    return value;
  }, token);
}

/** One frame of what the page shows while something loads. */
interface Frame {
  /** Milliseconds since the press (or since the page started). */
  t: number;
  overview: boolean;
  /** How visible the first placeholder shape is (its opacity times its ancestors'). */
  shape: number | null;
  done: boolean;
}

/**
 * Records every frame from now on (`fromPress`: from the next pointer press) until `done`
 * matches: whether the day overview is there, how visible the first shape of `shapes` is.
 */
function recordFrames(options: { shapes: string; done: string; fromPress: boolean }): void {
  const frames: Frame[] = [];
  (window as unknown as { __frames: Frame[] }).__frames = frames;
  let start: number | null = options.fromPress ? null : performance.now();
  if (options.fromPress) {
    document.addEventListener('pointerdown', () => (start ??= performance.now()), true);
  }
  const visible = (node: Element | null): number | null => {
    if (node === null) return null;
    let value = 1;
    for (let at: Element | null = node; at !== null; at = at.parentElement) {
      value *= Number(getComputedStyle(at).opacity);
    }
    return value;
  };
  const tick = (): void => {
    const done = document.querySelector(options.done) !== null;
    if (start !== null) {
      frames.push({
        t: performance.now() - start,
        overview: document.querySelector('[data-testid="day-overview"]') !== null,
        shape: visible(document.querySelector(options.shapes)),
        done,
      });
    }
    if (!done) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

const framesOf = (page: Page): Promise<Frame[]> =>
  page.evaluate(() => (window as unknown as { __frames: Frame[] }).__frames);

test.describe('placeholders wait once', () => {
  test('a slow job keeps the overview, then its placeholder is there at once', async ({ page }) => {
    await open(page, WIN);
    await page.evaluate(() => (window.__harness.detailDelay = 1500));
    await page.evaluate(recordFrames, {
      shapes: '[data-testid="reader-skeleton"] .skeleton',
      done: '[data-testid="reader-title"]',
      fromPress: true,
    });
    await rows(page).first().click();
    await expect(page.getByTestId('reader-title')).toBeVisible({ timeout: 5000 });
    const frames = await framesOf(page);
    const first = frames.findIndex((frame) => frame.shape !== null);
    expect(first).toBeGreaterThan(0);
    // Until the placeholder comes, the overview stays (the pane never goes blank).
    expect(frames[first]!.t).toBeGreaterThanOrEqual(250);
    expect(frames.slice(0, first).every((frame) => frame.overview)).toBe(true);
    // And once it comes, it is visible after its fade (no second wait of 300 ms; some room for
    // a loaded machine).
    const later = frames.find((frame) => frame.shape !== null && frame.t >= frames[first]!.t + 250);
    expect(later?.shape ?? 0).toBeGreaterThanOrEqual(0.9);
  });

  test('a slow list shows its placeholder rows whole at once', async ({ page }) => {
    await page.addInitScript(recordFrames, {
      shapes: '[data-testid="list-skeleton"] .skeleton',
      done: '[data-testid="job-rows"]',
      fromPress: false,
    });
    await open(page, `${WIN}&scenario=slow`);
    await expect(rows(page).first()).toBeVisible({ timeout: 10_000 });
    const frames = await framesOf(page);
    const first = frames.find((frame) => frame.shape !== null);
    expect(first).toBeDefined();
    const later = frames.find((frame) => frame.shape !== null && frame.t >= first!.t + 250);
    expect(later?.shape ?? 0).toBeGreaterThanOrEqual(0.9);
  });
});

test('deleting for good waits for a run in the reader too', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  await tool(page, 'trash', 'freelancermap-2803');
  await page.getByTestId('nav-trash').click();
  await settle(page);
  await row(page, 'freelancermap-2803').click();
  const purge = page.getByTestId('reader-purge');
  await expect(purge).toBeVisible();
  await expect(purge).not.toHaveAttribute('aria-disabled', 'true');
  await page.evaluate(() => (window.__harness.holdAfter = 1));
  await page.getByTestId('fetch').click();
  await expect(purge).toHaveAttribute('aria-disabled', 'true');
  await purge.hover();
  await expect(page.getByRole('tooltip')).toHaveText(/Abruf/);
  await purge.click({ force: true });
  await expect(page.getByTestId('dialog-purge')).toBeHidden();
  expect(await calls(page, 'purge_jobs')).toEqual([]);
  await page.evaluate(() => (window.__harness.holdAfter = null));
  await runFinished(page);
  await expect(purge).not.toHaveAttribute('aria-disabled', 'true');
});

test('a data folder that does not open says so in the first run', async ({ page }) => {
  await open(page, `${WIN}&scenario=reset`);
  await failNext(page, 'open_target');
  await page.getByTestId('first-reset-report').getByRole('button').click();
  await expect(page.getByTestId('folder-error')).toHaveText('Die Datenbank meldet einen Fehler.');
  expect(await calls(page, 'open_target')).toHaveLength(1);
});

test('a row date stands on the baseline of its title', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  const gaps = await list(page).evaluate((node) =>
    [...node.querySelectorAll('.job')].map((job) => {
      // The bottom of a zero-size inline block is its line's baseline.
      const baseline = (inside: Element): number => {
        const probe = document.createElement('span');
        probe.style.display = 'inline-block';
        probe.style.width = '0';
        probe.style.height = '0';
        inside.prepend(probe);
        const y = probe.getBoundingClientRect().bottom;
        probe.remove();
        return y;
      };
      return baseline(job.querySelector('.title')!) - baseline(job.querySelector('.date .stamp')!);
    }),
  );
  expect(gaps.length).toBeGreaterThan(5);
  for (const gap of gaps) expect(Math.abs(gap)).toBeLessThan(0.5);
  // The pinned row too (its star before the date), and a one-line row keeps its height.
  await expect(row(page, 'freelancermap-2801').locator('.mark')).toBeVisible();
  expect(Math.round((await row(page, 'linkedin-4100200301').boundingBox())!.height)).toBe(86);
});

test('the selected row deepens while pressed, only under the pointer', async ({ page }) => {
  await open(page, WIN);
  const target = row(page, 'linkedin-4100200301');
  await target.click();
  await expect(target).toHaveAttribute('aria-current', 'true');
  const rest = await tokenColour(page, '--surface-selected');
  const hover = await tokenColour(page, '--surface-selected-hover');
  const press = await tokenColour(page, '--surface-selected-press');
  expect(new Set([rest, hover, press]).size).toBe(3);
  const wash = (): Promise<string> =>
    target.evaluate((node) => getComputedStyle(node).backgroundColor);
  const box = (await target.boundingBox())!;
  const on = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const away = { x: box.x + box.width + 200, y: box.y + box.height + 200 };
  await page.mouse.move(away.x, away.y);
  await page.waitForTimeout(250);
  expect(await wash()).toBe(rest);
  await page.mouse.move(on.x, on.y);
  await page.waitForTimeout(250);
  expect(await wash()).toBe(hover);
  await page.mouse.down();
  await page.waitForTimeout(250);
  expect(await wash()).toBe(press);
  // Held and left: at rest again.
  await page.mouse.move(away.x, away.y, { steps: 4 });
  await page.waitForTimeout(250);
  expect(await wash()).toBe(rest);
  await page.mouse.up();
  // The middle button presses nothing.
  await page.mouse.move(on.x, on.y);
  await page.waitForTimeout(250);
  await page.mouse.down({ button: 'middle' });
  await page.waitForTimeout(250);
  expect(await wash()).toBe(hover);
  await page.mouse.up({ button: 'middle' });
  await page.mouse.move(on.x + 1, on.y + 1);
  await page.mouse.move(4, 4);
});

test('the history head darkens its chevron while pressed, only under the pointer', async ({
  page,
}) => {
  await open(page, `${WIN}&tick=15`);
  await page.getByTestId('fetch').click();
  await runFinished(page);
  const head = page.getByTestId('run-history').locator('button').first();
  const chevron = head.locator('.chevron');
  const pressed = await tokenColour(page, '--pressed');
  const colour = (): Promise<string> => chevron.evaluate((node) => getComputedStyle(node).color);
  const box = (await head.boundingBox())!;
  const on = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const away = { x: box.x + box.width + 200, y: box.y + box.height + 200 };
  await page.mouse.move(away.x, away.y);
  await page.waitForTimeout(250);
  const rest = await colour();
  await page.mouse.move(on.x, on.y);
  await page.waitForTimeout(250);
  const hover = await colour();
  expect(hover).not.toBe(pressed);
  await page.mouse.down();
  await page.waitForTimeout(250);
  expect(await colour()).toBe(pressed);
  await page.mouse.move(away.x, away.y, { steps: 4 });
  await page.waitForTimeout(250);
  expect(await colour()).toBe(rest);
  await page.mouse.up();
  await expect(head).toHaveAttribute('aria-expanded', 'false');
  // The middle button presses nothing.
  await page.mouse.move(on.x, on.y);
  await page.waitForTimeout(250);
  await page.mouse.down({ button: 'middle' });
  await page.waitForTimeout(250);
  expect(await colour()).toBe(hover);
  await page.mouse.up({ button: 'middle' });
  await page.mouse.move(on.x + 1, on.y + 1);
  await page.mouse.move(4, 4);
});

test('a detail state has one tone in the row, the reader and the run card', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  const key = { portal: 'freelancermap', id: '2805' } as const;
  const job = await page.evaluate((k) => window.__harness.job(k), key);
  await page.evaluate(
    (base) =>
      window.__harness.emit({
        type: 'jobUpdated',
        job: { ...base, detail: { kind: 'unfetchable' } },
        fresh: false,
      }),
    job!,
  );
  const badge = row(page, 'freelancermap-2805').locator('.badge');
  await expect(badge).toHaveText('Nicht abrufbar');
  await expect(badge).toHaveClass(/warning/);
  await row(page, 'freelancermap-2805').click();
  await expect(page.getByTestId('detail-note')).toHaveClass(/warning/);
  // A details run that found an ad gone says it as a warning, like the row and the reader.
  await page.evaluate(() => {
    const at = new Date(Date.now()).toISOString();
    window.__harness.emit({ type: 'started', kind: 'details' });
    window.__harness.emit({
      type: 'finished',
      summary: {
        run: 60,
        kind: 'details',
        outcome: { kind: 'completed' },
        dryRun: false,
        startedAt: at,
        finishedAt: at,
        scan: null,
        perPortal: [
          {
            portal: 'freelancermap',
            new: 0,
            known: 0,
            dup: 0,
            fetched: 0,
            failed: 1,
            gone: 1,
            skipped: 0,
            stopped: null,
          },
        ],
        newJobs: null,
        score: null,
        export: null,
        emptyAlerts: [],
      },
    });
  });
  await expect(page.getByTestId('details-gone')).toHaveClass(/warning/);
  await expect(page.getByTestId('details-failed')).toHaveClass(/warning/);
});

test('cut words in a row and in the compact bar show in full in a tooltip', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 700 });
  await open(page, `${WIN}&lang=en`);
  await facet(page, 'All').click();
  const labels = list(page).locator('.reason .label');
  const measure = (): Promise<boolean[]> =>
    labels.evaluateAll((nodes) => nodes.map((node) => node.scrollWidth > node.clientWidth));
  // The rows settle (fonts, the first layout) before a label counts as cut.
  await expect.poll(async () => (await measure()).includes(true)).toBe(true);
  const cut = await measure();
  const cutAt = cut.indexOf(true);
  const wholeAt = cut.indexOf(false);
  expect(cutAt).toBeGreaterThanOrEqual(0);
  expect(wholeAt).toBeGreaterThanOrEqual(0);
  const long = labels.nth(cutAt);
  await long.hover();
  await expect(page.getByRole('tooltip')).toHaveText((await long.textContent()) ?? '');
  await page.mouse.move(4, 4);
  await expect(page.getByRole('tooltip')).toHaveCount(0);
  await labels.nth(wholeAt).hover();
  await page.waitForTimeout(900);
  await expect(page.getByRole('tooltip')).toHaveCount(0);

  // The compact bar of a reader scrolled past its actions: a cut title shows in full.
  const title = 'Interim CFO for a family business with a focus on restructuring and financing';
  const job = await page.evaluate((key) => window.__harness.job(key), {
    portal: 'linkedin',
    id: '4100200301',
  } as const);
  await page.evaluate(
    ([base, long]) =>
      window.__harness.emit({ type: 'jobUpdated', job: { ...base, title: long }, fresh: false }),
    [job!, title] as const,
  );
  await row(page, 'linkedin-4100200301').click();
  await expect(page.getByTestId('reader-title')).toHaveText(title);
  await page.getByTestId('stage').evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
  const bar = page.getByTestId('reader-compact');
  await expect(bar).toHaveCSS('opacity', '1');
  const compact = bar.locator('.compact-title');
  expect(await compact.evaluate((node) => node.scrollWidth > node.clientWidth)).toBe(true);
  await compact.hover();
  await expect(page.getByRole('tooltip')).toHaveText(title);
});

for (const { width, height } of [
  { width: 480, height: 360 },
  { width: 510, height: 700 },
]) {
  test(`the reader's action row stays one line at ${width} px`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await open(page, WIN);
    await facet(page, 'Alle').click();
    await rows(page).first().click();
    const top = async (id: string): Promise<number> =>
      (await page.getByTestId(id).boundingBox())?.y ?? -1;
    await expect
      .poll(async () => {
        const ad = await top('open-ad');
        return ad >= 0 && (await top('open-mail')) === ad && (await top('prompt')) === ad;
      })
      .toBe(true);
    // The mail action is an icon then: its tooltip names it.
    await page.getByTestId('open-mail').hover();
    await expect(page.getByRole('tooltip')).toHaveText('Alert-Mail öffnen');
  });
}

test('the terms label takes the size of the line it labels', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  const size = (target: Locator): Promise<string> =>
    target.evaluate((node) => getComputedStyle(node).fontSize);
  await row(page, 'freelancermap-2801').click();
  // The strip as chips or, when every criterion is met, as one quiet line.
  const strip = page.getByTestId('criteria').or(page.getByTestId('criteria-clean'));
  await expect(strip).toBeVisible();
  expect(await size(strip.locator('.strip-label'))).toBe(
    await size(strip.locator('.chip, .clean-values').first()),
  );
  await row(page, 'linkedin-4100200301').click();
  const clean = page.getByTestId('criteria-clean');
  await expect(clean).toBeVisible();
  expect(await size(clean.locator('.strip-label'))).toBe(
    await size(clean.locator('.clean-values')),
  );
});

test('the compact bar is for the pointer: Tab reaches each tool once', async ({ page }) => {
  // A low window, so the ad scrolls its actions away in every engine.
  await page.setViewportSize({ width: 1360, height: 560 });
  await open(page, WIN);
  const opened = row(page, 'linkedin-4100200301');
  await opened.click();
  const bar = page.getByTestId('reader-compact');
  // Scroll to the end once the ad is there (it arrives after the head).
  await expect
    .poll(async () => {
      await page.getByTestId('stage').evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
      return bar.evaluate((node) => getComputedStyle(node).opacity);
    })
    .toBe('1');
  const tabIndexes = await bar
    .locator('button')
    .evaluateAll((nodes) => nodes.map((node) => (node as HTMLElement).tabIndex));
  expect(tabIndexes.length).toBeGreaterThan(3);
  expect(tabIndexes.every((index) => index === -1)).toBe(true);
  await opened.focus();
  const walk: string[] = [];
  for (let step = 0; step < 8; step++) {
    await page.keyboard.press('Tab');
    walk.push(
      await page.evaluate(
        () => (document.activeElement as HTMLElement | null)?.dataset.testid ?? '',
      ),
    );
  }
  expect(walk.filter((id) => id.startsWith('compact-'))).toEqual([]);
  expect(walk.filter((id) => id === 'reader-archive')).toHaveLength(1);
});

test('the trash says in how many days a job goes, following the clock', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  await tool(page, 'trash', 'freelancermap-2803');
  await settleMoves(page);
  await page.getByTestId('nav-trash').click();
  await settle(page);
  await row(page, 'freelancermap-2803').click();
  const line = page.getByTestId('place-line');
  await expect(line).toHaveText('Im Papierkorb, wird in 30 Tagen gelöscht');
  const later = async (days: number): Promise<void> => {
    await page.clock.setFixedTime(new Date(NOW.getTime() + days * DAY));
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  };
  await later(27);
  await expect(line).toHaveText('Im Papierkorb, wird in 3 Tagen gelöscht');
  await later(29.5);
  await expect(line).toHaveText('Im Papierkorb, wird in 1 Tag gelöscht');
  await later(30);
  await expect(line).toHaveText('Im Papierkorb, wird bald gelöscht');
});

test('a copy of the facts starts with the first fact', async ({ page }) => {
  await open(page, WIN);
  await facet(page, 'Alle').click();
  const copy = (target: Locator): Promise<string> =>
    target.evaluate((line) => {
      const selection = window.getSelection();
      selection?.selectAllChildren(line);
      return selection?.toString() ?? '';
    });
  await row(page, 'freelancermap-2801').click();
  // The open job's stage (the one on its way out has dropped its test id).
  const head = page.getByTestId('stage').locator('.head .facts');
  await expect.poll(() => copy(head)).toMatch(/^\S/);
  const facts = await copy(head);
  expect(facts).toMatch(/\S · \S/);
  await row(page, 'linkedin-4100200301').click();
  const clean = page.getByTestId('stage').getByTestId('criteria-clean').locator('.clean-values');
  await expect.poll(() => copy(clean)).toMatch(/^\S/);
});
