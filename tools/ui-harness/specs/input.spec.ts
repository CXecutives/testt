// The input policy of lib/input/input.ts: like a native app. Controls react to the left
// button only, the middle button scrolls scroll areas, text a user would copy selects and
// copies. Keys: Tab moves the focus and Enter/Space press controls everywhere; fields take
// every character of the keyboard layout (AltGr, Option) and the editing keys of the OS;
// dialogs hold the focus; everything else is swallowed.

import type { Page } from '@playwright/test';
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
      // At rest no wheel listener that may cancel is attached (scrolling never waits for
      // the page); it is there only while Ctrl or Cmd is held.
      ctrlWheelAtRest: fire(view, new WheelEvent('wheel', { ...init, ctrlKey: true, deltaY: 1 })),
      holdControl: fire(
        view,
        new KeyboardEvent('keydown', { ...init, key: 'Control', ctrlKey: true }),
      ),
      ctrlWheel: fire(view, new WheelEvent('wheel', { ...init, ctrlKey: true, deltaY: 100 })),
      releaseControl: fire(view, new KeyboardEvent('keyup', { ...init, key: 'Control' })),
      ctrlWheelReleased: fire(view, new WheelEvent('wheel', { ...init, ctrlKey: true, deltaY: 1 })),
      holdCommand: fire(
        view,
        new KeyboardEvent('keydown', { ...init, key: 'Meta', metaKey: true }),
      ),
      metaWheel: fire(view, new WheelEvent('wheel', { ...init, metaKey: true, deltaY: 100 })),
      plainWheel: fire(view, new WheelEvent('wheel', { ...init, deltaY: 100 })),
      metaWheelAfterPlain: fire(
        view,
        new WheelEvent('wheel', { ...init, metaKey: true, deltaY: 1 }),
      ),
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
    ctrlWheelAtRest: false,
    holdControl: true,
    ctrlWheel: true,
    releaseControl: false,
    ctrlWheelReleased: false,
    holdCommand: true,
    metaWheel: true,
    // A wheel without the modifier means it was released unseen: the watch ends.
    plainWheel: false,
    metaWheelAfterPlain: false,
  });
});

test('controls react to the left button only', async ({ page }) => {
  const focused = (): Promise<string | null> =>
    page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? null);
  // Real middle and right clicks on a nav entry, a list row and a switch: nothing is pressed,
  // opened or focused.
  await page.getByTestId('nav-profile').click({ button: 'right' });
  await page.getByTestId('nav-settings').click({ button: 'middle' });
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  expect(await focused()).toBeNull();
  const row = page.locator('[data-testid^="job-row-"]').first();
  await row.click({ button: 'middle' });
  await row.click({ button: 'right' });
  await expect(page.getByTestId('reader')).toHaveCount(0);
  expect(await focused()).toBeNull();
  await page.getByTestId('nav-settings').click();
  const toggle = page.getByTestId('toggle-auto-fetch');
  await toggle.click({ button: 'right' });
  await toggle.click({ button: 'middle' });
  await page.waitForTimeout(300);
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  expect(await focused()).not.toBe('toggle-auto-fetch');
  const calls = await page.evaluate(() => window.__harness.calls.map(([name]) => name));
  expect(calls).not.toContain('save_settings');
  // Over a scroll area the middle click started the autoscroll of the engine (Windows
  // behaviour): the next click only ends it. Then the left button works.
  await page.getByTestId('settings-mailbox').getByRole('heading').click();
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
});

/** The parts of a KeyboardEventInit the tests use (serialisable into the page). */
interface Key {
  key: string;
  code?: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
  metaKey?: boolean;
  modifierAltGraph?: boolean;
}

/**
 * Dispatch keydowns (each `[label, init]`) on a fresh field, the jobs view or the Abrufen
 * button and report which ones the page cancelled.
 */
async function prevented(
  page: Page,
  where: 'field' | 'view' | 'button',
  keys: [string, Key][],
): Promise<Record<string, boolean>> {
  return page.evaluate(
    ({ where, keys }) => {
      getSelection()?.removeAllRanges();
      const field = document.body.appendChild(document.createElement('input'));
      const target =
        where === 'field'
          ? field
          : where === 'button'
            ? document.querySelector('[data-testid="fetch"]')!
            : document.querySelector('[data-testid="view-jobs"]')!;
      const out: Record<string, boolean> = {};
      for (const [label, init] of keys) {
        const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
        target.dispatchEvent(event);
        out[label] = event.defaultPrevented;
      }
      field.remove();
      return out;
    },
    { where, keys },
  );
}

