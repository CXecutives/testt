import { expect, expectShot, motionSettled, open, settle, test } from './fixtures';

test('the preview server sends the production CSP', async ({ page }) => {
  const response = await page.goto('/');
  const csp = response?.headers()['content-security-policy'] ?? '';
  expect(csp).toContain("script-src 'self'");
  expect(csp).toContain("style-src 'self'");
});

test('the shell renders sidebar and the jobs view', async ({ page }) => {
  await open(page, '?platform=windows');
  await expect(page.getByTestId('shell')).toBeVisible();
  await expect(page.getByTestId('sidebar')).toBeVisible();
  for (const item of ['nav-jobs', 'nav-profile', 'nav-settings']) {
    await expect(page.getByTestId(item)).toBeVisible();
  }
  await expect(page.getByTestId('nav-jobs')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('nav-jobs')).toHaveText('Jobs');
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await expect(page.getByTestId('fetch')).toHaveClass(/primary/);
});

// Every view switch, the Jobs view included, is the same quick cross-fade: the new view fades
// in on top while the old one fades out below it, so no frame shows an empty sheet. Recorded
// from the Web Animations Svelte starts (deterministic, no frame timing involved).
interface Fade {
  view: string;
  from: string;
  to: string;
  ms: number;
  order: string[];
}

test('every view switch is the same cross-fade, the new view on top', async ({ page }) => {
  await page.addInitScript(() => {
    const fades: Fade[] = [];
    (window as unknown as { __fades: Fade[] }).__fades = fades;
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (this: Element, keyframes, options) {
      const view = this instanceof HTMLElement ? (this.dataset.testid ?? '') : '';
      const ms = typeof options === 'number' ? options : Number(options?.duration ?? 0);
      if (view.startsWith('view-') && Array.isArray(keyframes) && ms > 0) {
        fades.push({
          view,
          from: String(keyframes[0]?.opacity),
          to: String(keyframes.at(-1)?.opacity),
          ms,
          order: [...(this.parentElement?.children ?? [])].map(
            (node) => (node as HTMLElement).dataset.testid ?? '',
          ),
        });
      }
      return animate.call(this, keyframes, options);
    };
  });
  // At start nothing animates: the first view is simply there.
  await open(page, '?platform=windows');
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { __fades: Fade[] }).__fades)).toEqual([]);
  const steps = [
    ['jobs', 'profile'],
    ['profile', 'settings'],
    ['settings', 'jobs'],
    ['jobs', 'settings'],
    ['settings', 'profile'],
    ['profile', 'jobs'],
  ];
  for (const [from, to] of steps) {
    await page.evaluate(() => (window as unknown as { __fades: Fade[] }).__fades.splice(0));
    await page.getByTestId(`nav-${to}`).click();
    await expect(page.locator('main.views > section')).toHaveCount(1);
    await expect(page.getByTestId(`view-${to}`)).toBeVisible();
    const fades = await page.evaluate(() => (window as unknown as { __fades: Fade[] }).__fades);
    const out = fades.find((fade) => fade.view === `view-${from}`);
    const into = fades.find((fade) => fade.view === `view-${to}`);
    expect(out, `${from} -> ${to}`).toMatchObject({ from: '1', to: '0', ms: 100 });
    expect(into, `${from} -> ${to}`).toMatchObject({
      from: '0',
      to: '1',
      ms: 100,
      order: [`view-${from}`, `view-${to}`],
    });
  }
});

// The OS window's focus (Tauri's window events) is one attribute on <html>; selections grey
// out against it as in Mail and Explorer.
test('the window focus state follows the OS window', async ({ page }) => {
  await open(page, '?platform=windows');
  const root = page.locator('html');
  await expect(root).toHaveAttribute('data-window', 'active');
  await page.evaluate(() => window.__harness.fire('tauri://blur', null));
  await expect(root).toHaveAttribute('data-window', 'inactive');
  await page.evaluate(() => window.__harness.fire('tauri://focus', null));
  await expect(root).toHaveAttribute('data-window', 'active');
});

