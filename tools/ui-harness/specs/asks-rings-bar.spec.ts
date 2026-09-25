// Two asks of the user about the jobs list and the reader (2026-09-25): every score ring has
// the same solid track (the centre and the arc say the state), and the coral bar of the
// open row slides from row to row like the sidebar's pill.

import type { Page } from '@playwright/test';
import { expect, open, runFinished, settle, test } from './fixtures';

const WIN = '?platform=windows';
const SIZES = ['sm', 'md', 'lg'] as const;
const RINGS = [
  'full',
  'high',
  'mid',
  'low',
  'excluded',
  'unscorable',
  'provisional',
  'pending',
  'none',
] as const;

/** A colour token as the engine computes it. */
async function tokenColour(page: Page, name: string): Promise<string> {
  return page.evaluate((token) => {
    const probe = document.createElement('span');
    probe.style.setProperty('color', `var(${token})`);
    document.body.append(probe);
    const value = getComputedStyle(probe).color;
    probe.remove();
    return value;
  }, name);
}

async function windowInactive(page: Page, inactive: boolean): Promise<void> {
  await page.evaluate((value) => {
    document.documentElement.dataset['window'] = value ? 'inactive' : 'active';
  }, inactive);
}

/* ------------------------------------------------------------------- rings */

/** How a ring is drawn, apart from the colour of its value. */
async function look(page: Page, testid: string): Promise<Record<string, string>> {
  return page.getByTestId(testid).evaluate((ring) => {
    const style = (selector: string): CSSStyleDeclaration | null => {
      const node = ring.querySelector(selector);
      return node ? getComputedStyle(node) : null;
    };
    const track = style('.track');
    const value = style('.value');
    const disc = style('.disc');
    const centre = style('.center');
    return {
      track: `${track?.stroke} ${track?.strokeWidth} ${track?.strokeDasharray}`,
      value: `${value?.strokeWidth} ${value?.strokeDasharray} ${value?.strokeLinecap}`,
      disc: disc?.fill ?? '',
      centre: `${centre?.color} ${centre?.font}`,
    };
  });
}

/** The dash pattern of the circles a ring draws as its track. */
async function trackDashes(page: Page, testid: string): Promise<string[]> {
  return page.getByTestId(testid).evaluate((ring) => {
    const circles = [ring.querySelector('.track')!];
    // The list ring that waits draws its track on the layer that breathes.
    if (ring.matches('.sm.pending')) circles.push(ring.querySelector('.wait .arc')!);
    return circles.map((circle) => getComputedStyle(circle).strokeDasharray);
  });
}

test('every ring has one solid track in every state and size; the centre and the arc say the state', async ({
  page,
}) => {
  await open(page, `?gallery`);
  await page.getByTestId('gallery-rings').scrollIntoViewIfNeeded();
  for (const size of SIZES) {
    for (const id of RINGS) {
      const dashes = await trackDashes(page, `ring-${id}-${size}`);
      expect(dashes.length, `${id} ${size}`).toBeGreaterThan(0);
      for (const dash of dashes) expect(dash, `${id} ${size}`).toBe('none');
    }
    // Scored and provisional look exactly alike (the band and the digits too); only their
    // names differ.
    expect(await look(page, `ring-provisional-${size}`)).toEqual(
      await look(page, `ring-mid-${size}`),
    );
    await expect(page.getByTestId(`ring-provisional-${size}`)).toHaveAttribute(
      'aria-label',
      /vorläufig/,
    );
    // Not scored yet: the plain track and an empty centre, no arc.
    const none = page.getByTestId(`ring-none-${size}`);
    await expect(none).toHaveText('');
    await expect(none.locator('.value, .wait')).toHaveCount(0);
    expect((await look(page, `ring-none-${size}`)).track).toBe(
      (await look(page, `ring-unscorable-${size}`)).track,
    );
  }
  // Waiting: in the reader a quarter arc turns on the track, in the list the track breathes.
  const pending = (size: string) =>
    page.getByTestId(`ring-pending-${size}`).evaluate((ring) => {
      const layer = ring.querySelector('.wait')!;
      const arc = getComputedStyle(layer.querySelector('.arc')!);
      return { name: getComputedStyle(layer).animationName, dashes: arc.strokeDasharray };
    });
  expect(await pending('md')).toEqual({
    name: 'spin',
    dashes: expect.stringMatching(/^25(px)?,? 75(px)?$/),
  });
  expect(await pending('sm')).toEqual({ name: 'breathe', dashes: 'none' });
  // On the wash of a selected row every track is warm and solid; grey with the window.
  const tracks = () =>
    page
      .getByTestId('ring-wash')
      .locator('.ring:not(.excluded, .pending) .track')
      .evaluateAll((circles) =>
        circles.map(
          (circle) =>
            `${getComputedStyle(circle).stroke} ${getComputedStyle(circle).strokeDasharray}`,
        ),
      );
  const warm = await tokenColour(page, '--ring-track-selected');
  expect(new Set(await tracks())).toEqual(new Set([`${warm} none`]));
  await windowInactive(page, true);
  const grey = await tokenColour(page, '--ring-track-inactive');
  await expect.poll(async () => new Set(await tracks())).toEqual(new Set([`${grey} none`]));
});