const plain = (key: string): [string, Key] => [key, { key }];

test('keys outside fields: Tab moves, Enter and Space press, everything else is swallowed', async ({
  page,
}) => {
  const outside = await prevented(page, 'view', [
    ...['Enter', 'Escape', 'F5', 'r', 'ArrowDown', ' '].map(plain),
    ['Ctrl+R', { key: 'r', ctrlKey: true }],
    ['Ctrl+C without selection', { key: 'c', ctrlKey: true }],
    ['Alt+ArrowLeft', { key: 'ArrowLeft', altKey: true }],
    ['Ctrl+Tab', { key: 'Tab', ctrlKey: true }],
  ]);
  expect(Object.entries(outside).filter(([, cancelled]) => !cancelled)).toEqual([]);
  const moves = await prevented(page, 'view', [
    ['Tab', { key: 'Tab' }],
    ['Shift+Tab', { key: 'Tab', shiftKey: true }],
    ['Alt+F4', { key: 'F4', altKey: true }],
  ]);
  expect(moves).toEqual({ Tab: false, 'Shift+Tab': false, 'Alt+F4': false });
  // On a focused button Enter and Space press it (the engine clicks), like a native one.
  const onButton = await prevented(page, 'button', [
    ['Enter', { key: 'Enter' }],
    ['Space', { key: ' ' }],
    ['Tab', { key: 'Tab' }],
    ['Ctrl+Enter', { key: 'Enter', ctrlKey: true }],
    ['r', { key: 'r' }],
  ]);
  expect(onButton).toEqual({ Enter: false, Space: false, Tab: false, 'Ctrl+Enter': true, r: true });
});

test('Tab from a field goes on through the controls, Enter and Space press them', async ({
  page,
}) => {
  const focused = (): Promise<string | null> =>
    page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? null);
  await page.getByTestId('search').focus();
  await page.keyboard.press('Tab');
  const first = await focused();
  expect(first).not.toBe('search');
  await page.keyboard.press('Tab');
  expect(await focused()).not.toBe(first);
  await page.keyboard.press('Shift+Tab');
  expect(await focused()).toBe(first);
  await page.keyboard.press('Shift+Tab');
  expect(await focused()).toBe('search');
  // Enter presses a focused nav entry, Space a focused switch.
  await page.getByTestId('nav-settings').focus();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('view-settings')).toBeVisible();
  const toggle = page.getByTestId('toggle-auto-fetch');
  const before = await toggle.getAttribute('aria-checked');
  await toggle.focus();
  await page.keyboard.press('Space');
  await expect(toggle).not.toHaveAttribute('aria-checked', before!);
});

test('fields keep typing, the clipboard keys and the editing keys; shortcuts stay out', async ({
  page,
}) => {
  const allowed = await prevented(page, 'field', [
    ...['a', 'Z', '@', 'Backspace', 'Delete', 'ArrowLeft', 'Home', 'End', 'Tab', 'Enter'].map(
      plain,
    ),
    ['Escape', { key: 'Escape' }],
    ['Shift+Home', { key: 'Home', shiftKey: true }],
    ...['c', 'v', 'x', 'a', 'z'].map((k): [string, Key] => [
      `Ctrl+${k}`,
      { key: k, ctrlKey: true },
    ]),
    ['Ctrl+Shift+Z', { key: 'Z', ctrlKey: true, shiftKey: true }],
  ]);
  expect(Object.entries(allowed).filter(([, cancelled]) => cancelled)).toEqual([]);
  const blocked = await prevented(page, 'field', [
    ['F5', { key: 'F5' }],
    ['F12', { key: 'F12' }],
    ['Ctrl+P', { key: 'p', ctrlKey: true }],
    ['Ctrl+F', { key: 'f', ctrlKey: true }],
    ['Ctrl++', { key: '+', ctrlKey: true }],
    ['Ctrl+Shift+I', { key: 'I', ctrlKey: true, shiftKey: true }],
  ]);
  expect(Object.entries(blocked).filter(([, cancelled]) => !cancelled)).toEqual([]);
});

/** Characters of the Option (macOS) and AltGr (Windows) layer of German and other layouts. */
const LAYER = ['@', '€', '{', '}', '[', ']', '|', '~', '\\', 'µ', '²', 'ą'];

