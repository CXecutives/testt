// Einstellungen and the first-run page against the stub.

import type { Page } from '@playwright/test';
import { calls, expect, expectShot, open, test, visibleCount } from './fixtures';

const WIN = '?platform=windows';

async function settings(page: Page, query = WIN): Promise<void> {
  await open(page, query);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings')).toBeVisible();
}

test('first run: three steps that tick themselves, fetch locked until a mailbox', async ({
  page,
}) => {
  await open(page, `${WIN}&scenario=first-run`);
  await expect(page.getByTestId('first-run')).toBeVisible();
  expect(await visibleCount(page, '.btn.primary')).toBe(1);
  // The caret waits in the first field; the setup is no view, so no nav entry is current.
  await expect(page.getByTestId('mailbox-user')).toBeFocused();
  await expect(page.getByTestId('nav-jobs')).not.toHaveAttribute('aria-current', 'page');

  // Where the jobs come from, and what an app password needs, before anything is typed:
  // one line under both fields, the two pages in the order she needs them.
  const links = await page
    .getByTestId('mailbox-form')
    .locator('.help .btn')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-testid')));
  expect(links).toEqual(['two-step', 'create-password']);
  // Empty fields are said at once, both, without asking Gmail.
  await page.getByTestId('mailbox-save').click();
  await expect(page.getByTestId('mailbox-form')).toContainText('Die Gmail-Adresse fehlt.');
  await expect(page.getByTestId('mailbox-form')).toContainText('Das App-Passwort fehlt.');
  await expect(page.getByTestId('mailbox-user')).toBeFocused();
  expect(await calls(page, 'save_mailbox')).toHaveLength(0);
  await expect(page.getByTestId('step-mailbox')).toContainText(
    'An diese Gmail-Adresse müssen die Alert-Mails der Portale gehen.',
  );
  await page.getByTestId('two-step').click();
  expect((await calls(page, 'open_target')).at(-1)?.[1]).toEqual({
    target: { kind: 'twoStepPage' },
  });

  const fetch = page.getByTestId('first-fetch');
  await expect(fetch).toHaveAttribute('aria-disabled', 'true');
  await fetch.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Erst ein Postfach verbinden.');

  await page.getByTestId('mailbox-user').fill('alerts.demo');
  await page.getByTestId('mailbox-password').fill('abcdabcdabcdabcd');
  await page.getByTestId('mailbox-save').click();
  await expect(page.getByTestId('mailbox-form')).toContainText('Die Adresse ist unvollständig.');
  await page.getByTestId('mailbox-user').fill('alerts.demo@gmail.com');
  await page.getByTestId('mailbox-password').fill('kurz');
  await page.getByTestId('mailbox-password').press('Enter');
  await expect(page.getByTestId('mailbox-form')).toContainText('16 Buchstaben');
  await expect(page.getByTestId('step-mailbox')).toHaveAttribute('aria-current', 'step');
  await page.getByTestId('mailbox-password').fill('abcd efgh ijkl mnop');
  await page.getByTestId('mailbox-password').press('Enter');
  await expect(page.getByTestId('step-mailbox')).toHaveAttribute('data-done', 'true');
  // The stepper moves on: the profile is the current step now, the ticked check draws.
  await expect(page.getByTestId('step-profile')).toHaveAttribute('aria-current', 'step');
  await expect(page.getByTestId('step-mailbox').locator('.marker')).toHaveClass(/drawn/);
  await expect(page.getByTestId('step-mailbox')).not.toHaveAttribute('aria-current', 'step');
  await expect(fetch).not.toHaveAttribute('aria-disabled', 'true');
  // The connect form is gone; the focus waits on the next step's action.
  await expect(page.getByTestId('first-profile')).toBeFocused();

  // "Profil anlegen" opens the empty form in one click; after saving, a button leads on to
  // the first fetch and the step is done with the person's name.
  await page.getByTestId('first-profile').click();
  await expect(page.getByTestId('profile-form')).toBeVisible();
  await expect(page.getByTestId('profile-name')).toHaveText('Neues Profil');
  await page.getByTestId('profile-name-field').fill('Erika Beispiel');
  await page.getByTestId('competence-name').fill('Controlling');
  await page.getByTestId('profile-save').click();
  await expect(page.getByTestId('profile-name')).toHaveText('Erika Beispiel');
  // The way on is where the user just saved (the bottom bar), once.
  const next = page.getByTestId('profile-next');
  await expect(next).toHaveCount(1);
  await expect(next).toHaveText('Weiter zum ersten Abruf');
  await expect(page.getByTestId('profile-understood')).not.toContainText('SAP');
  await expect(next).toBeInViewport();
  await next.click();
  await expect(page.getByTestId('step-profile')).toHaveAttribute('data-done', 'true');
  await expect(page.getByTestId('step-profile')).toContainText('Erika Beispiel');
  expect(await visibleCount(page, '.btn.primary')).toBe(1);

  await fetch.click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  await expect(page.getByTestId('run-running')).toBeVisible();
});

