// The input policy of lib/input/input.ts: like a native app. Controls react to the left
// button only, the middle button scrolls scroll areas, text a user would copy selects and
// copies, keys work only in fields.

import { expect, open, test } from './fixtures';

test.beforeEach(async ({ page }) => {
  await open(page, '?platform=windows');
});

test('right click, middle click and drag: what the page lets through', async ({ page }) => {
  // A list long enough to scroll: the middle button may start the autoscroll there.
  await page.setViewportSize({ width: 1360, height: 560 });
  await page.getByTestId('facet').getByRole('radio', { name: /Alle/ }).click();
  const result = await page.evaluate(() => {
    const field = document.body.appendChild(document.createElement('input'));
    const view = document.querySelector('[data-testid="view-jobs"]')!;
    const header = document.querySelector('[data-testid="list-header"]')!;
    const row = document.querySelector('[data-testid^="job-row-"]')!;
    const scroll = document.querySelector('[data-testid="list-scroll"]')!;
    const fire = (target: Element, event: Event): boolean => {
      target.dispatchEvent(event);
      return event.defaultPrevented;
    };
    const init = { bubbles: true, cancelable: true };
    const out = {
      scrolls: scroll.scrollHeight > scroll.clientHeight,
      contextmenu: fire(view, new MouseEvent('contextmenu', init)),
      contextmenuInField: fire(field, new MouseEvent('contextmenu', init)),
      middleDownOutsideScrollArea: fire(
        header,
        new MouseEvent('mousedown', { ...init, button: 1 }),
      ),
      middleDownOnListRow: fire(row, new MouseEvent('mousedown', { ...init, button: 1 })),
      rightDown: fire(row, new MouseEvent('mousedown', { ...init, button: 2 })),
      leftDown: fire(view, new MouseEvent('mousedown', { ...init, button: 0 })),
      backUp: fire(view, new MouseEvent('mouseup', { ...init, button: 3 })),
      auxclick: fire(row, new MouseEvent('auxclick', { ...init, button: 1 })),
      dragstart: fire(view, new Event('dragstart', init)),
      selectstart: fire(view, new Event('selectstart', init)),
      selectstartInField: fire(field, new Event('selectstart', init)),
      dblclick: fire(view, new MouseEvent('dblclick', init)),
      dblclickTitlebar: fire(
        document.querySelector('[data-testid="titlebar"]')!,
        new MouseEvent('dblclick', init),
      ),
      ctrlWheel: fire(view, new WheelEvent('wheel', { ...init, ctrlKey: true, deltaY: 100 })),
      plainWheel: fire(view, new WheelEvent('wheel', { ...init, deltaY: 100 })),
    };
    field.remove();
    return out;
  });
  expect(result).toEqual({
    scrolls: true,
    contextmenu: true,
    contextmenuInField: true,
    middleDownOutsideScrollArea: true,
    middleDownOnListRow: false,
    rightDown: true,
    leftDown: false,
    backUp: true,
    auxclick: true,
    dragstart: true,
    selectstart: true,
    selectstartInField: false,
    dblclick: true,
    dblclickTitlebar: false,
    ctrlWheel: true,
    plainWheel: false,
  });
});

test('controls react to the left button only', async ({ page }) => {
  const focused = (): Promise<string | null> =>
    page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? null);
  // Real middle and right clicks on a nav entry, a list row, a switch and a caption button:
  // nothing is pressed, opened or focused.
  await page.getByTestId('nav-profile').click({ button: 'right' });
  await page.getByTestId('nav-settings').click({ button: 'middle' });
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  expect(await focused()).toBeNull();
  const row = page.locator('[data-testid^="job-row-"]').first();
  await row.click({ button: 'middle' });
  await row.click({ button: 'right' });
  await expect(page.getByTestId('reader')).toHaveCount(0);
  expect(await focused()).toBeNull();
  await page.getByTestId('window-controls').getByRole('button').nth(1).click({ button: 'middle' });
  await page.getByTestId('nav-settings').click();
  const toggle = page.getByTestId('toggle-auto-fetch');
  await toggle.click({ button: 'right' });
  await toggle.click({ button: 'middle' });
  await page.waitForTimeout(300);
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  expect(await focused()).not.toBe('toggle-auto-fetch');
  const calls = await page.evaluate(() => window.__harness.calls.map(([name]) => name));
  expect(calls).not.toContain('window.toggleMaximize');
  expect(calls).not.toContain('save_settings');
  // Over a scroll area the middle click started the autoscroll of the engine (Windows
  // behaviour): the next click only ends it. Then the left button works.
  await page.getByTestId('settings-mailbox').getByRole('heading').click();
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
});

