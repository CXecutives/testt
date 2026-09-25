// Reduced motion is enforced by lib/motion/motion.ts, not by the CSS media query.

import { expect, open, settle, test } from './fixtures';

test('full motion by default', async ({ page }) => {
  await open(page, '?platform=windows');
  await expect(page.locator('html')).toHaveAttribute('data-motion', 'full');
});

test('reduced motion: no movement, instant tokens, the new view is in place at once', async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, '?platform=windows');
  await expect(page.locator('html')).toHaveAttribute('data-motion', 'reduce');
  const tokens = await page.evaluate(() => {
    const style = getComputedStyle(document.documentElement);
    return ['--dur-base', '--move-lg', '--dur-reveal', '--loop-state'].map((name) =>
      style.getPropertyValue(name).trim(),
    );
  });
  expect(tokens.slice(0, 3).map((value) => parseFloat(value))).toEqual([0, 0, 0]);
  expect(tokens[3]).toBe('paused');

  await page.getByTestId('nav-settings').click();
  const view = page.getByTestId('view-settings');
  await expect(view).toBeVisible();
  // A cross-fade may remain; a rise may not: the view never starts below its place.
  const transform = await view.evaluate((node) => getComputedStyle(node).transform);
  expect(['none', 'matrix(1, 0, 0, 1, 0, 0)']).toContain(transform);
});

test('switching reduced motion at runtime is followed', async ({ page }) => {
  await open(page, '?gallery');
  await expect(page.getByTestId('motion-state')).toHaveText('Volle Bewegung ist an.');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(page.getByTestId('motion-state')).toHaveText('Reduzierte Bewegung ist an.');
  await page.getByTestId('motion-play').click();
  await expect(page.getByTestId('motion-count')).toContainText('87');
});

test('nothing animates at start, and a view that comes back does not replay', async ({ page }) => {
  await open(page, '?platform=windows');
  // What runs (by element and animation), so a failure names the culprit.
  const running = (): Promise<string[]> =>
    page.evaluate(() =>
      document
        .getAnimations()
        .filter((a) => a.playState === 'running')
        .map((a) => {
          const target = (a.effect as KeyframeEffect).target;
          const name = a instanceof CSSAnimation ? a.animationName : a.constructor.name;
          return `${target?.className ?? ''} ${name}`;
        }),
    );
  expect(await running()).toEqual([]);
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('nav-jobs').click();
  await page.waitForTimeout(250);
  // Nothing in the view that came back enters, rolls or slides.
  const inView = await page.evaluate(() => {
    const view = document.querySelector('[data-testid="view-jobs"]')!;
    return document
      .getAnimations()
      .filter((a) => a.playState === 'running')
      .map((a) => (a.effect as KeyframeEffect).target)
      .filter((target) => target !== null && view.contains(target))
      .map((target) => target!.className);
  });
  expect(inView).toEqual([]);
});

test('the reader ring fills from empty the first time a job opens, once', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.locator('[data-testid^="job-row-"]').first().click();
  const ring = page.getByTestId('reader-ring');
  await expect(ring).toBeVisible();
  // One Web Animation on the arc, starting from empty (stroke-dashoffset 100).
  const fill = await ring.evaluate((node) =>
    node
      .querySelector('.value')!
      .getAnimations()
      .map((a) => String((a.effect as KeyframeEffect).getKeyframes()[0]?.strokeDashoffset)),
  );
  expect(fill).toEqual(['100']);
  // It is dropped when done.
  await page.waitForTimeout(500);
  const after = await ring.evaluate((node) => node.querySelector('.value')!.getAnimations().length);
  expect(after).toBe(0);
});