test('Windows: AltGr (and Ctrl+Alt) characters type; Alt alone and Alt+Arrow do not', async ({
  page,
}) => {
  const typed = await prevented(page, 'field', [
    ...LAYER.map((key): [string, Key] => [
      `AltGr ${key}`,
      { key, ctrlKey: true, altKey: true, modifierAltGraph: true },
    ]),
    // Left Ctrl+Alt works as AltGr on Windows but does not report the AltGraph modifier.
    ...LAYER.map((key): [string, Key] => [`Ctrl+Alt ${key}`, { key, ctrlKey: true, altKey: true }]),
  ]);
  expect(Object.entries(typed).filter(([, cancelled]) => cancelled)).toEqual([]);
  const editing = await prevented(page, 'field', [
    ['Ctrl+ArrowLeft', { key: 'ArrowLeft', ctrlKey: true }],
    ['Ctrl+Shift+ArrowRight', { key: 'ArrowRight', ctrlKey: true, shiftKey: true }],
    ['Ctrl+Backspace', { key: 'Backspace', ctrlKey: true }],
    ['Ctrl+Delete', { key: 'Delete', ctrlKey: true }],
    ['Ctrl+Home', { key: 'Home', ctrlKey: true }],
    ['Ctrl+Shift+End', { key: 'End', ctrlKey: true, shiftKey: true }],
    ['Ctrl+Y', { key: 'y', ctrlKey: true }],
  ]);
  expect(Object.entries(editing).filter(([, cancelled]) => cancelled)).toEqual([]);
  const blocked = await prevented(page, 'field', [
    ['Alt+d', { key: 'd', altKey: true }],
    ['Alt+ArrowLeft', { key: 'ArrowLeft', altKey: true }],
    ['Alt+ArrowRight', { key: 'ArrowRight', altKey: true }],
    ['Ctrl+Alt+d', { key: 'd', ctrlKey: true, altKey: true }],
    ['Cmd+ArrowLeft', { key: 'ArrowLeft', metaKey: true }],
    ['Ctrl+E', { key: 'e', ctrlKey: true }],
  ]);
  expect(Object.entries(blocked).filter(([, cancelled]) => !cancelled)).toEqual([]);
});

test('macOS: Option characters, dead keys and word moves type; Cmd editing keys work', async ({
  page,
}) => {
  await open(page, '?platform=macos');
  const typed = await prevented(page, 'field', [
    ...LAYER.map((key): [string, Key] => [`Option ${key}`, { key, altKey: true }]),
    ['Option+Shift 7 (backslash)', { key: '\\', altKey: true, shiftKey: true }],
    ['Option dead key', { key: 'Dead', altKey: true }],
    ['Option+ArrowLeft', { key: 'ArrowLeft', altKey: true }],
    ['Option+Shift+ArrowRight', { key: 'ArrowRight', altKey: true, shiftKey: true }],
    ['Option+Backspace', { key: 'Backspace', altKey: true }],
    ['Cmd+ArrowLeft', { key: 'ArrowLeft', metaKey: true }],
    ['Cmd+Shift+ArrowRight', { key: 'ArrowRight', metaKey: true, shiftKey: true }],
    ['Cmd+Backspace', { key: 'Backspace', metaKey: true }],
    ['Cmd+ArrowUp', { key: 'ArrowUp', metaKey: true }],
    ['Cmd+Shift+Z', { key: 'z', metaKey: true, shiftKey: true }],
    ...['c', 'v', 'x', 'a', 'z'].map((k): [string, Key] => [`Cmd+${k}`, { key: k, metaKey: true }]),
    ...['a', 'e', 'k'].map((k): [string, Key] => [`Ctrl+${k}`, { key: k, ctrlKey: true }]),
  ]);
  expect(Object.entries(typed).filter(([, cancelled]) => cancelled)).toEqual([]);
  const blocked = await prevented(page, 'field', [
    ['Cmd+P', { key: 'p', metaKey: true }],
    ['Cmd+F', { key: 'f', metaKey: true }],
    ['Cmd+R', { key: 'r', metaKey: true }],
    ['Cmd++', { key: '+', metaKey: true }],
    ['Cmd+Y', { key: 'y', metaKey: true }],
    ['Cmd+Option+I', { key: 'ˆ', code: 'KeyI', metaKey: true, altKey: true }],
    ['Ctrl+ArrowLeft', { key: 'ArrowLeft', ctrlKey: true }],
    ['F5', { key: 'F5' }],
  ]);
  expect(Object.entries(blocked).filter(([, cancelled]) => !cancelled)).toEqual([]);
});