test('first run: a step done before the page opened is simply there', async ({ page }) => {
  await open(page, `${WIN}&scenario=mailbox-only`);
  const mailbox = page.getByTestId('step-mailbox');
  await expect(mailbox).toHaveAttribute('data-done', 'true');
  await expect(mailbox.locator('.marker')).not.toHaveClass(/drawn/);
  await expect(page.getByTestId('step-profile')).toHaveAttribute('aria-current', 'step');
  expect(await page.evaluate(() => document.getAnimations().length)).toBe(0);
});

test('settings: sections, no primary while nothing asks for one', async ({ page }) => {
  await settings(page);
  for (const id of ['mailbox', 'fetch', 'portals', 'files', 'care', 'reset']) {
    await expect(page.getByTestId(`settings-${id}`)).toBeVisible();
  }
  // "Alles zurücksetzen" stands alone, not between the harmless rows.
  await expect(page.getByTestId('settings-care').getByTestId('reset')).toHaveCount(0);
  await expect(page.getByTestId('settings-reset').getByTestId('reset')).toBeVisible();
  // "Abrufen" lives in the list of the Jobs view: nothing here asks for a primary.
  expect(await visibleCount(page, '.btn.primary')).toBe(0);
  await expect(page.getByTestId('settings-mailbox')).toContainText('alerts.demo@gmail.com');
});

test('changing the mailbox: form with save and cancel, Esc cancels', async ({ page }) => {
  await settings(page);
  await page.getByTestId('mailbox-change').click();
  await expect(page.getByTestId('mailbox-form')).toBeVisible();
  expect(await visibleCount(page, '.btn.primary')).toBe(1);
  await expect(page.getByTestId('mailbox-save')).toHaveClass(/primary/);
  await page.getByTestId('mailbox-password').press('Escape');
  await expect(page.getByTestId('mailbox-form')).toHaveCount(0);
});

test('the mailbox form orders save and cancel like the dialogs of the OS', async ({ page }) => {
  const order = async (): Promise<(string | null)[]> =>
    page
      .getByTestId('mailbox-form')
      .locator('.actions .btn')
      .evaluateAll((buttons) => buttons.map((button) => button.getAttribute('data-testid')));
  await settings(page);
  await page.getByTestId('mailbox-change').click();
  expect(await order()).toEqual(['mailbox-save', 'mailbox-cancel']);
  await settings(page, '?platform=macos');
  await page.getByTestId('mailbox-change').click();
  expect(await order()).toEqual(['mailbox-cancel', 'mailbox-save']);
});

