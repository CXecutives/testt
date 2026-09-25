// Wave 1, track jobs: the job list, its rows and the reader, and the errors they say (see
// docs/PLAN.md, UI "Jobs").

import type { Page } from '@playwright/test';
import { expect, open, runFinished, test } from './fixtures';

const WIN = '?platform=windows';
const list = (page: Page) => page.getByTestId('job-list');
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');
const row = (page: Page, key: string) => list(page).getByTestId(`job-row-${key}`);
const facet = (page: Page, name: string) =>
  page.getByTestId('facet').getByRole('radio', { name: new RegExp(name) });

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