test('rings in the list and the reader: solid when selected, inactive or waiting', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await settle(page);
  const list = page.getByTestId('job-list');
  // No ring of the list draws a dashed track.
  const dashes = await list
    .locator('.ring .track')
    .evaluateAll((circles) => circles.map((circle) => getComputedStyle(circle).strokeDasharray));
  expect(dashes.length).toBeGreaterThan(10);
  expect(new Set(dashes)).toEqual(new Set(['none']));
  // A score from a teaser rings like any other; the badge says it.
  const teaser = list.locator('.job', { has: page.getByTestId('job-row-freelance-900411') });
  const scored = list.locator('.job', { has: page.getByTestId('job-row-freelancermap-2802') });
  await expect(teaser.locator('.ring')).toHaveClass(/provisional/);
  await expect(teaser).toContainText('Nur Anriss');
  const lookOf = (row: typeof teaser) =>
    row.locator('.ring').evaluate((ring) => {
      const track = getComputedStyle(ring.querySelector('.track')!);
      const disc = getComputedStyle(ring.querySelector('.disc')!);
      return `${track.stroke} ${track.strokeDasharray} ${disc.fill}`;
    });
  expect(await lookOf(teaser)).toBe(await lookOf(scored));
  // Not scored yet: the track with an empty centre.
  const none = list.locator('.job', { has: page.getByTestId('job-row-linkedin-4100200302') });
  await expect(none.locator('.ring')).toHaveClass(/none/);
  await expect(none.locator('.ring .center')).toHaveText('');
  // Selected: the track turns warm; while the window is in the back, grey like the wash.
  await page.getByTestId('job-row-freelance-900411').click();
  const trackOf = () =>
    teaser.locator('.ring .track').evaluate((circle) => getComputedStyle(circle).stroke);
  await expect.poll(trackOf).toBe(await tokenColour(page, '--ring-track-selected'));
  await windowInactive(page, true);
  await expect.poll(trackOf).toBe(await tokenColour(page, '--ring-track-inactive'));
  await windowInactive(page, false);
  // The reader's ring of that job: provisional, drawn like any score.
  const reader = page.getByTestId('reader-ring');
  await expect(reader).toHaveClass(/provisional/);
  await expect(reader.locator('.track')).toHaveCSS('stroke-dasharray', 'none');
});

test('a list ring that waits breathes on its solid track', async ({ page }) => {
  await open(page, `${WIN}&scenario=running`);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await settle(page);
  const ring = page
    .getByTestId('job-list')
    .locator('.job', { has: page.getByTestId('job-row-linkedin-4100200302') })
    .locator('.ring');
  await expect(ring).toHaveClass(/pending/);
  const wait = await ring.evaluate((node) => {
    const layer = node.querySelector('.wait')!;
    return {
      name: getComputedStyle(layer).animationName,
      dashes: getComputedStyle(layer.querySelector('.arc')!).strokeDasharray,
      track: getComputedStyle(node.querySelector('.track')!).stroke,
    };
  });
  expect(wait).toEqual({ name: 'breathe', dashes: 'none', track: 'none' });
});

/* --------------------------------------------------------------------- bar */

interface Sample {
  t: number;
  top: number;
  height: number;
  left: number;
  opacity: number;
  /** The bar's animations: the duration and easing of each (CSS transitions too). */
  moves: string[];
  /** The open job's row as it is drawn (its glide included). */
  row: { top: number; height: number } | null;
  key: string | null;
}