test('removing the mailbox asks first', async ({ page }) => {
  await settings(page);
  await page.getByTestId('mailbox-remove').click();
  const dialog = page.getByTestId('dialog-remove-mailbox');
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Entfernen' }).click();
  await expect(page.getByTestId('mailbox-form')).toBeVisible();
  expect(await calls(page, 'remove_mailbox')).toHaveLength(1);
});

test('the trash empties itself after 30 days unless switched off', async ({ page }) => {
  await settings(page);
  const trash = page.getByTestId('toggle-auto-empty-trash');
  const fetch = page.getByTestId('settings-fetch');
  await expect(fetch).toContainText('Papierkorb nach 30 Tagen leeren');
  await expect(fetch).toContainText('Gelöschte Jobs sind danach endgültig weg.');
  const on = (await trash.getAttribute('aria-checked')) === 'true';
  await trash.click();
  await expect(trash).toHaveAttribute('aria-checked', on ? 'false' : 'true');
  expect((await calls(page, 'save_settings')).map(([, args]) => args)).toEqual([
    {
      patch: {
        portals: [],
        autoFetchOnStart: null,
        autoArchiveDays: null,
        autoEmptyTrashDays: on ? 0 : 30,
        language: null,
      },
    },
  ]);
});

test('old jobs archive themselves unless switched off', async ({ page }) => {
  await settings(page);
  const archive = page.getByTestId('toggle-auto-archive');
  await expect(archive).toHaveAttribute('aria-checked', 'true');
  await expect(page.getByTestId('settings-fetch')).toContainText('Jobs nach 30 Tagen archivieren');
  await expect(page.getByTestId('settings-fetch')).toContainText(
    'Favoriten werden nie archiviert.',
  );
  await archive.click();
  await expect(archive).toHaveAttribute('aria-checked', 'false');
  await archive.click();
  await expect(archive).toHaveAttribute('aria-checked', 'true');
  expect((await calls(page, 'save_settings')).map(([, args]) => args)).toEqual([
    {
      patch: {
        portals: [],
        autoFetchOnStart: null,
        autoArchiveDays: 0,
        autoEmptyTrashDays: null,
        language: null,
      },
    },
    {
      patch: {
        portals: [],
        autoFetchOnStart: null,
        autoArchiveDays: 30,
        autoEmptyTrashDays: null,
        language: null,
      },
    },
  ]);
});

test('a stored sign-in can be removed whatever the switches say', async ({ page }) => {
  await settings(page, `${WIN}&scenario=session-left`);
  const card = page.getByTestId('portal-freelance');
  await expect(card).toContainText('Die Anmeldung ist noch gespeichert.');
  await expect(card.getByTestId('session-freelance')).toContainText('Anmeldung');
  // The portal switched off: the row stays until the sign-in is gone.
  await page.getByTestId('toggle-enabled-freelance').click();
  await expect(card.getByTestId('sign-out-freelance')).toBeVisible();
  await card.getByTestId('sign-out-freelance').click();
  await expect(card.getByTestId('sign-out-freelance')).toHaveCount(0);
  expect(await calls(page, 'portal_logout')).toHaveLength(1);
});

test('auto fetch and portal switches save at once', async ({ page }) => {
  await settings(page);
  await page.getByTestId('toggle-auto-fetch').click();
  await expect(page.getByTestId('toggle-auto-fetch')).toHaveAttribute('aria-checked', 'false');
  await page.getByTestId('toggle-enabled-linkedin').click();
  await expect(page.getByTestId('toggle-enabled-linkedin')).toHaveAttribute(
    'aria-checked',
    'false',
  );
  // "Aktiv" sits in the portal's header row; an inactive portal hides "Details holen".
  await expect(
    page.getByTestId('portal-linkedin').locator('.head').getByTestId('toggle-enabled-linkedin'),
  ).toBeVisible();
  await expect(page.getByTestId('toggle-details-linkedin')).toHaveCount(0);
  await expect(page.getByTestId('toggle-details-freelancermap')).toBeVisible();
  // A switch is its own answer: no "Gespeichert" toast piles up.
  await expect(page.getByTestId('toast')).toHaveCount(0);
  const saved = (await calls(page, 'save_settings')).map(([, args]) => args);
  expect(saved).toEqual([
    {
      patch: {
        portals: [],
        autoFetchOnStart: false,
        autoArchiveDays: null,
        autoEmptyTrashDays: null,
        language: null,
      },
    },
    {
      patch: {
        portals: [{ portal: 'linkedin', enabled: false, fetchDetails: null, loginEnabled: null }],
        autoFetchOnStart: null,
        autoArchiveDays: null,
        autoEmptyTrashDays: null,
        language: null,
      },
    },
  ]);
});

