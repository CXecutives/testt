// Final round, team T1 (input): only the left button presses, a click beside a field ends its
// focus, Enter presses buttons only, the arrows choose in a radio group, the middle button
// stays out of dialogs and fields, the keys of the Windows window menus reach the OS, and
// keyboard focus stays clear of the scroll edges.

import type { Locator, Page } from '@playwright/test';
import { calls, expect, open, test } from './fixtures';

const WIN = '?platform=windows';
const GALLERY = '?gallery&platform=windows';

/** What a pressed look changes: the colours and the scale of the control and its track. */
async function look(target: Locator): Promise<string> {
  return target.evaluate((node) => {
    const parts = [node, ...node.querySelectorAll('.track, .pill')];
    return parts
      .map((part) => {
        const style = getComputedStyle(part);
        return `${style.backgroundColor} ${style.color} ${style.transform}`;
      })
      .join(' | ');
  });
}

/** The look under the pointer at rest, and while the given button is held on it. */
async function heldLook(
  page: Page,
  target: Locator,
  button: 'right' | 'middle',
): Promise<{ hover: string; held: string; after: string }> {
  const box = (await target.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.waitForTimeout(200);
  const hover = await look(target);
  await page.mouse.down({ button });
  await page.waitForTimeout(200);
  const held = await look(target);
  await page.mouse.up({ button });
  await page.waitForTimeout(200);
  const after = await look(target);
  // Away, so the next control starts at rest (and a started autoscroll ends).
  await page.mouse.move(box.x + box.width / 2 + 1, box.y + box.height / 2 + 1);
  await page.mouse.move(4, 4);
  await page.waitForTimeout(200);
  return { hover, held, after };
}

test('the right and the middle button never press a control', async ({ page }) => {
  // Eight held presses, each waiting for the transitions to settle.
  test.setTimeout(60_000);
  await open(page, WIN);
  const targets = [
    page.getByTestId('fetch'),
    page.locator('[data-testid^="job-row-"]').first(),
    page.getByTestId('nav-settings'),
    page.getByTestId('facet').getByRole('radio').nth(1),
  ];
  for (const target of targets) {
    for (const button of ['right', 'middle'] as const) {
      const { hover, held, after } = await heldLook(page, target, button);
      expect(held, `${button} held on ${String(target)}`).toBe(hover);
      expect(after).toBe(hover);
    }
  }
  expect(await calls(page, 'start_run')).toHaveLength(0);
  await expect(page.getByTestId('reader')).toHaveCount(0);
  await expect(page.getByTestId('nav-jobs')).toHaveAttribute('aria-current', 'page');
  // The left button still presses.
  const fetch = page.getByTestId('fetch');
  const box = (await fetch.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.waitForTimeout(200);
  const hover = await look(fetch);
  await page.mouse.down();
  await page.waitForTimeout(200);
  expect(await look(fetch)).not.toBe(hover);
  await page.mouse.move(4, 4, { steps: 4 });
  await page.mouse.up();
  expect(await calls(page, 'start_run')).toHaveLength(0);
});

test('a switch held with the right button looks at rest and keeps its state', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-settings').click();
  const toggle = page.getByTestId('toggle-auto-fetch');
  await expect(toggle).toBeVisible();
  const before = await toggle.getAttribute('aria-checked');
  for (const button of ['right', 'middle'] as const) {
    const { hover, held } = await heldLook(page, toggle, button);
    expect(held).toBe(hover);
  }
  await expect(toggle).toHaveAttribute('aria-checked', before!);
});

test('a click beside a focused field ends its focus', async ({ page }) => {
  await open(page, GALLERY);
  const field = page.locator('#gallery-address');
  await field.scrollIntoViewIfNeeded();
  const box = (await field.boundingBox())!;
  const label = page.locator('label[for="gallery-address"]');
  const labelBox = (await label.boundingBox())!;
  const beside: [string, () => Promise<void>][] = [
    [
      'the empty part of the label line',
      () => page.mouse.click(box.x + box.width - 4, labelBox.y + labelBox.height / 2),
    ],
    ['the hint below', () => page.locator('#gallery-address-message').click()],
    ['the heading', () => page.getByTestId('gallery-inputs').getByRole('heading').first().click()],
    [
      'the empty area to the right',
      () => page.mouse.click(box.x + box.width + 24, box.y + box.height / 2),
    ],
  ];
  for (const [where, click] of beside) {
    await field.click();
    await expect(field).toBeFocused();
    await click();
    await expect(field, where).not.toBeFocused();
  }
  // The words of its own label still lead into the field.
  await field.click();
  await page.mouse.click(labelBox.x + 4, labelBox.y + labelBox.height / 2);
  await expect(field).toBeFocused();
  // The chips and the empty part of a chip field keep its caret.
  const chips = page.locator('#gallery-chips');
  await chips.click();
  const chipBox = (await page.getByTestId('gallery-chips').boundingBox())!;
  await page.mouse.click(chipBox.x + chipBox.width - 6, chipBox.y + chipBox.height / 2);
  await expect(chips).toBeFocused();
});

test('a press on the title bar ends the focus of a field', async ({ page }) => {
  // Tauri's drag script cancels the press on a drag region (the window moves instead):
  // stand in for it, after the input policy's own listener like in the app.
  await page.addInitScript(() => {
    document.addEventListener('mousedown', (event) => {
      const target = event.target instanceof Element ? event.target : null;
      if (
        event.button === 0 &&
        target?.closest('[data-tauri-drag-region]') &&
        !target.closest('button')
      ) {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
    });
  });
  await open(page, WIN);
  const search = page.getByTestId('search');
  await search.click();
  await expect(search).toBeFocused();
  const bar = (await page.getByTestId('titlebar').boundingBox())!;
  await page.mouse.click(bar.x + bar.width / 2, bar.y + bar.height / 2);
  await expect(search).not.toBeFocused();
});

test('Enter presses buttons only; Space toggles a switch', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-settings').click();
  const toggle = page.getByTestId('toggle-auto-fetch');
  const before = await toggle.getAttribute('aria-checked');
  await toggle.focus();
  await page.keyboard.press('Enter');
  await page.waitForTimeout(200);
  await expect(toggle).toHaveAttribute('aria-checked', before!);
  await page.keyboard.press('Space');
  await expect(toggle).not.toHaveAttribute('aria-checked', before!);
});

test('a radio group is one Tab stop and the arrows choose', async ({ page }) => {
  await open(page, WIN);
  const radios = page.getByTestId('facet').getByRole('radio');
  const count = await radios.count();
  expect(count).toBeGreaterThan(1);
  const stops = await radios.evaluateAll((nodes) =>
    nodes.map((node) => `${node.getAttribute('aria-checked')}:${(node as HTMLElement).tabIndex}`),
  );
  expect(stops.filter((stop) => stop.endsWith(':0'))).toEqual(['true:0']);
  const checked = radios.and(page.locator('[aria-checked="true"]'));
  const first = await checked.textContent();
  await checked.focus();
  await page.keyboard.press('ArrowRight');
  const now = page.getByTestId('facet').locator('[aria-checked="true"]');
  await expect(now).not.toHaveText(first!);
  await expect(now).toBeFocused();
  await page.keyboard.press('ArrowLeft');
  await expect(page.getByTestId('facet').locator('[aria-checked="true"]')).toHaveText(first!);
  // Left from the first option wraps to the last.
  await radios.first().click();
  await radios.first().focus();
  await page.keyboard.press('ArrowLeft');
  await expect(radios.last()).toHaveAttribute('aria-checked', 'true');
});

test('the middle button stays out of dialogs and fields', async ({ page }) => {
  await open(page, GALLERY);
  await page.getByTestId('open-danger').click();
  const dialog = page.getByTestId('dialog-danger');
  await expect(dialog).toBeVisible();
  const prevented = await page.evaluate(() => {
    const scrim = document.querySelector('[aria-modal="true"]')!.parentElement!;
    const event = new MouseEvent('mousedown', { bubbles: true, cancelable: true, button: 1 });
    scrim.dispatchEvent(event);
    return event.defaultPrevented;
  });
  expect(prevented).toBe(true);
  // A real middle click on the backdrop: Enter then still answers the dialog.
  await page.mouse.click(8, 200, { button: 'middle' });
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(dialog).toHaveCount(0);
  // A middle click on a field in a scrolling view does not focus it.
  await page.mouse.click(4, 4);
  const field = page.locator('#gallery-address');
  await field.scrollIntoViewIfNeeded();
  await field.click({ button: 'middle' });
  await page.waitForTimeout(100);
  await expect(field).not.toBeFocused();
});

test('Windows: Alt+Space and Shift+F10 reach the OS', async ({ page }) => {
  await open(page, WIN);
  const results = await page.evaluate(() => {
    const field = document.querySelector<HTMLInputElement>('[data-testid="search"]')!;
    const button = document.querySelector<HTMLElement>('[data-testid="fetch"]')!;
    const press = (target: Element, init: KeyboardEventInit): boolean => {
      const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
      target.dispatchEvent(event);
      return event.defaultPrevented;
    };
    const altSpace = { key: ' ', code: 'Space', altKey: true };
    const shiftF10 = { key: 'F10', code: 'F10', shiftKey: true };
    return {
      altSpaceBody: press(document.body, altSpace),
      altSpaceField: press(field, altSpace),
      altSpaceButton: press(button, altSpace),
      shiftF10Field: press(field, shiftF10),
      f10Field: press(field, { key: 'F10', code: 'F10' }),
    };
  });
  expect(results).toEqual({
    altSpaceBody: false,
    altSpaceField: false,
    altSpaceButton: false,
    shiftF10Field: false,
    f10Field: true,
  });
});

test('a field menu greys out Undo while there is nothing to undo', async ({ page }) => {
  await open(page, WIN);
  const search = page.getByTestId('search');
  await search.click({ button: 'right' });
  const undo = async (): Promise<boolean | undefined> =>
    page.evaluate(
      () => window.__harness.menus.at(-1)?.find((entry) => entry.text === 'Rückgängig')?.enabled,
    );
  expect(await undo()).toBe(false);
  await search.click();
  await page.keyboard.type('CFO');
  await search.click({ button: 'right' });
  expect(await undo()).toBe(true);
});

test('keyboard focus stays clear of the edges of its scroll area', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 600 });
  await open(page, WIN);
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('nav-settings').focus();
  const cut: string[] = [];
  for (let stop = 0; stop < 30; stop += 1) {
    await page.keyboard.press('Tab');
    await page.waitForTimeout(60);
    const gap = await page.evaluate(() => {
      const node = document.activeElement;
      if (!(node instanceof HTMLElement)) return null;
      let pane = node.parentElement;
      while (
        pane &&
        !(
          /auto|scroll/.test(getComputedStyle(pane).overflowY) &&
          pane.scrollHeight > pane.clientHeight
        )
      ) {
        pane = pane.parentElement;
      }
      if (pane === null) return null;
      const box = node.getBoundingClientRect();
      const view = pane.getBoundingClientRect();
      const top = view.top + pane.clientTop;
      return {
        id: node.getAttribute('data-testid') ?? node.tagName,
        above: box.top - top,
        below: top + pane.clientHeight - box.bottom,
      };
    });
    if (gap !== null && (gap.above < 4 || gap.below < 4)) cut.push(JSON.stringify(gap));
  }
  expect(cut).toEqual([]);
});

test('a waiting fold arrow gives the Jobs entry no hover wash', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-archive').click();
  const fold = page.getByTestId('places-toggle');
  await expect(fold).toHaveAttribute('aria-disabled', 'true');
  const jobs = page.getByTestId('nav-jobs');
  await page.mouse.move(4, 600);
  await page.waitForTimeout(200);
  const rest = await jobs.evaluate((node) => getComputedStyle(node).backgroundColor);
  await fold.hover();
  await page.waitForTimeout(200);
  expect(await jobs.evaluate((node) => getComputedStyle(node).backgroundColor)).toBe(rest);
});
