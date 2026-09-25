// Fixes of the backend-core track as the page sees them through the stub, which mirrors the
// backend's contract: a profile that no longer reads leaves no scores behind, and every
// portal may be switched off, while a fetch then is refused instead of failing.

import { calls, expect, open, test } from './fixtures';

const WIN = '?platform=windows';

test('every portal may be switched off; a fetch is then refused, never a failed one', async ({
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
  await page.getByTestId('nav-jobs').click();
  await page.getByTestId('fetch').click();
  await expect(page.getByTestId('start-error')).toContainText(
    'Mindestens ein Portal muss aktiv sein.',
  );
  // No run began, so no failed fetch is stored: the card keeps the last one.
  await expect(page.getByText('Abruf fehlgeschlagen')).toHaveCount(0);
  await expect(page.getByTestId('run-finished')).toContainText('Abruf fertig');
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
