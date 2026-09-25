// Shared test fixture: every console error, page error and CSP violation fails the test.

import { test as base, expect, type Page } from '@playwright/test';

/**
 * Pages currently being captured. For a screenshot Playwright injects its own <style>
 * (caret, animations); WebKit reports that as a CSP violation of the page. Those, and only
 * those, are not the app's fault.
 */
const capturing = new WeakSet<Page>();

export const test = base.extend<{ problems: string[] }>({
  problems: [
    async ({ page }, use) => {
      const problems: string[] = [];
      page.on('console', (message) => {
        if (message.type() === 'error' && !capturing.has(page)) {
          problems.push(`console: ${message.text()}`);
        }
      });
      page.on('pageerror', (error) => problems.push(`page: ${error.message}`));
      await page.addInitScript(() => {
        document.addEventListener('securitypolicyviolation', (event) => {
          console.error(`CSP violation: ${event.violatedDirective} ${event.blockedURI}`);
        });
      });
      await use(problems);
      expect(problems, 'console errors, page errors or CSP violations').toEqual([]);
    },
    { auto: true },
  ],
});

export { expect };

/** The fixed "now" of the stub data (relative dates stay the same every day). */
export const NOW = new Date('2026-09-24T09:30:00+02:00');

/** Open a page of the harness build and wait until fonts and the first frame are ready. */
export async function open(page: Page, query = ''): Promise<void> {
  await page.clock.setFixedTime(NOW);
  await page.goto(`/${query}`);
  await settle(page);
}

/** Names of the IPC commands called so far. */
export async function calls(page: Page, name?: string): Promise<[string, unknown][]> {
  const all = await page.evaluate(() => window.__harness.calls);
  return name ? all.filter(([command]) => command === name) : all;
}

/** Wait until a started run has finished in the stub. */
export async function runFinished(page: Page): Promise<void> {
  await page.waitForFunction(() => window.__harness.done, null, { timeout: 15_000 });
  await settle(page);
}

/** Visible elements matching a selector. */
export async function visibleCount(page: Page, selector: string): Promise<number> {
  await viewsSettled(page);
  return page.evaluate((css) => {
    return [...document.querySelectorAll(css)].filter((node) => {
      const box = node.getBoundingClientRect();
      return box.width > 0 && box.height > 0;
    }).length;
  }, selector);
}

/**
 * A view switch cross-fades: for 100 ms the old view fades out below the new one. What the
 * screen shows is counted once only the new view is left.
 */
export async function viewsSettled(page: Page): Promise<void> {
  await page.waitForFunction(() => document.querySelectorAll('main.views > section').length <= 1);
}

/**
 * A defined machine for a timing check: slows the CPU of the page (Chromium, through the
 * DevTools protocol) until a fixed piece of script and DOM work takes REFERENCE_MS, as long
 * as on the development machine throttled 4x (it takes 5 ms there unthrottled, the throttling
 * itself adds a little). A fast machine is slowed more, a slower CI runner less, so both check
 * the same speed. Returns the rate (1 = unthrottled: this machine is already as slow as the
 * reference; other engines are not throttled). HARNESS_CPU_RATE=4 sets a rate of its own
 * instead (to compare with a trace at that rate).
 */
