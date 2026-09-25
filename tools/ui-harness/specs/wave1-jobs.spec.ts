// Wave 1, track jobs: the job list, its rows and the reader, and the errors they say (see
// docs/PLAN.md, UI "Jobs").

import type { Page } from '@playwright/test';
import { expect, open, test } from './fixtures';

const WIN = '?platform=windows';
const rows = (page: Page) => page.getByTestId('job-rows').locator('[data-testid^="job-row-"]');

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
