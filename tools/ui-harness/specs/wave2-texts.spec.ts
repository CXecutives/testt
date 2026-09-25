// Wave 2, texts: one word per thing and the words of the form (see docs/PLAN.md, glossary).

import type { Page } from '@playwright/test';
import { calls, expect, open, test } from './fixtures';

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

test('Einstellungen lists every file the app writes, the overview too', async ({ page }) => {
  await open(page, WIN);
  await page.getByTestId('nav-settings').click();
  const files = page.getByTestId('settings-files');
  // The same files as the day overview's "Dateien": Excel-Datei, Übersicht, then text files.
  const overview = files.getByTestId('overview');
  await expect(overview).toContainText('Übersicht');
  await overview.getByRole('button', { name: 'Öffnen' }).click();
  expect((await calls(page, 'open_target')).at(-1)?.[1]).toEqual({
    target: { kind: 'overview' },
  });
});
