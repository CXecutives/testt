import { expect, expectShot, open, test } from './fixtures';

test('the preview server sends the production CSP', async ({ page }) => {
  const response = await page.goto('/');
  const csp = response?.headers()['content-security-policy'] ?? '';
  expect(csp).toContain("script-src 'self'");
  expect(csp).toContain("style-src 'self'");
});

test('the shell renders title bar, tabs and the jobs view', async ({ page }) => {
  await open(page, '?platform=windows');
  await expect(page.getByTestId('shell')).toBeVisible();
  await expect(page.getByTestId('titlebar')).toBeVisible();
  for (const tab of ['tab-jobs', 'tab-profile', 'tab-settings']) {
    await expect(page.getByTestId(tab)).toBeVisible();
  }
  await expect(page.getByTestId('tab-jobs')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await expect(page.getByTestId('window-controls')).toBeVisible();
});

test('tabs switch the view', async ({ page }) => {
  await open(page, '?platform=windows');
  await page.getByTestId('tab-profile').click();
  await expect(page.getByTestId('view-profile')).toBeVisible();
  await expect(page.getByTestId('view-jobs')).toHaveCount(0);
  await expect(page.getByTestId('tab-profile')).toHaveAttribute('aria-selected', 'true');
  await page.getByTestId('tab-settings').click();
  await expect(page.getByTestId('view-settings')).toBeVisible();
  await expect(page.getByTestId('view-profile')).toHaveCount(0);
  await page.getByTestId('tab-jobs').click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
});

test('the title bar carries the drag region, the tab buttons do not', async ({ page }) => {
  await open(page, '?platform=windows');
  await expect(page.getByTestId('titlebar')).toHaveAttribute('data-tauri-drag-region', '');
  await expect(page.getByTestId('tab-jobs')).not.toHaveAttribute('data-tauri-drag-region');
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

test('macOS shows no caption buttons and leaves room for the traffic lights', async ({ page }) => {
  await open(page, '?platform=macos');
  await expect(page.locator('html')).toHaveAttribute('data-platform', 'macos');
  await expect(page.getByTestId('window-controls')).toHaveCount(0);
  const left = await page
    .getByTestId('brand')
    .evaluate((node) => node.getBoundingClientRect().left);
  expect(left).toBeGreaterThanOrEqual(80);
});

test('icon-only buttons show a styled tooltip after the delay', async ({ page }) => {
  await open(page, '?platform=windows');
  const close = page.getByTestId('window-controls').getByRole('button').last();
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