declare global {
  interface Window {
    __barSamples: Sample[];
    /** The sampling under way (a stopped one ends at its next frame). */
    __barSampling: number;
  }
}

/**
 * Records the bar and the open row once per frame, as the frame is drawn, until
 * `stopSampling`. Measured in the frame's last resize-observer callback (a new observer each
 * frame comes last), after the app's own work of that frame: a measurement between frames
 * would also see rows a task changed that no frame has drawn yet.
 */
async function startSampling(page: Page): Promise<void> {
  await page.evaluate(() => {
    window.__barSamples = [];
    const session = (window.__barSampling ?? 0) + 1;
    window.__barSampling = session;
    const t0 = performance.now();
    const probe = document.createElement('div');
    probe.setAttribute('aria-hidden', 'true');
    probe.style.setProperty('position', 'fixed');
    probe.style.setProperty('width', '1px');
    probe.style.setProperty('height', '1px');
    document.body.append(probe);
    const sample = (): void => {
      const bar = document.querySelector<HTMLElement>('[data-testid="row-bar"]');
      const list = bar?.parentElement;
      if (!bar || !list) return;
      const frame = list.getBoundingClientRect();
      const box = bar.getBoundingClientRect();
      const open = list.querySelector<HTMLElement>('.item[data-open]');
      const row = open?.getBoundingClientRect();
      window.__barSamples.push({
        t: performance.now() - t0,
        top: box.top - frame.top,
        height: box.height,
        left: box.left - frame.left,
        opacity: Number(getComputedStyle(bar).opacity),
        moves: bar.getAnimations().map((animation) => {
          const timing = (animation.effect as KeyframeEffect).getComputedTiming();
          return `${timing.duration} ${timing.easing}`;
        }),
        row: row ? { top: row.top - frame.top, height: row.height } : null,
        key: open?.dataset['key'] ?? null,
      });
    };
    const next = (): void => {
      if (window.__barSampling !== session) {
        probe.remove();
        return;
      }
      const observer = new ResizeObserver(() => {
        observer.disconnect();
        if (window.__barSampling === session) sample();
        requestAnimationFrame(next);
      });
      observer.observe(probe);
    };
    requestAnimationFrame(next);
  });
}

async function stopSampling(page: Page): Promise<Sample[]> {
  return page.evaluate(() => {
    window.__barSampling += 1;
    return window.__barSamples;
  });
}

const INSET = 12;

/** The bar stands on the open row: its top and bottom inset by the row's padding. */
function onRow(sample: Sample): boolean {
  if (sample.row === null) return false;
  return (
    Math.abs(sample.top - (sample.row.top + INSET)) < 1 &&
    Math.abs(sample.height - (sample.row.height - 2 * INSET)) < 1
  );
}

async function allJobs(page: Page, query = WIN): Promise<void> {
  await open(page, query);
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  await settle(page);
}

const rowOf = (page: Page, key: string) =>
  page.getByTestId('job-list').getByTestId(`job-row-${key}`);

/** The bar and the open row in the next drawn frame. */
async function nextFrame(page: Page): Promise<Sample> {
  await startSampling(page);
  await expect.poll(() => page.evaluate(() => window.__barSamples.length)).toBeGreaterThan(0);
  return (await stopSampling(page)).at(-1)!;
}

/** The bar once it has come to rest: shown (grey on an excluded row), nothing moving, on
 *  the open row. */
async function resting(page: Page): Promise<Sample> {
  await expect
    .poll(() =>
      page
        .getByTestId('row-bar')
        .evaluate(
          (bar) => bar.getAnimations().length === 0 && getComputedStyle(bar).opacity !== '0',
        ),
    )
    .toBe(true);
  const last = await nextFrame(page);
  expect(last.moves).toEqual([]);
  expect(onRow(last), JSON.stringify(last)).toBe(true);
  return last;
}

/** The easing token as the engine writes it. */
async function easing(page: Page, name: string): Promise<string> {
  return page.evaluate((token) => {
    const probe = document.createElement('span');
    probe.style.setProperty('transition-timing-function', `var(${token})`);
    document.body.append(probe);
    const value = getComputedStyle(probe).transitionTimingFunction;
    probe.remove();
    return value;
  }, name);
}

