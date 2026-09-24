import { expect, expectShot, open, test } from './fixtures';

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
  await expect(page.getByTestId('nav-jobs')).toContainText('6');
  await expect(page.getByTestId('view-jobs')).toBeVisible();
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

// The window frame is the native one of each OS (icon, title, caption buttons, system menu,
// snap layouts): the page draws none of it and has no drag region of its own.
for (const os of ['windows', 'macos']) {
  test(`${os}: no title bar in the page, the content starts at the top`, async ({ page }) => {
    await open(page, `?platform=${os}`);
    await expect(page.locator('html')).toHaveAttribute('data-platform', os);
    await expect(page.locator('[data-tauri-drag-region]')).toHaveCount(0);
    await expect(page.getByTestId('titlebar')).toHaveCount(0);
    expect((await page.getByTestId('sidebar').boundingBox())!.y).toBe(0);
  });

  test(`${os}: the first view sits on the line of the search field`, async ({ page }) => {
    await open(page, `?platform=${os}`);
    // The field's frame is the input's parent (the input sits inside its border).
    const nav = (await page.getByTestId('nav-jobs').boundingBox())!.y;
    const field = (await page.getByTestId('search').locator('xpath=..').boundingBox())!.y;
    expect(nav).toBe(field);
  });
}

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
  expect(await order('windows')).toEqual(['Postfach lesen', 'Abbrechen']);
  expect(await order('macos')).toEqual(['Abbrechen', 'Postfach lesen']);
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
  const sort = page.getByTestId('sort');
  await sort.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Beste Passung zuerst');
  await expect(sort).not.toHaveAttribute('title');
});

test('baseline: shell on Windows', async ({ page }) => {
  await open(page, '?platform=windows');
  await expectShot(page, 'shell-windows');
});

test('baseline: shell on macOS', async ({ page }) => {
  await open(page, '?platform=macos');
  await expectShot(page, 'shell-macos');
});
