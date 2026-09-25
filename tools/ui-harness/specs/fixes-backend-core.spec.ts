// Fixes of the backend-core track as the page sees them through the stub, which mirrors the
// backend's contract: a profile that no longer reads leaves no scores behind, and every
// portal may be switched off, while a fetch then waits for one, like it waits for a mailbox
// (the backend refuses it too, never with a failed fetch).

import { calls, expect, open, test } from './fixtures';

const WIN = '?platform=windows';

test('every portal may be switched off; Abrufen then waits for one and says why', async ({
  page,
}) => {
  await open(page, WIN);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings')).toBeVisible();
  for (const portal of ['linkedin', 'freelance', 'freelancermap']) {
    await page.getByTestId(`toggle-enabled-${portal}`).click();
    await expect(page.getByTestId(`toggle-enabled-${portal}`)).toHaveAttribute(
      'aria-checked',
      'false',
    );
  }
  // Saved as chosen: no refusal after the switches moved.
  await expect(page.getByTestId('portal-error')).toHaveCount(0);
  expect(await calls(page, 'save_settings')).toHaveLength(3);
  // Reading the whole mailbox waits for a portal too.
  const whole = page.getByTestId('full-mailbox');
  await expect(whole).toHaveAttribute('aria-disabled', 'true');
  await whole.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Schalte erst ein Portal ein.');

  await page.getByTestId('nav-jobs').click();
  const fetch = page.getByTestId('fetch');
  await expect(fetch).toHaveAttribute('aria-disabled', 'true');
  await expect(fetch).not.toHaveClass(/primary/);
  await fetch.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Schalte erst ein Portal ein.');
  await fetch.click({ force: true });
  expect(await calls(page, 'start_run')).toHaveLength(0);
  // No run began, so no failed fetch and no refusal.
  await expect(page.getByTestId('start-error')).toHaveCount(0);
  await expect(page.getByText('Abruf fehlgeschlagen')).toHaveCount(0);

  // One portal back on: Abrufen is the primary action again.
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('toggle-enabled-freelancermap').click();
  await page.getByTestId('nav-jobs').click();
  await expect(fetch).not.toHaveAttribute('aria-disabled', 'true');
  await expect(fetch).toHaveClass(/primary/);
});

test('a profile that no longer reads leaves no verdicts in the list', async ({ page }) => {
  await open(page, `${WIN}&scenario=profile-broken`);
  await expect(page.getByTestId('no-profile')).toContainText(
    'Die Jobs zeigen deshalb keine Passung.',
  );
  await expect(
    page.getByTestId('job-rows').locator('[data-testid^="job-row-"]').first(),
  ).toBeVisible();
  // Nothing is excluded or scored by a profile that is not there.
  await expect(page.getByTestId('excluded-divider')).toHaveCount(0);
});