test('the bar slides to the clicked row like the sidebar pill: 180 ms, emphasized, one height on its way', async ({
  page,
}) => {
  await allJobs(page);
  // Two rows of the one height (86 px).
  await rowOf(page, 'freelancermap-2802').click();
  const start = await resting(page);
  expect(start.height).toBe(86 - 2 * INSET);
  await startSampling(page);
  await rowOf(page, 'freelancermap-2803').click();
  await page.waitForTimeout(400);
  const samples = await stopSampling(page);
  const end = samples.at(-1)!;
  expect(onRow(end)).toBe(true);
  expect(end.height).toBe(86 - 2 * INSET);
  // One move, the pill's: 180 ms with the emphasized easing (the nav pill's transition).
  const emphasized = await easing(page, '--ease-emphasized');
  const moves = new Set(samples.flatMap((sample) => sample.moves));
  expect([...moves]).toEqual([`180 ${emphasized}`]);
  const nav = await page.evaluate(() => {
    const style = getComputedStyle(document.querySelector('nav .indicator')!);
    return `${style.transitionDuration} ${style.transitionTimingFunction}`;
  });
  expect(nav).toBe(`0.18s ${emphasized}`);
  // On its way: between the rows, never back, never past, and with the one row height.
  const between = samples.filter(
    (sample) => sample.top > start.top + 1 && sample.top < end.top - 1,
  );
  expect(between.length, JSON.stringify(samples)).toBeGreaterThan(0);
  for (const sample of between) {
    expect(Math.abs(sample.height - start.height)).toBeLessThan(0.5);
  }
  for (let at = 1; at < samples.length; at += 1) {
    expect(samples[at]!.top).toBeGreaterThanOrEqual(samples[at - 1]!.top - 0.5);
    expect(samples[at]!.top).toBeLessThanOrEqual(end.top + 0.5);
  }
  // Back up to a row of 86 px: the height follows.
  await rowOf(page, 'freelancermap-2801').click();
  const back = await resting(page);
  expect(back.height).toBe(86 - 2 * INSET);
  expect(back.top).toBe(INSET);
});

test('the arrow keys slide the bar; Home and End jump far and place it', async ({ page }) => {
  await allJobs(page);
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press('ArrowDown');
  await expect(rowOf(page, 'freelancermap-2801')).toHaveAttribute('aria-current', 'true');
  await resting(page);
  await startSampling(page);
  await page.keyboard.press('ArrowDown');
  await expect(rowOf(page, 'linkedin-4100200301')).toHaveAttribute('aria-current', 'true');
  await page.waitForTimeout(350);
  let samples = await stopSampling(page);
  expect(samples.some((sample) => sample.moves.some((move) => move.startsWith('180 ')))).toBe(true);
  expect(onRow(samples.at(-1)!)).toBe(true);
  // End: the last job, further away than the list is high: the bar is simply there.
  await startSampling(page);
  await page.keyboard.press('End');
  await expect(rowOf(page, 'linkedin-4100200305')).toHaveAttribute('aria-current', 'true');
  await page.waitForTimeout(300);
  samples = await stopSampling(page);
  expect(samples.flatMap((sample) => sample.moves)).not.toContainEqual(
    expect.stringMatching(/^180 /),
  );
  const last = await resting(page);
  expect(last.key).toBe('linkedin:4100200305');
  // The last job is an excluded one: the bar is as grey as its row.
  expect(last.opacity).toBe(0.6);
  // And Home again, to the first row.
  await startSampling(page);
  await page.keyboard.press('Home');
  await expect(rowOf(page, 'freelancermap-2801')).toHaveAttribute('aria-current', 'true');
  await page.waitForTimeout(300);
  samples = await stopSampling(page);
  expect(samples.flatMap((sample) => sample.moves)).not.toContainEqual(
    expect.stringMatching(/^180 /),
  );
  expect((await resting(page)).top).toBe(INSET);
});