test('keys do nothing outside fields; fields keep typing and clipboard keys', async ({ page }) => {
  const result = await page.evaluate(() => {
    const field = document.body.appendChild(document.createElement('input'));
    const view = document.querySelector('[data-testid="view-jobs"]')!;
    const key = (target: Element, init: KeyboardEventInit): boolean => {
      const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
      target.dispatchEvent(event);
      return event.defaultPrevented;
    };
    getSelection()?.removeAllRanges();
    const out = {
      outside: ['Enter', 'Escape', 'Tab', 'F5', 'r', 'ArrowDown', ' '].map((k) =>
        key(view, { key: k }),
      ),
      outsideCtrlR: key(view, { key: 'r', ctrlKey: true }),
      outsideCopyWithoutSelection: key(view, { key: 'c', ctrlKey: true }),
      altF4: key(view, { key: 'F4', altKey: true }),
      typing: ['a', 'Z', '@', 'Backspace', 'ArrowLeft', 'Tab', 'Enter', 'Escape'].map((k) =>
        key(field, { key: k }),
      ),
      clipboard: ['c', 'v', 'x', 'a', 'z'].map((k) => key(field, { key: k, ctrlKey: true })),
      blockedInField: [
        key(field, { key: 'F5' }),
        key(field, { key: 'p', ctrlKey: true }),
        key(field, { key: 'f', ctrlKey: true }),
        key(field, { key: '+', ctrlKey: true }),
      ],
    };
    field.remove();
    return out;
  });
  expect(result.outside.every(Boolean)).toBe(true);
  expect(result.outsideCtrlR).toBe(true);
  expect(result.outsideCopyWithoutSelection).toBe(true);
  expect(result.altF4).toBe(false);
  expect(result.typing.some(Boolean)).toBe(false);
  expect(result.clipboard.some(Boolean)).toBe(false);
  expect(result.blockedInField.every(Boolean)).toBe(true);
});

test('the ad text, title and facts select and copy; the rest does not select', async ({
  page,
  browserName,
}) => {
  await page.locator('[data-testid^="job-row-"]').first().click();
  const text = page.getByTestId('ad-text');
  await expect(text).toBeVisible();
  // A drag across the ad text selects it, like in a document.
  const box = (await text.boundingBox())!;
  await page.mouse.move(box.x + 4, box.y + 6);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 8, box.y + 40, { steps: 8 });
  await page.mouse.up();
  const selected = await page.evaluate(() => getSelection()?.toString() ?? '');
  expect(selected.length).toBeGreaterThan(10);
  // Ctrl/Cmd+C goes through to the web view while there is a selection.
  const copyLetThrough = await page.evaluate(() => {
    const event = new KeyboardEvent('keydown', {
      key: 'c',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    document.querySelector('[data-testid="ad-text"]')!.dispatchEvent(event);
    return !event.defaultPrevented;
  });
  expect(copyLetThrough).toBe(true);
  if (browserName === 'chromium') {
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.keyboard.press('Control+c');
    const copied = await page.evaluate(() => navigator.clipboard.readText());
    expect(copied.trim()).toBe(selected.trim());
  }
  // Title and facts select too; controls and labels do not (a drag over them selects
  // nothing).
  const drag = async (id: string): Promise<string> => {
    await page.evaluate(() => getSelection()?.removeAllRanges());
    const target = (await page.getByTestId(id).boundingBox())!;
    await page.mouse.move(target.x + 2, target.y + target.height / 2);
    await page.mouse.down();
    await page.mouse.move(target.x + target.width - 2, target.y + target.height / 2, {
      steps: 6,
    });
    await page.mouse.up();
    return page.evaluate(() => getSelection()?.toString() ?? '');
  };
  expect((await drag('reader-title')).length).toBeGreaterThan(5);
  for (const id of ['band', 'must', 'reasons-met']) {
    expect(await drag(id), id).toBe('');
  }
});

test('native cursor: the arrow on controls, the text cursor on copyable text', async ({ page }) => {
  await expect(page.getByTestId('fetch')).toHaveCSS('cursor', 'default');
  await expect(page.getByTestId('nav-profile')).toHaveCSS('cursor', 'default');
  await page.locator('[data-testid^="job-row-"]').first().click();
  await expect(page.getByTestId('ad-text')).toHaveCSS('cursor', 'text');
});