test('a switch row toggles from its text like the system settings', async ({ page }) => {
  await settings(page);
  const auto = page.getByTestId('toggle-auto-fetch');
  await expect(auto).toHaveAttribute('aria-checked', 'true');
  await page.getByTestId('settings-fetch').getByText('Beim Start abrufen').click();
  await expect(auto).toHaveAttribute('aria-checked', 'false');
  await page
    .getByTestId('details-linkedin')
    .getByText('Gastzugang, kein Konto ist betroffen.')
    .click();
  await expect(page.getByTestId('toggle-details-linkedin')).toHaveAttribute(
    'aria-checked',
    'false',
  );
  // Off, the row says what that changes instead of the risk of the requests.
  await expect(page.getByTestId('details-linkedin')).toContainText(
    'Ohne Details bekommen die Jobs dieses Portals keine Passung.',
  );
  // A copyable path is text to select, never a switch: the workspace row has no label.
  await expect(page.getByTestId('settings-files').locator('label')).toHaveCount(0);
});

test('each switch carries its risk once; freelance.de sign in and out', async ({ page }) => {
  await settings(page);
  const card = page.getByTestId('portal-freelance');
  // Guest details are a grey area; signing in would risk the account (said on that switch).
  await expect(page.getByTestId('details-freelance')).toContainText('Graubereich');
  await expect(page.getByTestId('login-freelance')).toContainText('Kontorisiko');
  await expect(page.getByTestId('details-freelancermap')).toContainText('Geringes Risiko');
  // Each risk word explains itself on hover.
  await page.getByTestId('details-freelance').getByText('Graubereich').hover();
  await expect(page.getByRole('tooltip')).toHaveText(
    'Das Portal erlaubt automatisches Lesen nicht ausdrücklich.',
  );
  await page.getByTestId('login-freelance').getByText('Kontorisiko').hover();
  await expect(page.getByRole('tooltip')).toHaveText(
    'Im schlimmsten Fall sperrt das Portal das eigene Konto.',
  );
  await page.getByTestId('toggle-login-freelance').click();
  // Each switch keeps its own badge: nothing jumps between rows.
  await expect(page.getByTestId('details-freelance')).toContainText('Graubereich');
  await expect(page.getByTestId('login-freelance')).toContainText('Kontorisiko');
  await expect(card.getByText('Kontorisiko')).toHaveCount(1);
  // The sign-in row is named for what it is; its state is the badge or the hint.
  const session = page.getByTestId('session-freelance');
  await expect(session).toContainText('Anmeldung');
  await expect(session).toContainText('Nicht angemeldet.');
  await page.getByTestId('sign-in-freelance').click();
  await expect(session.locator('.badge')).toHaveText('Angemeldet');
  await page.getByTestId('sign-out-freelance').click();
  await expect(session).toContainText('Nicht angemeldet.');
  // Details off: no request, so no risk badge, sign-in shown off, no sign-in row.
  await page.getByTestId('toggle-details-freelance').click();
  await expect(page.getByTestId('details-freelance').locator('.badge')).toHaveCount(0);
  await expect(page.getByTestId('toggle-login-freelance')).toHaveAttribute('aria-checked', 'false');
  await expect(session).toHaveCount(0);
  expect(await calls(page, 'portal_logout')).toHaveLength(1);
});