test('no motion when the list is built: start, the view coming back, another filter, a search', async ({
  page,
}) => {
  await allJobs(page);
  await rowOf(page, 'freelancermap-2803').click();
  await resting(page);
  const still = (samples: Sample[]) => {
    const shown = samples.filter((sample) => sample.opacity > 0);
    expect(shown.length).toBeGreaterThan(0);
    for (const sample of shown) {
      expect(sample.moves, JSON.stringify(sample)).toEqual([]);
      expect(sample.opacity).toBe(1);
      expect(onRow(sample), JSON.stringify(sample)).toBe(true);
    }
  };
  // Another view and back: the list is built anew, the bar simply stands there.
  await page.getByTestId('nav-profile').click();
  await settle(page);
  await startSampling(page);
  await page.getByTestId('nav-jobs').click();
  await expect(rowOf(page, 'freelancermap-2803')).toHaveAttribute('aria-current', 'true');
  await page.waitForTimeout(300);
  still(await stopSampling(page));
  // A search that keeps the open job: nothing moves.
  await startSampling(page);
  await page.getByTestId('search').fill('Leitung');
  await expect(rowOf(page, 'freelancermap-2801')).toHaveCount(0);
  await page.waitForTimeout(300);
  still(await stopSampling(page));
  await page.getByTestId('search').fill('');
  await expect(rowOf(page, 'freelancermap-2801')).toHaveCount(1);
  // Another filter: the bar never slides from where it stood; it stays on its row (which
  // may glide to its new place, the bar with it).
  await rowOf(page, 'freelancermap-2801').click();
  await resting(page);
  await startSampling(page);
  await page
    .getByTestId('facet')
    .getByRole('radio', { name: /Favoriten/ })
    .click();
  await expect(rowOf(page, 'freelancermap-2802')).toHaveCount(0);
  await page.waitForTimeout(300);
  const filtered = await stopSampling(page);
  for (const sample of filtered.filter((s) => s.opacity > 0 && s.row !== null)) {
    expect(sample.moves.filter((move) => move.startsWith('180 '))).toEqual([]);
    expect(onRow(sample), JSON.stringify(sample)).toBe(true);
  }
});

test('under reduced motion the bar never moves, it is simply at the next row', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await allJobs(page);
  await rowOf(page, 'freelancermap-2802').click();
  await resting(page);
  await startSampling(page);
  await rowOf(page, 'freelancermap-2803').click();
  await page.keyboard.press('ArrowDown');
  await page.waitForTimeout(300);
  const samples = await stopSampling(page);
  for (const sample of samples) {
    expect(sample.moves, JSON.stringify(sample)).toEqual([]);
    expect(onRow(sample) || sample.key === null, JSON.stringify(sample)).toBe(true);
  }
});

test('several chosen rows each show their own still bar; back to one the bar is simply there', async ({
  page,
}) => {
  await allJobs(page);
  const own = (key: string) =>
    rowOf(page, key).evaluate((row) => getComputedStyle(row, '::before').opacity);
  await rowOf(page, 'freelancermap-2801').click();
  await resting(page);
  await startSampling(page);
  await rowOf(page, 'freelancermap-2802').click({ modifiers: ['Control'] });
  await rowOf(page, 'freelancermap-2803').click({ modifiers: ['Control'] });
  await expect(page.getByTestId('selection-bar')).toContainText('3 ausgewählt');
  // The chosen rows mark themselves; the open job's row keeps the list's bar, which stays.
  await expect.poll(() => own('freelancermap-2802')).toBe('1');
  await expect.poll(() => own('freelancermap-2803')).toBe('1');
  expect(await own('freelancermap-2801')).toBe('0');
  await page.waitForTimeout(200);
  let samples = await stopSampling(page);
  expect(samples.flatMap((sample) => sample.moves)).toEqual([]);
  expect(samples.every((sample) => sample.key === 'freelancermap:2801' && onRow(sample))).toBe(
    true,
  );
  // The open job taken out of the choice: its bar goes with its highlight.
  await rowOf(page, 'freelancermap-2801').click({ modifiers: ['Control'] });
  await expect(page.getByTestId('selection-bar')).toContainText('2 ausgewählt');
  await expect
    .poll(() => page.getByTestId('row-bar').evaluate((bar) => getComputedStyle(bar).opacity))
    .toBe('0');
  // One left: that job opens, the list's bar simply stands on it (no slide, no fade), and
  // its own bar is gone at once.
  await startSampling(page);
  await rowOf(page, 'freelancermap-2803').click({ modifiers: ['Control'] });
  await expect(page.getByTestId('selection-bar')).toHaveCount(0);
  await expect(rowOf(page, 'freelancermap-2802')).toHaveAttribute('aria-current', 'true');
  await page.waitForTimeout(300);
  samples = await stopSampling(page);
  const shown = samples.filter((sample) => sample.opacity > 0);
  expect(shown.length).toBeGreaterThan(0);
  for (const sample of shown) {
    expect(sample.moves, JSON.stringify(sample)).toEqual([]);
    expect(sample.opacity).toBe(1);
    expect(sample.key).toBe('freelancermap:2802');
    expect(onRow(sample)).toBe(true);
  }
  expect(await own('freelancermap-2802')).toBe('0');
  // Esc after a choice with the open job in it: the bar stays where it is.
  await rowOf(page, 'freelancermap-2803').click({ modifiers: ['Control'] });
  await expect(page.getByTestId('selection-bar')).toContainText('2 ausgewählt');
  await startSampling(page);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('selection-bar')).toHaveCount(0);
  await page.waitForTimeout(200);
  samples = await stopSampling(page);
  expect(samples.flatMap((sample) => sample.moves)).toEqual([]);
  expect(samples.every((sample) => sample.key === 'freelancermap:2802' && onRow(sample))).toBe(
    true,
  );
});

