import { expect, expectShot, open, settle, test } from './fixtures';

const SECTIONS = [
  'colours',
  'type',
  'spacing',
  'radii',
  'shadows',
  'gradients',
  'motion',
  'icons',
  'buttons',
  'navigation',
  'tiles',
  'empty',
  'activity',
  'cards',
  'badges',
  'loading',
  'split',
  'inputs',
  'rings',
  'stats',
  'notices',
  'dialogs',
  'reasons',
  'rows',
];

test('the gallery renders every board and component section', async ({ page }) => {
  await open(page, '?gallery');
  await expect(page.getByTestId('gallery')).toBeVisible();
  for (const section of SECTIONS) {
    await expect(page.getByTestId(`gallery-${section}`)).toBeVisible();
  }
  // Contrast is computed from the live tokens: body text passes AA on white.
  await expect(page.getByTestId('swatch-text')).toContainText('AA');
});

test('score rings show their value; excluded and unscorable show no number', async ({ page }) => {
  await open(page, '?gallery');
  const high = page.getByTestId('ring-high-lg');
  await high.scrollIntoViewIfNeeded();
  await expect(high).toHaveText('91');
  await expect(high).toHaveAttribute(
    'aria-label',
    new RegExp(`Passung 91${String.fromCharCode(0x202f)}% · Hohe Passung`),
  );
  await expect(page.getByTestId('ring-excluded-lg')).toHaveText('');
  await expect(page.getByTestId('ring-unscorable-lg')).toHaveText('–');
});

test("a ring that waits: the reader's arc turns, the list's dashed track breathes", async ({
  page,
}) => {
  await open(page, '?gallery');
  const wait = (id: string): Promise<{ name: string; dashes: string }> =>
    page.getByTestId(id).evaluate((ring) => {
      const layer = ring.querySelector('.wait')!;
      return {
        name: getComputedStyle(layer).animationName,
        dashes: getComputedStyle(layer.querySelector('circle')!).strokeDasharray,
      };
    });
  expect((await wait('ring-pending-md')).name).toBe('spin');
  // In the list no spinner shape stands still: a dashed full track, breathing.
  const list = await wait('ring-pending-sm');
  expect(list.name).toBe('breathe');
  expect(list.dashes).toMatch(/^2\.5(px)?,? 2\.5(px)?$/);
});

test('the evidence of a reason is part of it: its wash and its click cover the line', async ({
  page,
}) => {
  await open(page, '?gallery');
  const reason = page.getByTestId('gallery-reasons').locator('button.reason').first();
  const evidence = reason.getByTestId('evidence');
  await evidence.scrollIntoViewIfNeeded();
  await expect(evidence).toContainText('Konzerncontrolling');
  // Under the words, on their axis, inside the reason's box.
  const row = (await reason.boundingBox())!;
  const line = (await evidence.boundingBox())!;
  const words = (await reason.locator('.head > .label').boundingBox())!;
  expect(line.y).toBeGreaterThan(words.y + words.height - 1);
  expect(Math.abs(line.x - words.x)).toBeLessThan(1);
  expect(line.y + line.height).toBeLessThanOrEqual(row.y + row.height);
  // Hovering the evidence washes the whole reason.
  await evidence.hover();
  await expect(reason).not.toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
});

test('to check has one colour: the chip and the reason show the same navy', async ({ page }) => {
  await open(page, '?gallery');
  const section = page.getByTestId('gallery-reasons');
  await section.scrollIntoViewIfNeeded();
  const chip = await section
    .locator('.chip.unknown .chip-icon')
    .evaluate((node) => getComputedStyle(node).color);
  const reason = await section
    .locator('.reason.check .icon')
    .first()
    .evaluate((node) => getComputedStyle(node).color);
  expect(chip).toBe(reason);
});

test('under reduced motion the rings jump to their value', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, '?gallery');
  const mid = page.getByTestId('ring-mid-lg');
  await mid.scrollIntoViewIfNeeded();
  await settle(page);
  await expect(mid).toHaveText('64', { timeout: 200 });
});

