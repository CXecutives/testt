// Fixes of the final audit round for Einstellungen, the first run, the mailbox form and the
// Profil page (track "settings").

import type { Page } from '@playwright/test';
import { calls, expect, open, test } from './fixtures';

const WIN = '?platform=windows';

async function settings(page: Page, query = WIN): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings')).toBeVisible();
}

test('a rescore locks the settings with its own reason, not a fetch', async ({ page }) => {
  await settings(page);
  // The sign-in row exists only with "Mit Anmeldung" on.
  await page.getByTestId('toggle-login-freelance').click();
  await expect(page.getByTestId('sign-in-freelance')).toBeVisible();
  // The profile changed: the app scores every job anew by itself and holds the settings.
  await page.evaluate(() => {
    window.__harness.holdAfter = 2;
    window.__harness.appRun('rescore');
  });
  for (const id of [
    'mailbox-change',
    'txt-rewrite',
    'full-mailbox',
    'reset',
    'sign-in-freelance',
  ]) {
    const button = page.getByTestId(id);
    await expect(button).toHaveAttribute('aria-disabled', 'true');
    await button.hover();
    await expect(page.getByRole('tooltip')).toHaveText('Die Jobs werden gerade neu bewertet.');
  }
  await page.evaluate(() => (window.__harness.holdAfter = null));
});

test('one glyph per file and per action: Excel, Ändern, and a reset that warns', async ({
  page,
}) => {
  await settings(page);
  const glyph = (id: string) => page.getByTestId(id).locator('[data-icon]');
  // The Excel file has the spreadsheet glyph, as in the day overview ("Excel öffnen").
  await expect(glyph('excel-open')).toHaveAttribute('data-icon', 'file-spreadsheet');
  // A stored value changes with the pencil (the mailbox, the work folder).
  await expect(glyph('mailbox-change')).toHaveAttribute('data-icon', 'pencil');
  await expect(glyph('workspace-change')).toHaveAttribute('data-icon', 'pencil');
  // "Zurücksetzen" warns on hover like every milder button that removes something.
  const danger = await page.evaluate(() => {
    const probe = document.body.appendChild(document.createElement('span'));
    probe.style.color = 'var(--danger-strong)';
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  });
  for (const id of ['mailbox-remove', 'reset']) {
    const button = page.getByTestId(id);
    await expect(button).not.toHaveCSS('color', danger);
    await button.hover();
    await expect(button).toHaveCSS('color', danger);
  }
});

test('an unreachable Gmail is one red badge, said once', async ({ page }) => {
  await settings(page, `${WIN}&scenario=offline`);
  const mailbox = page.getByTestId('settings-mailbox');
  // The same tone as the sidebar's "Fehlgeschlagen" and the overview's failed fetch.
  const badge = mailbox.locator('.badge').filter({ hasText: 'Nicht erreichbar' });
  await expect(badge).toHaveClass(/danger/);
  // "Gmail ist nicht erreichbar." would only repeat the badge.
  await expect(mailbox).not.toContainText('Gmail ist nicht erreichbar.');
  await expect(page.getByTestId('mailbox-failure')).toHaveCount(0);
});

test('Ändern puts the caret in the form; closing it gives the focus back', async ({ page }) => {
  await settings(page);
  const change = page.getByTestId('mailbox-change');
  const password = page.getByTestId('mailbox-password');
  // The address stays, so the caret waits in the app password.
  await change.focus();
  await page.keyboard.press('Enter');
  await expect(password).toBeFocused();
  await expect(page.getByTestId('mailbox-user')).toHaveValue('alerts.demo@gmail.com');
  // Esc closes the form, and the focus is back on Ändern, like a dialog's on its opener.
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('mailbox-form')).toHaveCount(0);
  await expect(change).toBeFocused();
  // Abbrechen does the same, and so does a save.
  await page.keyboard.press('Enter');
  await expect(password).toBeFocused();
  await page.getByTestId('mailbox-cancel').focus();
  await page.keyboard.press('Enter');
  await expect(change).toBeFocused();
  await change.click();
  await expect(password).toBeFocused();
  await password.fill('abcd efgh ijkl mnop');
  await password.press('Enter');
  await expect(page.getByTestId('mailbox-form')).toHaveCount(0);
  await expect(change).toBeFocused();
});

test('the changed mailbox: save and cancel end on the edge like every control', async ({
  page,
}) => {
  for (const query of [WIN, '?platform=macos']) {
    await settings(page, query);
    await page.getByTestId('mailbox-change').click();
    const place = await page.evaluate(() => {
      const box = (selector: string): DOMRect =>
        document.querySelector(selector)!.getBoundingClientRect();
      const buttons = [
        ...document.querySelectorAll('[data-testid="mailbox-form"] .actions .btn'),
      ].map((node) => node.getBoundingClientRect());
      return {
        end: buttons.at(-1)!.right,
        gap: buttons[1]!.left - buttons[0]!.right,
        // A switch of the next card ends on the content edge of the column.
        edge: box('[data-testid="toggle-auto-fetch"]').right,
        fields: box('[data-testid="mailbox-form"] .fields').width,
      };
    });
    expect(Math.abs(place.end - place.edge)).toBeLessThanOrEqual(1);
    expect(place.gap).toBe(12);
    // The fields keep the measure of a form.
    expect(place.fields).toBeLessThanOrEqual(560);
  }
});