export async function asReferenceMachine(page: Page): Promise<number> {
  if (page.context().browser()?.browserType().name() !== 'chromium') return 1;
  const forced = Number(process.env.HARNESS_CPU_RATE);
  if (forced >= 1) {
    const session = await page.context().newCDPSession(page);
    await session.send('Emulation.setCPUThrottlingRate', { rate: forced });
    test.info().annotations.push({ type: 'cpu', description: `throttled ${forced}x (set)` });
    return forced;
  }
  const REFERENCE_MS = 24;
  const measure = (): Promise<number> =>
    page.evaluate(() => {
      const now = (): number => new Event('probe').timeStamp;
      let best = Number.POSITIVE_INFINITY;
      for (let round = 0; round < 7; round += 1) {
        const start = now();
        // Script: objects, sorting and JSON, as rendering a list does.
        const items = Array.from({ length: 4000 }, (_, i) => ({
          id: `job-${(i * 7919) % 4000}`,
          title: `Interim Controller ${i}`,
          score: (i * 37) % 100,
        }));
        items.sort((a, b) => b.score - a.score || (a.id < b.id ? -1 : 1));
        const back = JSON.parse(JSON.stringify(items)) as typeof items;
        let sum = 0;
        for (const item of back) sum += item.title.length;
        // The DOM: build, lay out and drop a block of elements, as a window of rows does.
        const host = document.createElement('div');
        for (let i = 0; i < 400; i += 1) {
          const row = document.createElement('div');
          const title = document.createElement('span');
          title.textContent = back[i]!.title;
          row.append(title, String(sum % (i + 1)));
          host.append(row);
        }
        document.body.append(host);
        void host.offsetHeight;
        host.remove();
        best = Math.min(best, now() - start);
      }
      return best;
    });
  const cdp = await page.context().newCDPSession(page);
  const plain = await measure();
  const clamp = (value: number): number => Math.min(8, Math.max(1, value));
  let rate = clamp(REFERENCE_MS / plain);
  // The throttling finds its pace over the first rounds at a new rate: those are not counted.
  const throttled = async (): Promise<number> => {
    await cdp.send('Emulation.setCPUThrottlingRate', { rate });
    await measure();
    return measure();
  };
  // The time does not grow in proportion to the rate (the throttling costs something of its
  // own): the rate is found by secant steps from the last two measurements.
  let previous = { rate: 1, ms: plain };
  let slowed = await throttled();
  const trail = [slowed];
  for (let step = 0; step < 4 && Math.abs(slowed - REFERENCE_MS) > REFERENCE_MS / 20; step += 1) {
    const slope = (slowed - previous.ms) / (rate - previous.rate);
    previous = { rate, ms: slowed };
    rate = clamp(
      Number.isFinite(slope) && slope > 0
        ? rate + (REFERENCE_MS - slowed) / slope
        : (rate * REFERENCE_MS) / slowed,
    );
    if (rate === previous.rate) break;
    slowed = await throttled();
    trail.push(slowed);
  }
  test.info().annotations.push({
    type: 'cpu',
    description: `throttled ${rate.toFixed(1)}x (work took ${plain.toFixed(1)} ms unthrottled, then ${trail.map((ms) => ms.toFixed(1)).join(', ')} ms)`,
  });
  return rate;
}

/**
 * Wait until nothing short moves any more (a view, a stage or a row still on its way into
 * place): on a busy machine that takes longer than the two frames of `settle`. Loops and long
 * timelines (a toast's life) do not count.
 */
export async function motionSettled(page: Page): Promise<void> {
  await page.waitForFunction(() =>
    document.getAnimations().every((animation) => {
      if (animation.playState !== 'running') return true;
      const end = Number(animation.effect?.getComputedTiming().endTime ?? 0);
      return !Number.isFinite(end) || end > 1000;
    }),
  );
}

export async function settle(page: Page): Promise<void> {
  await viewsSettled(page);
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  });
}

/**
 * Compare with the baseline and, if SHOTS_DIR is set, also save a copy for humans.
 * Console output during the capture is Playwright's own and is not counted.
 */
export async function expectShot(
  page: Page,
  name: string,
  options: { maxDiffPixelRatio?: number; timeout?: number } = {},
): Promise<void> {
  capturing.add(page);
  try {
    await expect(page).toHaveScreenshot(`${name}.png`, options);
    const dir = process.env.SHOTS_DIR;
    if (dir) {
      const engine = test.info().project.name;
      await page.screenshot({ path: `${dir}/${name}-${engine}.png`, animations: 'disabled' });
    }
    await settle(page);
  } finally {
    capturing.delete(page);
  }
}