test('controls: toggle, segmented, password reveal, search clear, disclosure', async ({ page }) => {
  await open(page, '?gallery');
  const section = page.getByTestId('gallery-inputs');
  const toggle = section.getByRole('switch').first();
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');

  const facet = page.getByTestId('segmented-facet');
  await facet.getByRole('radio', { name: /Alle/ }).click();
  await expect(facet.getByRole('radio', { name: /Alle/ })).toHaveAttribute('aria-checked', 'true');

  const password = page.locator('#gallery-password');
  await expect(password).toHaveAttribute('type', 'password');
  await section.getByRole('button', { name: 'Passwort zeigen' }).click();
  await expect(password).toHaveAttribute('type', 'text');
  await expect(password).toHaveAttribute('spellcheck', 'false');

  const search = section.getByRole('textbox', { name: 'Jobs durchsuchen' }).first();
  await expect(search).toHaveValue('Controlling');
  await section.getByRole('button', { name: 'Suche leeren' }).click();
  await expect(search).toHaveValue('');

  const disclosure = page.getByTestId('disclosure').getByRole('button');
  await expect(disclosure).toHaveAttribute('aria-expanded', 'true');
  await disclosure.click();
  await expect(disclosure).toHaveAttribute('aria-expanded', 'false');
});

test('dialog: Esc cancels, a press that ends on the scrim keeps it open', async ({ page }) => {
  await open(page, '?gallery');
  await page.getByTestId('open-danger').click();
  const dialog = page.getByTestId('dialog-danger');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Abbrechen' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);

  await page.getByTestId('open-danger').click();
  await expect(dialog).toBeVisible();
  const box = (await dialog.boundingBox())!;
  await page.mouse.move(box.x + 20, box.y + 20);
  await page.mouse.down();
  await page.mouse.move(5, 5);
  await page.mouse.up();
  await expect(dialog).toBeVisible();
  await page.mouse.click(5, 5);
  await expect(dialog).toHaveCount(0);
});

test('job rows select on click and reorder without losing a row', async ({ page }) => {
  await open(page, '?gallery');
  const list = page.getByTestId('job-list');
  await list.scrollIntoViewIfNeeded();
  const rows = list.locator('[data-testid^="job-row-"]');
  await expect(rows).toHaveCount(6);
  await rows.nth(1).click();
  await expect(rows.nth(1)).toHaveAttribute('aria-current', 'true');
  const first = await rows.first().getAttribute('data-testid');
  await page.getByTestId('rows-shuffle').click();
  await expect(rows).toHaveCount(6);
  await expect(rows.last()).toHaveAttribute('data-testid', first!);
});

test('a quiet button that resets warns on hover, like the trash ghost', async ({ page }) => {
  await open(page, '?gallery');
  const reset = page.getByTestId('button-warns');
  await reset.scrollIntoViewIfNeeded();
  const colour = (): Promise<string> => reset.evaluate((node) => getComputedStyle(node).color);
  const rest = await colour();
  await reset.hover();
  await expect.poll(colour).not.toBe(rest);
  const danger = await page.evaluate(() => {
    const probe = document.createElement('span');
    probe.style.color = 'var(--danger-strong)';
    document.body.append(probe);
    const value = getComputedStyle(probe).color;
    probe.remove();
    return value;
  });
  await expect.poll(colour).toBe(danger);
});

test('the hairline under a row spans it, or insets where the list reaches past its column', async ({
  page,
}) => {
  await open(page, '?gallery');
  const list = page.getByTestId('job-list');
  await list.scrollIntoViewIfNeeded();
  const rule = (): Promise<{ left: string; right: string; height: string }> =>
    list
      .locator('.row')
      .first()
      .evaluate((row) => {
        const style = getComputedStyle(row, '::after');
        return { left: style.left, right: style.right, height: style.height };
      });
  expect(await rule()).toEqual({ left: '0px', right: '0px', height: '1px' });
  await list.evaluate((node) => node.style.setProperty('--row-rule-inset', 'var(--pane-padding)'));
  expect(await rule()).toEqual({ left: '16px', right: '16px', height: '1px' });
});

test('job rows: tools, status, aged date, provisional ring, no dot on excluded', async ({
  page,
}) => {
  await open(page, '?gallery&platform=windows');
  const list = page.getByTestId('job-list');
  await list.scrollIntoViewIfNeeded();
  const job = (id: string) =>
    list.locator('.job', { has: page.locator(`[data-testid="job-row-${id}"]`) });
  // The archive tool names its action (a click moves the job out, see the collapse test).
  await expect(page.getByTestId('archive-freelancermap-1001')).toHaveAttribute(
    'aria-label',
    'Archivieren',
  );
  // No stage badges: a favourite has only its star.
  await expect(job('linkedin-1002')).not.toContainText('Beworben');
  await expect(job('freelancermap-1001')).not.toContainText('Gemerkt');
  // Older than ten days: the date sits on a tint.
  await expect(job('freelancermap-1005').locator('.date')).toHaveClass(/old/);
  await expect(job('freelancermap-1001').locator('.date')).not.toHaveClass(/old/);
  // A score from a teaser is provisional (dashed); an excluded unread row has no dot.
  await expect(job('freelance-1003').locator('.ring')).toHaveClass(/provisional/);
  await expect(job('freelancermap-1006').locator('.dot')).toHaveCount(0);
});