test('quota only from 80 %, pauses with reason and end', async ({ page }) => {
  await settings(page);
  const freelancermap = page.getByTestId('quota-freelancermap');
  await expect(freelancermap).toContainText('Heute 86 von 100 Seiten');
  await expect(freelancermap.getByRole('progressbar')).toBeVisible();
  // The status is the last part of the card, under the switches, with its own divider.
  const order = await page
    .getByTestId('portal-freelancermap')
    .evaluate((card) =>
      [...card.querySelectorAll('[data-testid]')].map((node) => node.getAttribute('data-testid')),
    );
  expect(order.indexOf('status-freelancermap')).toBeGreaterThan(
    order.indexOf('details-freelancermap'),
  );
  await settings(page, `${WIN}&scenario=paused`);
  // One sentence each, saying whether she has to act: a pause needs nothing (calm info),
  // alert mails without jobs ask her to look (warning).
  const pause = page.getByTestId('health-linkedin');
  await expect(pause).toHaveText(
    'Das Portal bremst die Anfragen, der Abruf macht ab 11:05 von selbst weiter.',
  );
  await expect(pause).toHaveClass(/info/);
  const mails = page.getByTestId('health-freelance');
  await expect(mails).toHaveText(
    '2 Alert-Mails enthielten keine Jobs, bitte in Gmail nachsehen, ob dort welche stehen.',
  );
  await expect(mails).toHaveClass(/warning/);
  // The hour binds: bar and words speak of the same window.
  const quota = page.getByTestId('quota-freelancermap');
  await expect(quota).toContainText('Diese Stunde 38 von 40 Seiten');
  await expect(quota.getByRole('progressbar')).toHaveAttribute('aria-valuenow', /^9[45]/);
});

test('first run: a profile that names nothing to score keeps step two open', async ({ page }) => {
  await open(page, `${WIN}&scenario=first-run-empty-profile`);
  const step = page.getByTestId('step-profile');
  await expect(step).toHaveAttribute('data-done', 'false');
  await expect(page.getByTestId('first-profile')).toHaveClass(/primary/);
  await expect(page.getByTestId('first-fetch')).not.toHaveClass(/primary/);
});

test('every path row works the same: the path as text, the folder or file opens', async ({
  page,
}) => {
  await settings(page);
  const row = page.getByTestId('excel');
  await expect(row).toContainText('C:/Users/demo/Documents/Job-Alerts/auswertung/JobAlerts.xlsx');
  await expect(row.locator('[data-copy]')).toHaveCount(1);
  await page.getByTestId('excel-open').click();
  expect((await calls(page, 'open_target')).at(-1)?.[1]).toEqual({ target: { kind: 'excel' } });
  for (const [id, kind] of [
    ['workspace-open', 'workspace'],
    ['logs-open', 'logDir'],
    ['data-open', 'dataDir'],
  ] as const) {
    await expect(page.getByTestId(id)).toHaveText('Ordner öffnen');
    await page.getByTestId(id).click();
    expect((await calls(page, 'open_target')).at(-1)?.[1]).toEqual({ target: { kind } });
  }
  await expect(page.getByText('Pfad kopieren')).toHaveCount(0);
});

test('a portal that is off says so; its name switches it', async ({ page }) => {
  await settings(page);
  await page.getByTestId('portal-linkedin').locator('label.name').click();
  await expect(page.getByTestId('toggle-enabled-linkedin')).toHaveAttribute(
    'aria-checked',
    'false',
  );
  await expect(page.getByTestId('portal-off-linkedin')).toHaveText('Wird beim Abruf übersprungen.');
});

