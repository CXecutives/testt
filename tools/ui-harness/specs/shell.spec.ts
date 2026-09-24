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

test('strip and sidebar carry the drag region, the controls do not', async ({ page }) => {
  await open(page, '?platform=windows');
  await expect(page.getByTestId('titlebar')).toHaveAttribute('data-tauri-drag-region', '');
  await expect(page.getByTestId('sidebar')).toHaveAttribute('data-tauri-drag-region', '');
  await expect(page.getByTestId('nav-jobs')).not.toHaveAttribute('data-tauri-drag-region');
  await expect(page.getByTestId('fetch')).not.toHaveAttribute('data-tauri-drag-region');
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

test('macOS: no caption buttons, the brand sits below the traffic lights', async ({ page }) => {
  await open(page, '?platform=macos');
  await expect(page.locator('html')).toHaveAttribute('data-platform', 'macos');
  await expect(page.getByTestId('window-controls')).toHaveCount(0);
  const top = await page.getByTestId('brand').evaluate((node) => node.getBoundingClientRect().top);
  expect(top).toBeGreaterThanOrEqual(40);
});

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