test('a row: the date ends the title line, the tools take its place on hover', async ({ page }) => {
  await open(page, '?gallery&platform=windows');
  const list = page.getByTestId('job-list');
  await list.scrollIntoViewIfNeeded();
  const job = list.locator('.job', { has: page.getByTestId('job-row-freelancermap-1001') });
  const box = async (selector: string) => (await job.locator(selector).first().boundingBox())!;
  const title = await box('.title');
  const date = await box('.date');
  const mark = await box('.mark');
  // The date on the first title line, the small pinned star just left of it.
  expect(Math.abs(date.y + date.height / 2 - (title.y + 10))).toBeLessThan(2);
  expect(mark.x + mark.width).toBeLessThanOrEqual(date.x);
  expect(mark.width).toBe(16);
  // Company, place and facts use the full width, up to the date's right edge.
  const meta = await box('.meta');
  const foot = await box('.foot');
  expect(meta.x + meta.width).toBeGreaterThan(date.x + date.width - 1);
  expect(foot.x + foot.width).toBeGreaterThan(date.x + date.width - 1);
  // On hover the date and its star give way to the tools, which sit over them.
  const end = job.locator('.end');
  const tools = job.locator('.tools');
  await expect(end).toHaveCSS('opacity', '1');
  await job.hover({ position: { x: 120, y: 30 } });
  await expect(end).toHaveCSS('opacity', '0');
  await expect(tools.locator('.tool').last()).toHaveCSS('opacity', '1');
  const over = (await tools.boundingBox())!;
  expect(Math.abs(over.x + over.width - (date.x + date.width))).toBeLessThan(1);
  expect(Math.abs(over.y + over.height / 2 - (date.y + date.height / 2))).toBeLessThan(2);
  // The title never runs under them: its line keeps their room free.
  expect(title.x + title.width).toBeLessThanOrEqual(over.x);
  // From the keyboard: Tab from the row to its first tool shows the tools, too.
  await page.mouse.move(0, 0);
  await expect(end).toHaveCSS('opacity', '1');
  await page.getByTestId('job-row-freelancermap-1001').focus();
  await page.keyboard.press('Tab');
  await expect(page.getByTestId('archive-freelancermap-1001')).toBeFocused();
  await expect(end).toHaveCSS('opacity', '0');
  await expect(tools.locator('.tool').first()).toHaveCSS('opacity', '1');
});

test('the facts of a row drop out whole, a value is never cut', async ({ page }) => {
  await open(page, '?gallery&platform=windows');
  const list = page.getByTestId('job-list');
  await list.scrollIntoViewIfNeeded();
  const facts = page.getByTestId('job-row-freelancermap-1001').getByTestId('row-facts');
  const fit = (): Promise<{ shown: string[]; hidden: string[]; cut: string[] }> =>
    facts.evaluate((line) => {
      const edge = line.getBoundingClientRect();
      const out = { shown: [] as string[], hidden: [] as string[], cut: [] as string[] };
      for (const fact of line.querySelectorAll<HTMLElement>('.fact')) {
        const box = fact.getBoundingClientRect();
        if (box.top >= edge.bottom - 0.5) out.hidden.push(fact.textContent ?? '');
        else out.shown.push(fact.textContent ?? '');
        if (box.top < edge.bottom - 0.5 && box.right > edge.right + 0.5) out.cut.push('right');
        if (fact.scrollWidth > fact.clientWidth) out.cut.push(fact.textContent ?? '');
      }
      return out;
    });
  const wide = await fit();
  expect(wide.shown).toHaveLength(4);
  expect(wide.cut).toEqual([]);
  // A narrow list: the facts at the end drop out whole, in the order of their weight.
  await list.evaluate((node) => node.style.setProperty('width', '330px'));
  const narrow = await fit();
  expect(narrow.hidden.length).toBeGreaterThan(0);
  expect(narrow.shown[0]).toBe('ab sofort');
  expect(narrow.cut).toEqual([]);
});