test('a run holds the mailbox, the folder and the files', async ({ page }) => {
  await settings(page, `${WIN}&scenario=running`);
  for (const id of ['mailbox-change', 'mailbox-remove', 'workspace-change', 'txt-clear']) {
    await expect(page.getByTestId(id)).toHaveAttribute('aria-disabled', 'true');
  }
  await page.getByTestId('mailbox-change').hover();
  await expect(page.getByRole('tooltip')).toHaveText('Ein Abruf läuft gerade.');
});

test('the mailbox says when the last fetch could not reach Gmail', async ({ page }) => {
  await settings(page, `${WIN}&scenario=offline`);
  const mailbox = page.getByTestId('settings-mailbox');
  await expect(mailbox).toContainText('Nicht erreichbar');
  await expect(mailbox).not.toContainText('Verbunden');
  // The badge says it all; no sentence under it repeats it.
  await expect(page.getByTestId('mailbox-failure')).toHaveCount(0);
});

test('the macOS demo shows the keychain and Mac paths', async ({ page }) => {
  await settings(page, '?platform=macos');
  await expect(page.getByTestId('settings-mailbox')).toContainText('macOS-Schlüsselbund');
  await expect(page.getByTestId('excel')).toContainText('/Users/demo/Documents/Job-Alerts');
  await expect(page.getByTestId('settings')).not.toContainText('C:/');
});

test('files: rewrite and delete the text files where they are', async ({ page }) => {
  await settings(page);
  // The result answers under Dateien, where the action happened (no toast).
  await page.getByTestId('txt-rewrite').click();
  await expect(page.getByTestId('files-note')).toHaveText('38 Dateien geschrieben.');
  await page.getByTestId('txt-clear').click();
  await page.getByTestId('dialog-clear').getByRole('button', { name: 'Löschen' }).click();
  await expect(page.getByTestId('files-note')).toHaveText('38 Dateien gelöscht.');
  await expect(page.getByTestId('toast')).toHaveCount(0);
  await expect(page.getByTestId('txt-clear')).toHaveAttribute('aria-disabled', 'true');
});

test('reading the whole mailbox asks first, then shows the run', async ({ page }) => {
  await settings(page);
  await page.getByTestId('full-mailbox').click();
  await page
    .getByTestId('dialog-full-mailbox')
    .getByRole('button', { name: 'Postfach lesen' })
    .click();
  await expect(page.getByTestId('view-jobs')).toBeVisible();
  const started = await calls(page, 'start_run');
  expect((started[0]?.[1] as { request: unknown }).request).toEqual({ kind: 'fullMailbox' });
});

test('reset asks with a danger dialog; the report shows after the restart', async ({ page }) => {
  await settings(page);
  await page.getByTestId('reset').click();
  const dialog = page.getByTestId('dialog-reset');
  await expect(dialog.getByRole('button', { name: 'Abbrechen' })).toBeFocused();
  await dialog.getByRole('button', { name: 'Zurücksetzen' }).click();
  expect(await calls(page, 'reset_all')).toHaveLength(1);
  // After the restart the app is empty: the first-run page with the report.
  await open(page, `${WIN}&scenario=reset`);
  const report = page.getByTestId('first-reset-report');
  await expect(report).toContainText('Die App ist zurückgesetzt, 1 Datei ließ sich nicht löschen.');
  // The file left behind can be found: the app's folder opens.
  await report.getByRole('button', { name: 'Ordner öffnen' }).click();
  expect((await calls(page, 'open_target')).at(-1)?.[1]).toEqual({
    target: { kind: 'dataDir' },
  });
  await expect(page.getByTestId('step-mailbox')).toHaveAttribute('data-done', 'false');
  await expect(page.getByTestId('step-profile')).toHaveAttribute('data-done', 'false');
});

