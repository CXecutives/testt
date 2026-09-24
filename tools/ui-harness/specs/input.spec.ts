// The input policy of lib/input/input.ts: left click and hover only; keys only in fields.

import { expect, open, test } from './fixtures';

test.beforeEach(async ({ page }) => {
  await open(page, '?platform=windows');
});

test('right click, middle click and drag do nothing outside and inside fields', async ({
  page,
}) => {
  const result = await page.evaluate(() => {
    const field = document.body.appendChild(document.createElement('input'));
    const view = document.querySelector('[data-testid="view-jobs"]')!;
    const fire = (target: Element, event: Event): boolean => {
      target.dispatchEvent(event);
      return event.defaultPrevented;
    };
    const init = { bubbles: true, cancelable: true };
    const out = {
      contextmenu: fire(view, new MouseEvent('contextmenu', init)),
      contextmenuInField: fire(field, new MouseEvent('contextmenu', init)),
      middleDown: fire(view, new MouseEvent('mousedown', { ...init, button: 1 })),
      rightDown: fire(view, new MouseEvent('mousedown', { ...init, button: 2 })),
      leftDown: fire(view, new MouseEvent('mousedown', { ...init, button: 0 })),
      auxclick: fire(view, new MouseEvent('auxclick', { ...init, button: 1 })),
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
    contextmenu: true,
    contextmenuInField: true,
    middleDown: true,
    rightDown: true,
    leftDown: false,
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

test('keys do nothing outside fields; fields keep typing and clipboard keys', async ({ page }) => {
  const result = await page.evaluate(() => {
    const field = document.body.appendChild(document.createElement('input'));
    const view = document.querySelector('[data-testid="view-jobs"]')!;
    const key = (target: Element, init: KeyboardEventInit): boolean => {
      const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
      target.dispatchEvent(event);
      return event.defaultPrevented;
    };
    const out = {
      outside: ['Enter', 'Escape', 'Tab', 'F5', 'r', 'ArrowDown', ' '].map((k) =>
        key(view, { key: k }),
      ),
      outsideCtrlR: key(view, { key: 'r', ctrlKey: true }),
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
  expect(result.altF4).toBe(false);
  expect(result.typing.some(Boolean)).toBe(false);
  expect(result.clipboard.some(Boolean)).toBe(false);
  expect(result.blockedInField.every(Boolean)).toBe(true);
});

test('real right and middle clicks change nothing', async ({ page }) => {
  await page.getByTestId('tab-profile').click({ button: 'right' });
  await page.getByTestId('tab-settings').click({ button: 'middle' });
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await page.keyboard.press('Tab');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('view-jobs')).toBeVisible();
});