test('the navigation switches the view', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.getByTestId('nav-profile').click();
  await expect(page.getByTestId('view-profile')).toBeVisible();
  await expect(page.getByTestId('view-jobs')).toHaveCount(0);
  await expect(page.getByTestId('nav-profile')).toHaveAttribute('aria-current', 'page');
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('view-settings')).toBeVisible();
  await expect(page.getByTestId('view-profile')).toHaveCount(0);
  await page.getByTestId('nav-jobs').click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
});

// The window frame is the native one of each OS (icon, title, caption buttons, system menu,
// snap layouts): the page draws none of it and has no drag region of its own.
for (const os of ['windows', 'macos']) {
  test(`${os}: no title bar in the page, the content starts at the top`, async ({ page }) => {
    await open(page, `?platform=${os}`);
    await expect(page.locator('html')).toHaveAttribute('data-platform', os);
    await expect(page.getByTestId('titlebar')).toHaveCount(0);
    expect((await page.getByTestId('sidebar').boundingBox())!.y).toBe(0);
  });
}

test('windows: no drag region; the first view sits on the line of the search field', async ({
  page,
}) => {
  await open(page, '?platform=windows');
  await expect(page.locator('[data-tauri-drag-region]')).toHaveCount(0);
  // The field's frame is the input's parent (the input sits inside its border).
  const nav = (await page.getByTestId('nav-jobs').boundingBox())!.y;
  const field = (await page.getByTestId('search').locator('xpath=..').boundingBox())!.y;
  expect(nav).toBe(field);
});

// macOS: the unified toolbar row of a Mac app. The title bar is transparent over the page
// (52 px, traffic lights at x 20, centred); the sidebar runs to the top with the lights in
// its first 52 px, the list's first row is centred on them, the sheet reaches the top edge,
// and the empty parts of the row move the window.
test('macos: the unified toolbar row', async ({ page, browserName }) => {
  await open(page, '?platform=macos');
  const ROW = 52;
  const lightsBand = page.getByTestId('sidebar').getByTestId('drag-band');
  expect(await lightsBand.boundingBox()).toMatchObject({ x: 0, y: 0, height: ROW });
  const search = (await page.getByTestId('search').locator('xpath=..').boundingBox())!;
  const fetch = (await page.getByTestId('fetch').boundingBox())!;
  expect(search.y + search.height / 2).toBe(ROW / 2);
  expect(fetch.y + fetch.height / 2).toBe(ROW / 2);
  // The row itself (outside the controls) drags; the reader side keeps a band of the row.
  await expect(page.getByTestId('list-header').locator('.top')).toHaveAttribute(
    'data-tauri-drag-region',
    '',
  );
  const readerBand = page.getByTestId('reader-pane').getByTestId('drag-band');
  expect(await readerBand.boundingBox()).toMatchObject({ y: 0, height: ROW });
  // The views start below the lights; the sheet has no top edge.
  expect((await page.getByTestId('nav-jobs').boundingBox())!.y).toBeGreaterThanOrEqual(ROW);
  const sheet = await page
    .locator('main.views')
    .evaluate((node) => [node.getBoundingClientRect().top, getComputedStyle(node).borderTopWidth]);
  expect(sheet).toEqual([0, '0px']);
  // Profil and Einstellungen keep the row free too.
  for (const view of ['profile', 'settings']) {
    await page.getByTestId(`nav-${view}`).click();
    const band = page.getByTestId(`view-${view}`).getByTestId('drag-band');
    // Once the cross-fade has settled.
    await expect.poll(async () => (await band.boundingBox())?.y).toBe(0);
    expect(await band.boundingBox()).toMatchObject({ height: ROW });
  }
  // A picture with the traffic lights drawn in where macOS puts them (for humans; WebKit
  // reports Playwright's screenshot styles as a CSP violation).
  if (process.env.SHOTS_DIR && browserName === 'chromium') {
    await page.getByTestId('nav-jobs').click();
    await page.evaluate(() => {
      const colours = ['#ff5f57', '#febc2e', '#28c840'];
      colours.forEach((colour, i) => {
        const dot = document.body.appendChild(document.createElement('div'));
        Object.assign(dot.style, {
          position: 'fixed',
          left: `${20 + i * 20}px`,
          top: '19px',
          width: '14px',
          height: '14px',
          borderRadius: '7px',
          background: colour,
          zIndex: '1000',
        });
      });
    });
    await page.screenshot({ path: `${process.env.SHOTS_DIR}/macos-toolbar-marked.png` });
  }
});