test('a refused app password says so in the form', async ({ page }) => {
  await open(page, `${WIN}&scenario=first-run`);
  await page.getByTestId('mailbox-user').fill('alerts.demo@gmail.com');
  await page.getByTestId('mailbox-password').fill('fals chfa lsch fals');
  await page.getByTestId('mailbox-password').press('Enter');
  // Said once above the button; both fields are marked, since either may be wrong.
  await expect(page.getByTestId('mailbox-error')).toHaveText(
    'Gmail lehnt Adresse oder App-Passwort ab.',
  );
  for (const id of ['mailbox-user', 'mailbox-password']) {
    await expect(page.getByTestId(id)).toHaveAttribute('aria-invalid', 'true');
  }
  await expect(page.getByTestId('step-mailbox')).toHaveAttribute('data-done', 'false');
});

test('a changed mailbox: save and cancel at the trailing edge, a note that says it', async ({
  page,
}) => {
  await settings(page);
  await page.getByTestId('mailbox-change').click();
  const form = page.getByTestId('mailbox-form');
  const edge = await form.evaluate((node) => {
    const actions = node.querySelector('.actions')!.getBoundingClientRect();
    const last = [...node.querySelectorAll('.actions .btn')].at(-1)!.getBoundingClientRect();
    return Math.abs(actions.right - last.right);
  });
  expect(edge).toBeLessThanOrEqual(1);
  await page.getByTestId('mailbox-password').fill('abcd efgh ijkl mnop');
  await page.getByTestId('mailbox-save').click();
  // Like every action in Einstellungen, it answers where it happened (no toast).
  await expect(page.getByTestId('mailbox-note')).toHaveText('Postfach verbunden.');
  await expect(page.getByTestId('toast')).toHaveCount(0);
});

test('the dry run shows its mailbox and refuses what would write outside it', async ({ page }) => {
  await settings(page, `${WIN}&scenario=dry-run`);
  await expect(page.getByTestId('settings')).toContainText('probelauf@example.org');
  const remove = page.getByTestId('mailbox-remove');
  await expect(remove).toHaveAttribute('aria-disabled', 'true');
  await remove.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Im Probelauf geht das nicht.');
});

test('locked buttons explain themselves', async ({ page }) => {
  // A workspace without files yet has no Excel file to show.
  await settings(page, `${WIN}&scenario=no-files`);
  const excel = page.getByTestId('excel-open');
  await excel.hover();
  await expect(page.getByRole('tooltip')).toHaveText('Die Excel-Datei entsteht beim ersten Abruf.');
  // Without a mailbox reading the whole mailbox is locked.
  await page.getByTestId('mailbox-remove').click();
  await page
    .getByTestId('dialog-remove-mailbox')
    .getByRole('button', { name: 'Entfernen' })
    .click();
  await expect(page.getByTestId('mailbox-form')).toBeVisible();
  await page.getByTestId('full-mailbox').hover();
  await expect(page.getByRole('tooltip')).toHaveText('Erst ein Postfach verbinden.');
});

test('first run: the sidebar waits until the setup is done', async ({ page }) => {
  await open(page, `${WIN}&scenario=first-run`);
  const nav = page.getByTestId('nav-settings').locator('xpath=../..');
  await expect(nav).toHaveAttribute('inert', '');
  await page
    .getByTestId('nav-settings')
    .click({ force: true, timeout: 2000 })
    .catch(() => {});
  await expect(page.getByTestId('first-run')).toBeVisible();
  // The helper line of the password carries the way to create one.
  await expect(
    page.getByTestId('mailbox-form').locator('.help').getByTestId('create-password'),
  ).toBeVisible();
});

test('baseline: first run', async ({ page }) => {
  await open(page, `${WIN}&scenario=first-run`);
  await expectShot(page, 'first-run');
});

test('baseline: settings', async ({ page }) => {
  await settings(page);
  await expectShot(page, 'settings');
});

test('baseline: settings with portals', async ({ page }) => {
  await settings(page, `${WIN}&scenario=paused`);
  await page.getByTestId('settings-portals').scrollIntoViewIfNeeded();
  await expectShot(page, 'settings-portals');
});
