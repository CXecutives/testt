<!--
  One portal in the settings, the same skeleton on every card: the header (the portal by its
  web address, the portal in the browser, its switch Aktiv; the name is the switch's label),
  then the switch rows, then the status under its own divider (the portal's problem in one
  sentence that says whether she has to act, with "Alert-Mail öffnen" when alert mails came
  without jobs, and the pages used today, the meter only from 80 % or while paused). What
  concerns only the pages (a pause, the limit, the sign-in) goes while details are off:
  then no page is fetched. A portal that is off says in one line that the fetch skips it.
  Each switch carries its own risk and keeps it: Details holen the risk of the requests
  (while it is on), Mit Anmeldung always Kontorisiko. Sign-in exists only while details and
  sign-in are both on; a stored sign-in the switches no longer show keeps its Abmelden.
  A switch moves at once (the state is patched before the save); a failure puts it back and
  says why here. The switch itself is the answer: no toast.
-->
<script lang="ts">
  import Badge, { type BadgeTone } from '$components/Badge.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import IconTile, { PORTAL_MONOGRAM } from '$components/IconTile.svelte';
  import Meter from '$components/Meter.svelte';
  import Notice from '$components/Notice.svelte';
  import SettingRow from '$components/SettingRow.svelte';
  import Toggle from '$components/Toggle.svelte';
  import { t } from '$lib/i18n/t';
  import { errorText, healthAdvice } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { PortalState, Risk } from '$lib/ipc/types';
  import { fade, rise } from '$lib/motion/transitions';
  import { app } from '$lib/state/app.svelte';
  import { run } from '$lib/state/run.svelte';

  interface Props {
    portal: PortalState;
  }
  let { portal }: Props = $props();

  type Switches = Partial<Pick<PortalState, 'enabled' | 'fetchDetails' | 'loginEnabled'>>;

  const QUOTA_SHOWN = 0.8;
  const RISK_TONE: Record<Risk, BadgeTone> = { low: 'success', grey: 'warning', account: 'danger' };

  /** Why the last action failed, said when it shows (so in the language of the moment). */
  let error = $state<(() => string) | null>(null);
  let busy = $state(false);
  /** Only the answer to the latest save may replace the state (quick double flips). */
  let saves = 0;

  /** Alert mails of the portal that came without jobs (a mail problem, not one of the pages). */
  const emptyMails = $derived(
    portal.health.kind === 'layoutSuspect' ? portal.health.emptyMails : 0,
  );
  /** Its problem in one sentence; `portal.actionNeeded` says whether she has to act. Without
   *  details only the alert mails can have one (no page is fetched). */
  const health = $derived(
    portal.fetchDetails || emptyMails > 0 ? healthAdvice(portal.health) : null,
  );
  /** One of those mails to look at in Gmail (from the last fetch), as the day overview does. */
  const alertMail = $derived(
    emptyMails > 0
      ? (app.state?.lastRun?.emptyAlerts.find(
          (alert) => alert.portal === portal.portal && alert.gmailId !== null,
        )?.gmailId ?? null)
      : null,
  );
  /** The pages used: the fuller window's numbers; the meter only near the limit. Only while
   *  details are fetched: without them no page counts. */
  const quota = $derived.by(() => {
    const q = portal.quota;
    if (q === null || !portal.fetchDetails) return null;
    const day = q.usedDay / Math.max(q.capDay, 1);
    const hour = q.usedHour / Math.max(q.capHour, 1);
    const paused = portal.health.kind === 'paused' || portal.health.kind === 'quotaReached';
    const share = Math.max(day, hour);
    const text =
      hour > day
        ? t.settings.quotaHour(q.usedHour, q.capHour)
        : t.settings.quota(q.usedDay, q.capDay);
    return { share, text, meter: share >= QUOTA_SHOWN || paused };
  });
  /** Sign-in only matters while details are fetched. */
  const signIn = $derived(portal.fetchDetails && portal.loginEnabled);
  /** A stored sign-in the switches no longer show: its Abmelden stays until it is gone. */
  const leftover = $derived(portal.signedIn === true && !(portal.enabled && signIn));
  /** The risk of the requests as a guest (the account risk belongs to Mit Anmeldung). */
  const detailsRisk = $derived<Risk>(portal.risk === 'account' ? 'grey' : portal.risk);
  const status = $derived(portal.enabled && (health !== null || quota !== null));
  const dryRun = $derived(app.state?.dryRun ?? false);
  const dryRunReason = $derived(t.error.text('dryRun', {}));

  async function change(patch: Switches): Promise<void> {
    error = null;
    const save = ++saves;
    // The switch and the rows that hang on it follow at once, not after the round trip.
    const item = app.state?.portals.find((p) => p.portal === portal.portal);
    if (item) Object.assign(item, patch);
    try {
      const next = await invoke('save_settings', {
        patch: {
          portals: [
            {
              portal: portal.portal,
              enabled: patch.enabled ?? null,
              fetchDetails: patch.fetchDetails ?? null,
              loginEnabled: patch.loginEnabled ?? null,
            },
          ],
          autoFetchOnStart: null,
          autoArchiveDays: null,
          autoEmptyTrashDays: null,
          language: null,
        },
      });
      if (save === saves) app.set(next);
    } catch (failure) {
      error = () => errorText(failure);
      void app.load();
    }
  }

  async function session(on: boolean): Promise<void> {
    error = null;
    busy = true;
    try {
      await invoke(on ? 'portal_login' : 'portal_logout', { portal: portal.portal });
      await app.load();
    } catch (failure) {
      error = () => errorText(failure);
    } finally {
      busy = false;
    }
  }

  function openPortal(): void {
    invoke('open_target', { target: { kind: 'portalHome', portal: portal.portal } }).catch(
      (failure: unknown) => (error = () => errorText(failure)),
    );
  }

  function openMail(gmailId: string): void {
    error = null;
    invoke('open_target', { target: { kind: 'alertMail', gmailId } }).catch(
      (failure: unknown) => (error = () => errorText(failure)),
    );
  }
</script>

{#snippet signOut()}
  <Button
    variant="secondary"
    size="sm"
    icon="log-out"
    label={t.settings.signOut}
    loading={busy}
    disabled={dryRun}
    disabledReason={dryRunReason}
    testid="sign-out-{portal.portal}"
    onclick={() => void session(false)}
  />
{/snippet}

<Card padding="none" testid="portal-{portal.portal}">
  <div class="head">
    <IconTile tone="navy" monogram={PORTAL_MONOGRAM[portal.portal]} size="md" />
    <div class="title">
      <label class="name" for="switch-enabled-{portal.portal}">{t.portal[portal.portal]}</label>
      {#if !portal.enabled}
        <p class="off" data-testid="portal-off-{portal.portal}">{t.settings.portalOff}</p>
      {/if}
    </div>
    <div class="tools">
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon="external-link"
        label={t.settings.openPortal}
        testid="open-portal-{portal.portal}"
        onclick={openPortal}
      />
      <Toggle
        id="switch-enabled-{portal.portal}"
        checked={portal.enabled}
        label={t.settings.active}
        testid="toggle-enabled-{portal.portal}"
        onchange={(on) => change({ enabled: on })}
      />
    </div>
  </div>
  <!-- Switching the portal on, its rows rise in; off, they fade (no height animation). -->
  {#if portal.enabled || error || leftover}
    <div class="body" in:rise={{ distance: 'sm' }} out:fade>
      <div class="rows">
        {#if portal.enabled}
          <SettingRow
            label={t.settings.details}
            hint={portal.fetchDetails ? t.settings.riskText[detailsRisk] : t.settings.detailsOff}
            for="switch-details-{portal.portal}"
            testid="details-{portal.portal}"
          >
            {#snippet badges()}
              {#if portal.fetchDetails}
                <Badge
                  label={t.settings.risk[detailsRisk]}
                  tone={RISK_TONE[detailsRisk]}
                  icon="shield"
                  hint={t.settings.riskInfo[detailsRisk]}
                />
              {/if}
            {/snippet}
            <Toggle
              id="switch-details-{portal.portal}"
              checked={portal.fetchDetails}
              label={t.settings.details}
              testid="toggle-details-{portal.portal}"
              onchange={(on) => change({ fetchDetails: on })}
            />
          </SettingRow>
          {#if portal.login === 'optional'}
            <SettingRow
              label={t.settings.login}
              hint={t.settings.loginHint}
              for="switch-login-{portal.portal}"
              testid="login-{portal.portal}"
            >
              {#snippet badges()}
                <Badge
                  label={t.settings.risk.account}
                  tone="danger"
                  icon="shield"
                  hint={t.settings.riskInfo.account}
                />
              {/snippet}
              <Toggle
                id="switch-login-{portal.portal}"
                checked={signIn}
                label={t.settings.login}
                disabled={!portal.fetchDetails}
                disabledReason={t.settings.needsDetails}
                testid="toggle-login-{portal.portal}"
                onchange={(on) => change({ loginEnabled: on })}
              />
            </SettingRow>
          {/if}
        {/if}
        {#if portal.enabled && signIn}
          <SettingRow
            label={t.settings.session}
            hint={run.loginNeeded === portal.portal
              ? t.settings.signInWaiting
              : portal.signedIn
                ? null
                : t.settings.notSignedIn}
            testid="session-{portal.portal}"
          >
            {#snippet badges()}
              {#if portal.signedIn}
                <Badge label={t.settings.signedIn} tone="success" icon="check" />
              {/if}
            {/snippet}
            {#if portal.signedIn}
              {@render signOut()}
            {:else}
              <Button
                variant="secondary"
                size="sm"
                icon="log-in"
                label={t.settings.signIn}
                loading={busy}
                disabled={run.active || dryRun}
                disabledReason={dryRun ? dryRunReason : run.busyText}
                testid="sign-in-{portal.portal}"
                onclick={() => void session(true)}
              />
            {/if}
          </SettingRow>
        {:else if leftover}
          <SettingRow
            label={t.settings.session}
            hint={t.settings.sessionLeft}
            testid="session-{portal.portal}"
          >
            {#snippet badges()}
              <Badge label={t.settings.signedIn} tone="success" icon="check" />
            {/snippet}
            {@render signOut()}
          </SettingRow>
        {/if}
      </div>
      {#if status}
        <div class="status" data-testid="status-{portal.portal}">
          {#if health}
            <Notice
              tone={portal.actionNeeded ? 'warning' : 'info'}
              variant="inline"
              text={health}
              action={alertMail
                ? { label: t.reader.mail, onclick: () => openMail(alertMail) }
                : null}
              testid="health-{portal.portal}"
            />
          {/if}
          {#if quota}
            <div class="quota" data-testid="quota-{portal.portal}">
              <span>{quota.text}</span>
              {#if quota.meter}
                <Meter value={quota.share} tone="warning" size="md" label={quota.text} />
              {/if}
            </div>
          {/if}
        </div>
      {/if}
      {#if error}
        <div class="status">
          <Notice tone="danger" variant="inline" text={error()} testid="portal-error" />
        </div>
      {/if}
    </div>
  {/if}
</Card>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-16) var(--space-20);
  }

  /* The rows run edge to edge like the card's dividers (--row-inset); the status keeps the
     card's inset of 20 under its own divider. */
  .body {
    display: flex;
    flex-direction: column;
    padding: 0 var(--space-20);
    border-top: var(--border-width) solid var(--border);
    --row-inset: var(--space-20);
  }

  .title {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  /* The card's title, above the 15/500 row labels. */
  .name {
    color: var(--text-heading);
    font: var(--type-title);
    font-weight: var(--weight-semibold);
  }

  .off {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .tools {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }

  .rows {
    display: flex;
    flex-direction: column;
  }

  .rows:empty {
    display: none;
  }

  .status {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    margin-inline: calc(-1 * var(--space-20));
    padding: var(--space-12) var(--space-20);
    border-top: var(--border-width) solid var(--border);
  }

  .rows:empty + .status {
    border-top: 0;
  }

  .quota {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }
</style>