test('a long row title takes two lines, the row grows by one line, the rest is a tooltip', async ({
  page,
}) => {
  await open(page, '?gallery&platform=windows');
  await page.getByTestId('job-list').scrollIntoViewIfNeeded();
  const long = page.getByTestId('job-row-freelancermap-1004');
  const short = page.getByTestId('job-row-freelancermap-1005');
  const title = long.locator('.title');
  const lines = await title.evaluate(
    (node) => node.clientHeight / parseFloat(getComputedStyle(node).lineHeight),
  );
  expect(Math.round(lines)).toBe(2);
  expect((await short.boundingBox())!.height).toBe(86);
  expect((await long.boundingBox())!.height).toBe(106);
  // Still cut off after two lines: the full title shows in a tooltip.
  await title.hover();
  await expect(page.getByRole('tooltip')).toContainText('vierzehn Ländern');
});

test('the column handle: left drag resizes within min and max, double click resets, it is kept', async ({
  page,
}) => {
  await open(page, '?gallery&platform=windows');
  const list = page.getByTestId('split-list');
  const handle = page.getByTestId('splitter').locator('.hit');
  await handle.scrollIntoViewIfNeeded();
  const width = async (): Promise<number> => Math.round((await list.boundingBox())!.width);
  expect(await width()).toBe(360);
  await expect(handle).toHaveCSS('cursor', 'col-resize');
  const drag = async (dx: number, button: 'left' | 'right' = 'left'): Promise<void> => {
    const box = (await handle.boundingBox())!;
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down({ button });
    await page.mouse.move(x + dx / 2, y, { steps: 3 });
    await page.mouse.move(x + dx, y, { steps: 3 });
    await page.mouse.up({ button });
  };
  await drag(60);
  await expect.poll(width).toBe(420);
  // Never past max (460) or min (360); the right button does nothing.
  await drag(200);
  await expect.poll(width).toBe(460);
  await drag(-40, 'right');
  await expect.poll(width).toBe(460);
  await drag(-400);
  await expect.poll(width).toBe(360);
  await drag(50);
  await expect.poll(width).toBe(410);
  // Kept across a reload; a double click sets it back.
  await page.reload();
  await handle.scrollIntoViewIfNeeded();
  await expect.poll(width).toBe(410);
  await handle.dblclick();
  await expect.poll(width).toBe(360);
  await page.reload();
  await handle.scrollIntoViewIfNeeded();
  await expect.poll(width).toBe(360);
});

test('nav sub-entries: quieter, indented, the one pill covers the active one (also in the rail)', async ({
  page,
}) => {
  await open(page, '?gallery&platform=windows');
  const section = page.getByTestId('gallery-navigation');
  await section.scrollIntoViewIfNeeded();
  for (const nav of [section.locator('nav').first(), section.locator('nav.collapsed')]) {
    for (const id of ['gnav-trash', 'gnav-archive', 'gnav-0']) {
      await nav.getByTestId(id).click();
      await expect(nav.getByTestId(id)).toHaveAttribute('aria-current', 'page');
      // The pill has slid onto the entry (it is exactly as high and at the same top).
      await expect
        .poll(async () => {
          const pill = (await nav.locator('.indicator').boundingBox())!;
          const entry = (await nav.getByTestId(id).boundingBox())!;
          return [Math.round(pill.y - entry.y), Math.round(pill.height - entry.height)];
        })
        .toEqual([0, 0]);
    }
  }
  // Collapsed, a sub-entry is an icon with its name as the accessible name (and tooltip).
  await expect(section.locator('nav.collapsed').getByTestId('gnav-trash')).toHaveAttribute(
    'aria-label',
    'Papierkorb',
  );
  // Expanded, it is indented under the parent's label and quieter (13 px).
  const [parent, sub] = await Promise.all(
    ['gnav-0', 'gnav-archive'].map((id) =>
      section.locator('nav').first().getByTestId(id).locator('.glyph').boundingBox(),
    ),
  );
  expect(sub!.x - parent!.x).toBeGreaterThan(20);
  await expect(section.locator('nav').first().getByTestId('gnav-archive')).toHaveCSS(
    'font-size',
    '13px',
  );
});