// macOS: every point of the 52 px toolbar row either moves the window (Tauri's drag script:
// a direct hit on an element with data-tauri-drag-region; a double click there zooms) or is
// a control. Hairlines (the borders between the columns) are the only exception.
const DRAG_PROBE = (): string[] => {
  const CONTROL =
    'a, button, input, select, textarea, label, summary, [contenteditable]:not([contenteditable="false"]), [tabindex]:not([tabindex="-1"]), [role="button"], [role="link"], [role="tab"], [role="switch"], [role="radio"], [role="checkbox"], [role="option"], [role="menuitem"]';
  const dead: string[] = [];
  const width = document.documentElement.clientWidth;
  for (const y of [1, 14, 26, 38, 51]) {
    for (let x = 1; x < width - 1; x += 5) {
      const hit = document.elementFromPoint(x, y);
      if (hit === null) {
        dead.push(`${x},${y} nothing`);
        continue;
      }
      if (hit.hasAttribute('data-tauri-drag-region')) continue;
      // A control, or the frame right around a field (it focuses the field); a point just
      // outside a rounded corner of either lands on the wrapper around it.
      const control = (node: Element): boolean =>
        node.closest(CONTROL) !== null ||
        node.querySelector(':scope > input, :scope > textarea') !== null;
      const corner = [...hit.querySelectorAll('*')].some((node) => {
        const b = node.getBoundingClientRect();
        return control(node) && x >= b.left && x < b.right && y >= b.top && y < b.bottom;
      });
      if (control(hit) || corner) continue;
      const box = hit.getBoundingClientRect();
      const style = getComputedStyle(hit);
      const hairline =
        x < box.left + parseFloat(style.borderLeftWidth) ||
        x >= box.right - parseFloat(style.borderRightWidth) ||
        y < box.top + parseFloat(style.borderTopWidth) ||
        y >= box.bottom - parseFloat(style.borderBottomWidth);
      if (!hairline) dead.push(`${x},${y} ${hit.tagName.toLowerCase()}.${hit.className}`);
    }
  }
  return dead;
};

test('macos: the whole toolbar row moves the window, in every view and width', async ({ page }) => {
  const states: [string, { width: number; height: number }, (p: typeof page) => Promise<void>][] = [
    ['overview', { width: 1360, height: 900 }, async () => undefined],
    [
      'reader',
      { width: 1360, height: 900 },
      (p) => p.locator('[data-testid^="job-row-"]').first().click(),
    ],
    ['rail', { width: 1000, height: 700 }, async () => undefined],
    ['narrow list', { width: 780, height: 560 }, async () => undefined],
    [
      'narrow reader',
      { width: 780, height: 560 },
      (p) => p.locator('[data-testid^="job-row-"]').first().click(),
    ],
    ['profile', { width: 1360, height: 900 }, (p) => p.getByTestId('nav-profile').click()],
    ['settings', { width: 1360, height: 900 }, (p) => p.getByTestId('nav-settings').click()],
    ['minimum', { width: 480, height: 360 }, async () => undefined],
  ];
  for (const [name, size, go] of states) {
    await page.setViewportSize(size);
    await open(page, '?platform=macos');
    await go(page);
    await settle(page);
    // Probe once the new stage or view has risen into place: halfway, the band of the row
    // stands a few pixels lower than the row.
    await motionSettled(page);
    expect(await page.evaluate(DRAG_PROBE), name).toEqual([]);
  }
  await page.setViewportSize({ width: 1360, height: 900 });
  await open(page, '?platform=macos&scenario=first-run');
  expect(await page.evaluate(DRAG_PROBE), 'first run').toEqual([]);
});

test('the native menu opens a view (macOS: Einstellungen with Cmd+,)', async ({ page }) => {
  await open(page, '?platform=macos');
  await page.evaluate(() => window.__harness.fire('navigate', 'settings'));
  await expect(page.getByTestId('view-settings')).toBeVisible();
  await page.evaluate(() => window.__harness.fire('navigate', 'nonsense'));
  await expect(page.getByTestId('view-settings')).toBeVisible();
});