test('notes and errors in Einstellungen follow a switch of the language', async ({ page }) => {
  await settings(page);
  // A note of an action, a refused switch and the errors of the open mailbox form.
  await page.getByTestId('txt-rewrite').click();
  await expect(page.getByTestId('files-note')).toHaveText('38 Dateien geschrieben.');
  await page.getByTestId('toggle-enabled-linkedin').click();
  await page.getByTestId('toggle-enabled-freelance').click();
  await page.getByTestId('toggle-enabled-freelancermap').click();
  const refused = page.getByTestId('portal-freelancermap').getByTestId('portal-error');
  await expect(refused).toHaveText('Mindestens ein Portal muss aktiv sein.');
  // One portal on again, so the language can be saved.
  await page.getByTestId('toggle-enabled-linkedin').click();
  await expect(page.getByTestId('toggle-enabled-linkedin')).toHaveAttribute('aria-checked', 'true');
  await page.getByTestId('mailbox-change').click();
  await page.getByTestId('mailbox-user').fill('');
  await page.getByTestId('mailbox-save').click();
  const form = page.getByTestId('mailbox-form');
  await expect(form).toContainText('Die Gmail-Adresse fehlt.');

  await page.getByTestId('language').getByRole('radio', { name: 'English' }).click();
  await expect(page.getByTestId('settings-files')).toContainText('Files');
  await expect(page.getByTestId('files-note')).toHaveText('38 files written.');
  await expect(refused).toHaveText('At least one portal must be active.');
  await expect(form).toContainText('The Gmail address is missing.');
  await expect(form).toContainText('The app password is missing.');
});

test('details off: no page counts, and only the alert mails keep their problem', async ({
  page,
}) => {
  await settings(page);
  const details = page.getByTestId('toggle-details-freelancermap');
  await expect(page.getByTestId('quota-freelancermap')).toContainText('Heute 86 von 100 Seiten');
  await details.click();
  await expect(details).toHaveAttribute('aria-checked', 'false');
  await expect(page.getByTestId('quota-freelancermap')).toHaveCount(0);
  await expect(page.getByTestId('status-freelancermap')).toHaveCount(0);

  await settings(page, `${WIN}&scenario=paused`);
  // A pause concerns the pages: it goes with the details.
  await expect(page.getByTestId('health-linkedin')).toBeVisible();
  await page.getByTestId('toggle-details-linkedin').click();
  await expect(page.getByTestId('health-linkedin')).toHaveCount(0);
  // Alert mails without jobs come from Gmail: they stay.
  await page.getByTestId('toggle-details-freelance').click();
  await expect(page.getByTestId('toggle-details-freelance')).toHaveAttribute(
    'aria-checked',
    'false',
  );
  await expect(page.getByTestId('health-freelance')).toBeVisible();
});

test('alert mails without jobs: the portal card opens one, like the day overview', async ({
  page,
}) => {
  await settings(page, `${WIN}&scenario=paused`);
  await page
    .getByTestId('health-freelance')
    .getByRole('button', { name: 'Alert-Mail öffnen' })
    .click();
  expect((await calls(page, 'open_target')).at(-1)?.[1]).toEqual({
    target: { kind: 'alertMail', gmailId: '18c2f0a9d1e4b7a3' },
  });
  // A pause has no mail to open.
  await expect(page.getByTestId('health-linkedin').getByRole('button')).toHaveCount(0);
});

test('the first run opens at its top, the caret waiting in the address', async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 360 });
  for (const query of [`${WIN}&scenario=reset&lang=en`, '?platform=macos&scenario=first-run']) {
    await open(page, query);
    const user = page.getByTestId('mailbox-user');
    await expect(user).toBeFocused();
    // The mark and the title are in view, and after a reset its report (WebKit scrolled to a
    // field focused too early a moment later: wait for that moment).
    await page.waitForTimeout(200);
    const top = await page
      .getByTestId('first-run')
      .evaluate((node) => node.closest('.view')?.scrollTop ?? -1);
    expect(top).toBe(0);
    if (query.includes('reset')) {
      await expect(page.getByTestId('first-reset-report')).toBeInViewport({ ratio: 1 });
    }
    // Typing brings the field into view.
    await page.keyboard.type('alerts');
    await expect(user).toHaveValue('alerts');
    await expect(user).toBeInViewport();
  }
});