test('a menu button opens the OS menu of choices below it; a choice applies', async ({ page }) => {
  await open(page, '?gallery&platform=windows');
  const button = page.getByTestId('menu-order');
  await button.scrollIntoViewIfNeeded();
  await expect(button).toHaveText('Nach Passung');
  await expect(button).toHaveAttribute('aria-haspopup', 'menu');
  await button.click();
  const shown = await page.evaluate(() => ({
    menu: window.__harness.menus.at(-1),
    at: window.__harness.menuAt,
  }));
  expect(shown.menu!.map((entry) => [entry.text, entry.checked])).toEqual([
    ['Nach Passung', true],
    ['Nach Datum', false],
  ]);
  // Right below the button, on its left edge.
  const box = (await button.boundingBox())!;
  expect(Math.round(shown.at!.x)).toBe(Math.round(box.x));
  expect(shown.at!.y).toBeGreaterThanOrEqual(box.y + box.height);
  await page.evaluate(() => window.__harness.pick(1));
  await expect(button).toHaveText('Nach Datum');
  // Disabled: the tooltip says why, no menu opens.
  const count = await page.evaluate(() => window.__harness.menus.length);
  const off = page.getByTestId('menu-order-off');
  await off.click({ force: true });
  expect(await page.evaluate(() => window.__harness.menus.length)).toBe(count);
  await page.mouse.move(0, 0);
  await off.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Ohne Profil nur nach Datum.');
});

for (const [os, toggle] of [
  ['windows', 'Control'],
  ['macos', 'Meta'],
] as const) {
  test(`a mail app's selection on ${os}: toggle, range, the bar, Esc clears`, async ({ page }) => {
    await open(page, `?gallery&platform=${os}`);
    const list = page.getByTestId('job-list');
    await list.scrollIntoViewIfNeeded();
    const row = (id: string) => page.getByTestId(`job-row-freelancermap-${id}`);
    const bar = page.getByTestId('selection-bar');
    await expect(bar).toHaveCount(0);
    await page.getByTestId('job-row-linkedin-1002').click({ modifiers: [toggle] });
    await expect(page.getByTestId('selection-count')).toHaveText('2 ausgewählt');
    await expect(row('1001')).toHaveAttribute('aria-current', 'true');
    // Shift+click: the range from the last toggled row (1002) to 1005.
    await row('1005').click({ modifiers: ['Shift'] });
    await expect(page.getByTestId('selection-count')).toHaveText('4 ausgewählt');
    await expect(row('1001')).not.toHaveAttribute('aria-current', 'true');
    await expect(bar.getByTestId('bulk-archive')).toHaveAttribute('aria-label', 'Archivieren');
    // Esc (outside fields) clears the selection; the bar goes.
    await page.keyboard.press('Escape');
    await expect(bar).toHaveCount(0);
    // A plain click selects one job again.
    await row('1004').click();
    await expect(list.locator('[aria-current="true"]')).toHaveCount(1);
  });
}

test('moving jobs out: the row folds away, one toast merges them, one undo brings all back', async ({
  page,
}) => {
  await open(page, '?gallery&platform=windows');
  const list = page.getByTestId('job-list');
  await list.scrollIntoViewIfNeeded();
  const rows = list.locator('[data-testid^="job-row-"]');
  await expect(rows).toHaveCount(6);
  // The row folds away: its wrapper animates its height while the rows below follow.
  const folding = await page.evaluate(async () => {
    const button = document.querySelector<HTMLElement>(
      '[data-testid="archive-freelancermap-1004"]',
    )!;
    const wrapper = button.closest('.job')!.parentElement!;
    button.click();
    await new Promise((resolve) => requestAnimationFrame(resolve));
    return wrapper.getAnimations().length;
  });
  expect(folding).toBeGreaterThan(0);
  await expect(rows).toHaveCount(5);
  const toast = page.getByTestId('toast');
  await expect(toast).toContainText('„SAP FI Berater');
  // A second one within two seconds joins the same toast.
  await page.getByTestId('archive-freelancermap-1005').click({ force: true });
  await expect(toast).toHaveCount(1);
  await expect(toast).toContainText('2 Jobs archiviert.');
  await expect(rows).toHaveCount(4);
  // One undo brings both back, in their places.
  await toast.getByTestId('toast-action').click();
  await expect(rows).toHaveCount(6);
  await expect(rows.nth(3)).toHaveAttribute('data-testid', 'job-row-freelancermap-1004');
  await expect(rows.nth(4)).toHaveAttribute('data-testid', 'job-row-freelancermap-1005');
});