test('the bar scrolls with the list and stays on its row while the rows grow window by window', async ({
  page,
}) => {
  await allJobs(page, `${WIN}&scenario=many`);
  await page.getByTestId('job-rows').locator('[data-testid^="job-row-"]').nth(3).click();
  await resting(page);
  const built = () => page.getByTestId('job-list').locator('.item[data-key]').count();
  const before = await built();
  await startSampling(page);
  // Small steps, a frame each, then the wheel; far enough that further rows are built.
  await page.getByTestId('list-scroll').evaluate(async (scroller) => {
    for (let step = 0; step < 40; step += 1) {
      scroller.scrollTop += 37 + step * 4;
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
  });
  await page.mouse.move(200, 500);
  for (let turn = 0; turn < 6; turn += 1) await page.mouse.wheel(0, 1200);
  await expect.poll(built).toBeGreaterThan(before);
  await page.getByTestId('list-scroll').evaluate((scroller) => (scroller.scrollTop = 0));
  await page.waitForTimeout(300);
  const samples = await stopSampling(page);
  expect(samples.length).toBeGreaterThan(40);
  for (const sample of samples) expect(onRow(sample), JSON.stringify(sample)).toBe(true);
  await resting(page);
});

test('archive, trash and undo: the bar ends on the job that opens, never at a stale place', async ({
  page,
}) => {
  await allJobs(page);
  // The job above another row: archived, the next row moves up into its place.
  await rowOf(page, 'freelancermap-2802').click();
  const slot = await resting(page);
  await startSampling(page);
  await page.getByTestId('reader-archive').click();
  await expect(rowOf(page, 'freelancermap-2803')).toHaveAttribute('aria-current', 'true');
  await page.waitForTimeout(400);
  let samples = await stopSampling(page);
  let end = samples.at(-1)!;
  expect(end.key).toBe('freelancermap:2803');
  expect(onRow(end)).toBe(true);
  expect(end.top).toBe(slot.top);
  expect(end.height).toBe(86 - 2 * INSET);
  // It never left the place: the rows moved up under it, and it only grew.
  for (const sample of samples) {
    expect(sample.top, JSON.stringify(sample)).toBe(slot.top);
    expect(sample.height).toBeGreaterThanOrEqual(slot.height);
    expect(sample.opacity).toBe(1);
  }
  // Undo: the job comes back above and opens again; the bar is back on it.
  await startSampling(page);
  await page.getByTestId('toast').last().getByTestId('toast-action').click();
  await expect(rowOf(page, 'freelancermap-2802')).toHaveAttribute('aria-current', 'true');
  await page.waitForTimeout(400);
  samples = await stopSampling(page);
  end = samples.at(-1)!;
  expect(end.key).toBe('freelancermap:2802');
  expect(onRow(end)).toBe(true);
  expect(end.top).toBe(slot.top);
  expect(end.height).toBe(slot.height);
  for (const sample of samples) expect(sample.top, JSON.stringify(sample)).toBe(slot.top);
  // The trash: the same with the next job.
  await page.waitForTimeout(600);
  await page.getByTestId('reader-trash').click();
  await expect(rowOf(page, 'freelancermap-2803')).toHaveAttribute('aria-current', 'true');
  const after = await resting(page);
  expect(after.top).toBe(slot.top);
  expect(after.height).toBe(86 - 2 * INSET);
});

test('the bar steps inside the keyboard focus ring and greys with the inactive window', async ({
  page,
}) => {
  await allJobs(page);
  await rowOf(page, 'freelancermap-2801').click();
  const clicked = await resting(page);
  expect(clicked.left).toBe(0);
  // From the keyboard the row shows its ring: the bar steps inside it.
  await page.keyboard.press('ArrowDown');
  await expect(rowOf(page, 'linkedin-4100200301')).toBeFocused();
  await expect.poll(async () => (await nextFrame(page)).left).toBe(2);
  // While the window is in the back the bar greys like a row's own bar.
  const barColour = () =>
    page.getByTestId('row-bar').evaluate((bar) => getComputedStyle(bar).backgroundColor);
  const active = await barColour();
  await windowInactive(page, true);
  const subtle = await tokenColour(page, '--text-subtle');
  await expect.poll(barColour).toBe(subtle);
  expect(subtle).not.toBe(active);
});

test('the bar keeps its row when the window narrows and in one column', async ({ page }) => {
  await allJobs(page);
  await rowOf(page, 'linkedin-4100200301').click();
  expect((await resting(page)).height).toBe(86 - 2 * INSET);
  // A narrower window: the title stays one line, the row and its bar keep their height.
  await page.setViewportSize({ width: 960, height: 900 });
  await expect
    .poll(() =>
      rowOf(page, 'linkedin-4100200301').evaluate((row) => (row as HTMLElement).offsetHeight),
    )
    .toBe(86);
  expect((await resting(page)).height).toBe(86 - 2 * INSET);
  // One column shows the reader; back in two columns the bar is simply on its row.
  await page.setViewportSize({ width: 780, height: 900 });
  await expect(page.getByTestId('reader')).toBeVisible();
  await startSampling(page);
  await page.setViewportSize({ width: 1360, height: 900 });
  await expect(rowOf(page, 'linkedin-4100200301')).toBeVisible();
  await page.waitForTimeout(300);
  const samples = await stopSampling(page);
  for (const sample of samples.filter((s) => s.opacity > 0)) {
    expect(sample.moves.filter((move) => move.startsWith('180 '))).toEqual([]);
  }
  await resting(page);
  // In one column rows are only chosen: each marks itself, the list's bar stays hidden.
  await page.getByTestId('reader-close').click();
  await page.setViewportSize({ width: 780, height: 900 });
  await rowOf(page, 'linkedin-4100200301').click({ modifiers: ['Control'] });
  await rowOf(page, 'freelancermap-2803').click({ modifiers: ['Control'] });
  await expect(page.getByTestId('selection-bar')).toContainText('2 ausgewählt');
  await expect
    .poll(() =>
      rowOf(page, 'freelancermap-2803').evaluate(
        (row) => getComputedStyle(row, '::before').opacity,
      ),
    )
    .toBe('1');
  await expect(page.getByTestId('row-bar')).toHaveCSS('opacity', '0');
});

test('a run brings new jobs above and re-sorts, the order changes: the bar stays on its row', async ({
  page,
}) => {
  await allJobs(page, `${WIN}&tick=30`);
  await rowOf(page, 'freelancermap-2804').click();
  await resting(page);
  await startSampling(page);
  // New jobs land above the open one while the run goes (it moves down at once), the list
  // re-sorts at its end (the row glides, the bar with it).
  await page.getByTestId('fetch').click();
  await runFinished(page);
  await page.waitForTimeout(300);
  // Another order: the row glides to its new place, the bar with it.
  await page.getByTestId('sort').click();
  await page.evaluate(() => window.__harness.pick(1));
  await page.waitForTimeout(400);
  const samples = await stopSampling(page);
  const drawn = samples.filter((sample) => sample.opacity > 0 && sample.row !== null);
  expect(drawn.length).toBeGreaterThan(20);
  for (const sample of drawn) {
    expect(sample.moves.filter((move) => move.startsWith('180 '))).toEqual([]);
    expect(onRow(sample), JSON.stringify(sample)).toBe(true);
  }
  await resting(page);
});
