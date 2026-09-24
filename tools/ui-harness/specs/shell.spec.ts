import { expect, expectShot, open, test } from './fixtures';

test('the preview server sends the production CSP', async ({ page }) => {
  const response = await page.goto('/');
  const csp = response?.headers()['content-security-policy'] ?? '';
  expect(csp).toContain("script-src 'self'");
  expect(csp).toContain("style-src 'self'");
});

test('the shell renders sidebar, top strip and the jobs view', async ({ page }) => {
  await open(page, '?platform=windows');
  await expect(page.getByTestId('shell')).toBeVisible();
  await expect(page.getByTestId('sidebar')).toBeVisible();
  await expect(page.getByTestId('titlebar')).toBeVisible();
  for (const item of ['nav-jobs', 'nav-profile', 'nav-settings']) {
    await expect(page.getByTestId(item)).toBeVisible();
  }
  await expect(page.getByTestId('nav-jobs')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('nav-jobs')).toContainText('6');
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await expect(page.getByTestId('window-controls')).toBeVisible();
  await expect(page.getByTestId('fetch')).toHaveClass(/primary/);
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

test('the Windows title bar: full width, 30 px, icon and name, drag region', async ({ page }) => {
  await open(page, '?platform=windows');
  const bar = page.getByTestId('titlebar');
  // The whole bar drags (Tauri's `deep`: every child but the buttons); nothing else does.
  await expect(bar).toHaveAttribute('data-tauri-drag-region', 'deep');
  await expect(page.getByTestId('sidebar')).not.toHaveAttribute('data-tauri-drag-region');
  await expect(page.getByTestId('fetch')).not.toHaveAttribute('data-tauri-drag-region');
  // Like a native Windows 11 title bar at 100 %: across the window above sidebar and
  // content, 30 px high, the 16 px icon 8 px from the edge, the name in 12 px regular.
  const box = (await bar.boundingBox())!;
  expect(box).toMatchObject({ x: 0, y: 0, width: 1360, height: 30 });
  expect((await page.getByTestId('sidebar').boundingBox())!.y).toBe(30);
  const icon = (await page.getByTestId('brand').locator('img').boundingBox())!;
  expect(icon).toMatchObject({ x: 8, y: 7, width: 16, height: 16 });
  const name = await page
    .getByTestId('brand')
    .getByText('Job-Alert-Monitor')
    .evaluate((node) => {
      const style = getComputedStyle(node);
      return [style.fontSize, style.fontWeight];
    });
  expect(name).toEqual(['12px', '400']);
  // Caption buttons at the native size: 46 x 30, flush with the right edge.
  const buttons = await page
    .getByTestId('window-controls')
    .getByRole('button')
    .evaluateAll((els) => els.map((el) => el.getBoundingClientRect().toJSON()));
  expect(buttons.map((b) => [b.width, b.height])).toEqual([
    [46, 30],
    [46, 30],
    [46, 30],
  ]);
  expect(buttons.at(-1)!.right).toBe(1360);
});

test('Windows caption buttons call the window API', async ({ page }) => {
  await open(page, '?platform=windows');
  const controls = page.getByTestId('window-controls').getByRole('button');
  await expect(controls).toHaveCount(3);
  await controls.nth(1).click();
  await expect(controls.nth(1)).toHaveAttribute('aria-label', 'Verkleinern');
  const calls = await page.evaluate(() => window.__harness.calls.map(([name]) => name));
  expect(calls).toContain('window.toggleMaximize');
});

test('macOS: the native title bar, no own one; the content starts at the top', async ({ page }) => {
  await open(page, '?platform=macos');
  await expect(page.locator('html')).toHaveAttribute('data-platform', 'macos');
  await expect(page.getByTestId('titlebar')).toHaveCount(0);
  await expect(page.getByTestId('window-controls')).toHaveCount(0);
  await expect(page.getByTestId('brand')).toHaveCount(0);
  expect((await page.getByTestId('sidebar').boundingBox())!.y).toBe(0);
});

for (const os of ['windows', 'macos']) {
  test(`${os}: the first view sits on the line of the search field`, async ({ page }) => {
    await open(page, `?platform=${os}`);
    // The field's frame is the input's parent (the input sits inside its border).
    const nav = (await page.getByTestId('nav-jobs').boundingBox())!.y;
    const field = (await page.getByTestId('search').locator('xpath=..').boundingBox())!.y;
    expect(nav).toBe(field);
  });
}

test('the run status in the sidebar opens the last run', async ({ page }) => {
  await open(page, '?platform=windows');
  await expect(page.getByTestId('run-card')).toHaveCount(0);
  await expect(page.getByTestId('run-status')).toContainText('Zuletzt 08:30');
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('run-status').click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await expect(page.getByTestId('run-finished')).toContainText('7 neue Jobs');
  await page.getByTestId('run-toggle').click();
  await expect(page.getByTestId('last-new')).toHaveCount(0);
  await page.getByTestId('run-close').click();
  await expect(page.getByTestId('run-card')).toHaveCount(0);
});

test('caption buttons: no tooltip, not in the tab order (like the native ones)', async ({
  page,
}) => {
  await open(page, '?platform=windows');
  const close = page.getByTestId('window-controls').getByRole('button').last();
  await close.hover();
  await page.waitForTimeout(700);
  await expect(page.getByRole('tooltip')).toHaveCount(0);
  await expect(close).not.toHaveAttribute('title');
  await expect(close).toHaveAttribute('tabindex', '-1');
  await expect(close).toHaveAttribute('aria-label', 'Schließen');
});

test('icon-only buttons show a styled tooltip after the delay', async ({ page }) => {
  await open(page, '?platform=windows');
  const sort = page.getByTestId('sort');
  await sort.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Beste Passung zuerst');
  await expect(sort).not.toHaveAttribute('title');
});

test('the title bar fades while the window is inactive', async ({ page }) => {
  await open(page, '?platform=windows');
  const bar = page.getByTestId('titlebar');
  await expect(bar).not.toHaveClass(/inactive/);
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect(bar).toHaveClass(/inactive/);
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect(bar).not.toHaveClass(/inactive/);
});

test('baseline: shell on Windows', async ({ page }) => {
  await open(page, '?platform=windows');
  await expectShot(page, 'shell-windows');
});

test('baseline: shell on macOS', async ({ page }) => {
  await open(page, '?platform=macos');
  await expectShot(page, 'shell-macos');
});