test('macOS: the menu shortcuts reach the menu, in a field and outside (Cmd+, too)', async ({
  page,
}) => {
  await open(page, '?platform=macos');
  const menu: [string, Key][] = [
    ['Cmd+,', { key: ',', metaKey: true }],
    ['Cmd+Q', { key: 'q', metaKey: true }],
    ['Cmd+W', { key: 'w', metaKey: true }],
    ['Cmd+M', { key: 'm', metaKey: true }],
    ['Cmd+H', { key: 'h', metaKey: true }],
    ['Cmd+Option+H', { key: '˙', code: 'KeyH', metaKey: true, altKey: true }],
  ];
  for (const where of ['view', 'field'] as const) {
    const result = await prevented(page, where, menu);
    expect(
      Object.entries(result).filter(([, cancelled]) => cancelled),
      where,
    ).toEqual([]);
  }
});

test('the list search clears on Esc; a click on its magnifier lands in the field', async ({
  page,
}) => {
  const search = page.getByTestId('search');
  await search.fill('Controlling');
  await search.press('Escape');
  await expect(search).toHaveValue('');
  await expect(search).toBeFocused();
  // An empty search passes Esc on (nothing happens, no error).
  await search.press('Escape');
  await expect(search).toHaveValue('');
  await search.evaluate((node) => node.blur());
  const box = (await search.boundingBox())!;
  // The magnifier sits in the first 36 px of the box.
  await page.mouse.click(box.x + 20, box.y + box.height / 2);
  await expect(search).toBeFocused();
});

test('the password eye and the search clear keep the caret in the field', async ({ page }) => {
  await open(page, '?gallery&platform=windows');
  const section = page.getByTestId('gallery-inputs');
  const password = page.locator('#gallery-password');
  await password.click();
  await page.keyboard.press('End');
  const eye = section.getByRole('button', { name: 'Passwort zeigen' });
  await expect(eye).toHaveAttribute('tabindex', '-1');
  await eye.click();
  await expect(password).toHaveAttribute('type', 'text');
  await expect(password).toBeFocused();
  await page.keyboard.type('x');
  await expect(password).toHaveValue('abcd efghx');
  // Tab skips the eye: from the password field it leaves the field.
  await page.keyboard.press('Tab');
  await expect(section.getByRole('button', { name: 'Passwort verbergen' })).not.toBeFocused();
  const search = section.getByRole('textbox', { name: 'Jobs durchsuchen' }).first();
  await search.click();
  const clear = section.getByRole('button', { name: 'Suche leeren' });
  await expect(clear).toHaveAttribute('tabindex', '-1');
  await clear.click();
  await expect(search).toHaveValue('');
  await expect(search).toBeFocused();
});

test('a dialog holds the focus: Tab cycles, a click on its text keeps Esc working', async ({
  page,
}) => {
  await open(page, '?gallery&platform=windows');
  // Opened from the keyboard: Enter presses the focused button.
  const opener = page.getByTestId('open-danger');
  await opener.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByTestId('dialog-danger');
  await expect(dialog).toBeVisible();
  const inside = (): Promise<boolean> =>
    dialog.evaluate((node) => node.contains(document.activeElement));
  for (const key of ['Tab', 'Tab', 'Tab', 'Shift+Tab', 'Shift+Tab', 'Shift+Tab']) {
    await page.keyboard.press(key);
    expect(await inside(), key).toBe(true);
  }
  // A click on the text keeps the focus in the dialog; Esc still closes it and the focus
  // goes back to the button that opened it.
  await dialog.getByRole('heading').click();
  expect(await inside()).toBe(true);
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(opener).toBeFocused();
  // Focus that slipped behind the dialog: Esc still answers the dialog.
  await page.keyboard.press('Enter');
  await expect(dialog).toBeVisible();
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

test('a dialog shows the failure of its action inside and stays open', async ({ page }) => {
  await open(page, '?gallery&platform=windows');
  await page.getByTestId('open-danger').click();
  const dialog = page.getByTestId('dialog-danger');
  await dialog.getByTestId('dialog-confirm').focus();
  await page.keyboard.press('Enter');
  await expect(dialog.getByTestId('dialog-error')).toBeVisible();
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

test('a switch flips at once and slides back when its save fails', async ({ page }) => {
  await open(page, '?gallery&platform=windows');
  const toggle = page.getByTestId('toggle-fails');
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  await toggle.click();
  // At once, before the save answers (it takes 400 ms, then fails).
  await expect(toggle).toHaveAttribute('aria-checked', 'true', { timeout: 100 });
  await expect(toggle).toHaveAttribute('aria-checked', 'false', { timeout: 2000 });
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