test('per-OS convention: the order of dialog buttons', async ({ page }) => {
  const order = async (os: string): Promise<string[]> => {
    await open(page, `?platform=${os}`);
    await page.getByTestId('nav-settings').click();
    await page.getByTestId('full-mailbox').click();
    const dialog = page.getByTestId('dialog-full-mailbox');
    await expect(dialog).toBeVisible();
    return dialog.getByRole('button').allInnerTexts();
  };
  // Windows: the action first; macOS: cancel, then the action on the right.
  expect(await order('windows')).toEqual(['Lesen', 'Abbrechen']);
  expect(await order('macos')).toEqual(['Abbrechen', 'Lesen']);
});

test('the run status in the sidebar opens the last run', async ({ page }) => {
  await open(page, '?platform=windows');
  await expect(page.getByTestId('run-card')).toHaveCount(0);
  await expect(page.getByTestId('run-status')).toContainText('Abgerufen 08:30');
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('run-status').click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  // Seven new in the mails, one of them excluded: the run brought six new jobs.
  await expect(page.getByTestId('run-finished')).toContainText('6 neu');
  await page.getByTestId('run-toggle').click();
  await expect(page.getByTestId('last-new')).toHaveCount(0);
  await page.getByTestId('run-close').click();
  await expect(page.getByTestId('run-card')).toHaveCount(0);
});

// A click on the status opens the run card: before the first fetch there is none, so the
// status is not there at all (the first-run page says it) instead of a dead button.
test('the run status shows only when there is a run to open', async ({ page }) => {
  await open(page, '?platform=windows&scenario=first-run');
  await expect(page.getByTestId('view-first-run')).toBeVisible();
  await expect(page.getByTestId('run-status')).toHaveCount(0);
  await open(page, '?platform=windows');
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('run-status')).toBeVisible();
});

// In one column an open job hides the list; the status brings the list with the run card back.
test('narrow: the run status opens the run card even while a job is open', async ({ page }) => {
  await page.setViewportSize({ width: 780, height: 560 });
  await open(page, '?platform=windows');
  await page.getByTestId('job-rows').locator('[data-testid^="job-row-"]').first().click();
  await expect(page.getByTestId('list-scroll')).toBeHidden();
  await page.getByTestId('run-status').click();
  await expect(page.getByTestId('list-scroll')).toBeVisible();
  await expect(page.getByTestId('run-card')).toBeVisible();
});

test('every run status fits the sidebar without being cut off', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.getByTestId('nav-settings').click();
  const codes = [
    'connectingMail',
    'searchingMail',
    'readingMails',
    'fetchingDetails',
    'signingIn',
    'waiting',
    'scoring',
    'writingFiles',
  ] as const;
  // A run begins with its kind, like every run of the backend.
  await page.evaluate(() => {
    window.__harness.emit({ type: 'started', kind: 'fetch' });
    window.__harness.emit({ type: 'progress', step: 'scan', portal: null, done: 0, total: 3 });
  });
  for (const code of codes) {
    await page.evaluate(
      (c) => window.__harness.emit({ type: 'status', code: c, portal: 'freelance', until: null }),
      code,
    );
    const status = page.getByTestId('run-status');
    await expect(status).not.toContainText('Abgerufen');
    const cut = await status.locator('.text').evaluate((node) => ({
      text: node.textContent,
      cut: node.scrollHeight > node.clientHeight + 1 || node.scrollWidth > node.clientWidth + 1,
    }));
    expect(cut.cut, `${code}: ${cut.text}`).toBe(false);
  }
});

test('icon-only buttons show a styled tooltip after the delay', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.locator('[data-testid^="job-row-"]').first().click();
  const close = page.getByTestId('reader-close');
  await close.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Schließen');
  await expect(close).not.toHaveAttribute('title');
});

test('baseline: shell on Windows', async ({ page }) => {
  await open(page, '?platform=windows');
  await expectShot(page, 'shell-windows');
});

test('baseline: shell on macOS', async ({ page }) => {
  await open(page, '?platform=macos');
  await expectShot(page, 'shell-macos');
});