test('an undo toast stays 10 s; toasts wait while the window is in the back', async ({ page }) => {
  await open(page, '?gallery&platform=windows');
  const list = page.getByTestId('job-list');
  await list.scrollIntoViewIfNeeded();
  const toast = page.getByTestId('toast');
  await page.getByTestId('archive-freelance-1003').click();
  await expect(toast.locator('.life')).toHaveCSS('animation-duration', '10s');
  await page.mouse.move(5, 5);
  await page.waitForTimeout(5000);
  await expect(toast).toHaveCount(1);
  await toast.getByRole('button', { name: 'Ausblenden' }).click();
  // A plain toast (4 s) outlives its time while the window is in the back.
  await page.getByRole('button', { name: 'Toast zeigen' }).click();
  await expect(toast).toHaveCount(1);
  await expect(toast.locator('.life')).toHaveCSS('animation-duration', '4s');
  await page.evaluate(() => window.__harness.fire('tauri://blur', null));
  await page.waitForTimeout(4500);
  await expect(toast).toHaveCount(1);
  await expect(toast.locator('.life')).toHaveCSS('animation-play-state', 'paused');
  await page.evaluate(() => window.__harness.fire('tauri://focus', null));
  await expect(toast).toHaveCount(0, { timeout: 5000 });
});

test('a modal dialog dims the toasts, blocks their undo and keeps their time', async ({ page }) => {
  await open(page, '?gallery&platform=windows');
  await page.getByRole('button', { name: 'Toast zeigen' }).click();
  const toast = page.getByTestId('toast');
  await expect(toast).toHaveCount(1);
  await page.getByTestId('open-danger').click();
  await expect(page.getByTestId('dialog-danger')).toBeVisible();
  // The scrim lies over the toast: a click there reaches the scrim, not the toast.
  const box = (await toast.boundingBox())!;
  const hit = await page.evaluate(
    ({ x, y }) => document.elementFromPoint(x, y)?.closest('[data-testid="toast"]') !== null,
    { x: box.x + box.width / 2, y: box.y + box.height / 2 },
  );
  expect(hit).toBe(false);
  await page.waitForTimeout(4500);
  await expect(toast).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('dialog-danger')).toHaveCount(0);
  await expect(toast).toHaveCount(0, { timeout: 5000 });
});

test('a switch row toggles from its text; an empty tile is no filter', async ({ page }) => {
  await open(page, '?gallery');
  const toggle = page.getByTestId('gallery-row-toggle');
  await toggle.scrollIntoViewIfNeeded();
  const before = await toggle.getAttribute('aria-checked');
  await page.getByText('Ruft neue Alert-Mails ab', { exact: false }).click();
  await expect(toggle).not.toHaveAttribute('aria-checked', before!);
  // The tile with 0 is plain text; the chosen filter is pressed.
  expect(await page.getByTestId('tile-empty').evaluate((node) => node.tagName)).toBe('DIV');
  await expect(page.getByTestId('tile-filter')).toHaveAttribute('aria-pressed', 'true');
});

test('a segmented control never overlaps: each pill covers exactly its option', async ({
  page,
}) => {
  await open(page, '?gallery&platform=windows');
  for (const id of ['segmented-views', 'segmented-narrow']) {
    const control = page.getByTestId(id);
    await control.scrollIntoViewIfNeeded();
    for (const option of ['Neu', 'Alle', 'Gemerkt', 'Bewerbungen']) {
      await control.getByRole('radio', { name: new RegExp(option) }).click();
      const geometry = await control.evaluate((node) => {
        const box = node.getBoundingClientRect();
        const options = [...node.querySelectorAll('.option')].map((o) => o.getBoundingClientRect());
        const chosen = node.querySelector('[aria-checked="true"]')!;
        const pill = chosen.querySelector('.pill')!.getBoundingClientRect();
        const own = chosen.getBoundingClientRect();
        return {
          inside: options.every((o) => o.left >= box.left - 0.5 && o.right <= box.right + 0.5),
          apart: options.every((o, i) => i === 0 || o.left >= options[i - 1]!.right - 0.5),
          pill: [pill.left - own.left, pill.right - own.right].map((d) => Math.abs(d) < 0.5),
          overflow: node.scrollWidth - node.clientWidth,
        };
      });
      expect(geometry, `${id} ${option}`).toEqual({
        inside: true,
        apart: true,
        pill: [true, true],
        overflow: 0,
      });
    }
  }
});

test('baseline: gallery (reduced motion, so counters and loops are at rest)', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, '?gallery');
  const height = await page.getByTestId('gallery').evaluate((node) => node.scrollHeight);
  await page.setViewportSize({ width: 1360, height: Math.ceil(height) });
  await settle(page);
  // A full-page capture of the long gallery takes WebKit a few seconds per frame.
  test.setTimeout(90_000);
  await expectShot(page, 'gallery', { maxDiffPixelRatio: 0.004, timeout: 30_000 });
});
