// Fixes of the backend-core track as the page sees them through the stub, which mirrors the
// backend's contract: a profile that no longer reads leaves no scores behind.

import { expect, open, test } from './fixtures';

const WIN = '?platform=windows';

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
