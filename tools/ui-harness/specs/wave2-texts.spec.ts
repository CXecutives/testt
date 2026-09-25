// Wave 2, texts: one word per thing and the words of the form (see docs/PLAN.md, glossary).

import type { Page } from '@playwright/test';
import { expect, open, test } from './fixtures';

const WIN = '?platform=windows';
const row = (page: Page, key: string) => page.getByTestId('job-list').getByTestId(`job-row-${key}`);

test('the English reader says must-have in the badge and in the line above it', async ({
  page,
}) => {
  await open(page, `${WIN}&lang=en`);
  await row(page, 'freelancermap-2802').click();
  const stage = page.getByTestId('stage');
  await expect(stage.getByTestId('must')).toContainText('must-have requirements met');
  await expect(stage.getByText('Must-have', { exact: true })).toBeVisible();
  await expect(stage.getByText('Required', { exact: true })).toHaveCount(0);
});