test('a count rolls when it changes on screen, not when it first shows', async ({ page }) => {
  await open(page, '?gallery');
  const count = page.getByTestId('count-soft');
  await count.scrollIntoViewIfNeeded();
  expect(await count.evaluate((node) => node.getAnimations({ subtree: true }).length)).toBe(0);
  // Click and look in the same frame (the roll lasts 150 ms).
  const rolling = await page.evaluate(async () => {
    document.querySelector<HTMLElement>('[data-testid="count-more"]')!.click();
    await new Promise((resolve) => requestAnimationFrame(resolve));
    return document
      .querySelector('[data-testid="count-soft"]')!
      .getAnimations({ subtree: true })
      .filter((a) => !(a instanceof CSSTransition)).length;
  });
  expect(rolling).toBeGreaterThan(0);
  await expect(count).toHaveText('13');
});

test('the pin star pops once when pinned, never when unpinned', async ({ page }) => {
  await open(page, '?gallery');
  const pin = page.getByTestId('motion-pin');
  await pin.scrollIntoViewIfNeeded();
  // Pops are Web Animations on the glyph (hover nudges are CSS transitions).
  const clickAndCount = (): Promise<number> =>
    page.evaluate(() => {
      const button = document.querySelector<HTMLElement>('[data-testid="motion-pin"]')!;
      button.click();
      return button
        .querySelector('.glyph')!
        .getAnimations()
        .filter((a) => !(a instanceof CSSTransition)).length;
    });
  expect(await clickAndCount()).toBe(1);
  await expect(pin).toHaveAttribute('aria-pressed', 'true');
  await page.waitForTimeout(400);
  expect(await clickAndCount()).toBe(0);
  await expect(pin).toHaveAttribute('aria-pressed', 'false');
});

test('under reduced motion nothing scales, pops or shakes', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, '?gallery');
  const tokens = await page.evaluate(() => {
    const style = getComputedStyle(document.documentElement);
    return ['--scale-press', '--scale-pop', '--scale-nudge', '--move-xs', '--dur-hover'].map(
      (name) => parseFloat(style.getPropertyValue(name)),
    );
  });
  expect(tokens).toEqual([1, 1, 1, 0, 0]);
  const pin = page.getByTestId('motion-pin');
  await pin.scrollIntoViewIfNeeded();
  await pin.click();
  await page.getByTestId('motion-shake').click();
  const scripted = await page.evaluate(
    () => document.getAnimations().filter((a) => !(a instanceof CSSAnimation)).length,
  );
  expect(scripted).toBe(0);
});

test('hover rests while a list scrolls', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  // The window mounts a few rows per frame: scroll once the list can.
  await expect
    .poll(() =>
      page.getByTestId('list-scroll').evaluate((node) => node.scrollHeight - node.clientHeight),
    )
    .toBeGreaterThan(200);
  // The pointer rests on a row (once the rows stand still: the switch glides them).
  await settle(page);
  const row = page.getByTestId('job-list').locator('[data-testid^="job-row-"]').first();
  await row.hover();
  const still = (): Promise<string[]> =>
    page.evaluate(() =>
      [...document.querySelectorAll<HTMLElement>('[data-still]')].map((node) => node.className),
    );
  expect(await still()).toEqual([]);
  // Scroll and look right after the scroll event: the row under the pointer rests (its hover
  // waits for `:not([data-still])`), the row button and the row around it; nothing else in
  // the list changes. The mark lasts --scroll-idle.
  const during = await page.getByTestId('list-scroll').evaluate(
    (node) =>
      new Promise<{ rows: (string | undefined)[]; hovered: boolean }>((resolve) => {
        node.addEventListener(
          'scroll',
          () => {
            const marked = [...document.querySelectorAll<HTMLElement>('[data-still]')];
            resolve({
              rows: marked.map(
                (el) =>
                  (el.querySelector<HTMLElement>('[data-testid^="job-row-"]') ?? el).dataset.testid,
              ),
              hovered: marked.every((el) => el.matches(':hover')),
            });
          },
          { once: true },
        );
        node.scrollBy(0, 200);
      }),
  );
  expect(during.rows).toHaveLength(2);
  expect(during.rows[0]).toMatch(/^job-row-/);
  expect(during.rows[1]).toBe(during.rows[0]);
  expect(during.hovered).toBe(true);
  await expect.poll(still, { timeout: 1000 }).toEqual([]);
});
