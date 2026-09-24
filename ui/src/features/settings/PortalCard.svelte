<!--
  One portal in the settings. The header row carries the portal, the portal in the browser
  and its switch (Aktiv); reading its alert mails touches nothing but the own Gmail. Only an
  active portal shows more: its problem in one sentence that says whether she has to act
  (warning) or the app carries on by itself (info), the quota only from 80 % or while paused
  (the window that binds, a 6 px meter), and one row per switch with the risk that switch brings (PLAN:
  a risk badge per switch). Details holen carries the risk of the requests with its sentence;
  for freelance.de Mit Anmeldung warns with Kontorisiko while the Details row does not say it
  yet, then sign in / sign out (which deletes the session).
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

  let error = $state<string | null>(null);
  let busy = $state(false);
  /** Only the answer to the latest save may replace the state (quick double flips). */
  let saves = 0;

  /** Its problem, and whether she has to act (warning) or the app carries on (info). */
  const health = $derived(healthAdvice(portal.health));
  /** The fuller of the two windows: its numbers are the ones the text names. */
  const quota = $derived.by(() => {
    const q = portal.quota;
    if (q === null) return null;
    const day = q.usedDay / Math.max(q.capDay, 1);
    const hour = q.usedHour / Math.max(q.capHour, 1);
    const paused = portal.health.kind === 'paused' || portal.health.kind === 'quotaReached';
    const share = Math.max(day, hour);
    if (share < QUOTA_SHOWN && !paused) return null;
    const text =
      hour > day
        ? t.settings.quotaHour(q.usedHour, q.capHour)
        : t.settings.quota(q.usedDay, q.capDay);
    return { share, text };
  });
  /** The risk of fetching details now: signed in it is the own account. */
  const risk = $derived<Risk>(portal.loginEnabled ? 'account' : portal.risk);
  const dryRun = $derived(app.state?.dryRun ?? false);

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
          language: null,
        },
      });
      if (save === saves) app.set(next);
    } catch (failure) {
      error = errorText(failure);
      void app.load();
    }
  }

  async function session(signIn: boolean): Promise<void> {
    error = null;
    busy = true;
    try {
      await invoke(signIn ? 'portal_login' : 'portal_logout', { portal: portal.portal });
      await app.load();
    } catch (failure) {
      error = errorText(failure);
    } finally {
      busy = false;
    }
  }

  function openPortal(): void {
    invoke('open_target', { target: { kind: 'portalHome', portal: portal.portal } }).catch(
      (failure: unknown) => (error = errorText(failure)),
    );
  }
</script>

<Card padding="none" testid="portal-{portal.portal}">
  <div class="head">
    <IconTile tone="navy" monogram={PORTAL_MONOGRAM[portal.portal]} size="md" />
    <h3 class="name">{t.portal[portal.portal]}</h3>
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
        checked={portal.enabled}
        label={t.settings.active}
        testid="toggle-enabled-{portal.portal}"
        onchange={(on) => change({ enabled: on })}
      />
    </div>
  </div>
  <!-- Switching the portal on, its rows rise in; off, they fade (no height animation). -->
  {#if portal.enabled || error}
    <div class="body" in:rise={{ distance: 'sm' }} out:fade>
      {#if portal.enabled && health}
        <Notice
          tone={health.act ? 'warning' : 'info'}
          variant="inline"
          text={health.text}
          testid="health-{portal.portal}"
        />
      {/if}
      {#if quota}
        <div class="quota" data-testid="quota-{portal.portal}">
          <span>{quota.text}</span>
          <Meter value={quota.share} tone="warning" size="md" label={quota.text} />
        </div>
      {/if}

      {#if portal.enabled}
        <div class="rows">
          <SettingRow
            label={t.settings.details}
            hint={t.settings.riskText[risk]}
            for="switch-details-{portal.portal}"
            testid="details-{portal.portal}"
          >
            {#snippet badges()}
              <Badge label={t.settings.risk[risk]} tone={RISK_TONE[risk]} icon="shield" />
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
                {#if risk !== 'account'}
                  <Badge label={t.settings.risk.account} tone="danger" icon="shield" />
                {/if}
              {/snippet}
              <Toggle
                id="switch-login-{portal.portal}"
                checked={portal.loginEnabled}
                label={t.settings.login}
                disabled={!portal.fetchDetails}
                disabledReason={t.settings.needsDetails}
                testid="toggle-login-{portal.portal}"
                onchange={(on) => change({ loginEnabled: on })}
              />
            </SettingRow>
            {#if portal.loginEnabled}
              <SettingRow
                label={portal.signedIn ? t.settings.signedIn : t.settings.signedOut}
                hint={run.loginNeeded === portal.portal ? t.settings.signInWaiting : null}
              >
                {#if portal.signedIn}
                  <Button
                    variant="secondary"
                    size="sm"
                    icon="log-out"
                    label={t.settings.signOut}
                    loading={busy}
                    disabled={dryRun}
                    disabledReason={t.error.text('dryRun', {})}
                    testid="sign-out-{portal.portal}"
                    onclick={() => void session(false)}
                  />
                {:else}
                  <Button
                    variant="secondary"
                    size="sm"
                    icon="log-in"
                    label={t.settings.signIn}
                    loading={busy}
                    disabled={run.active || dryRun}
                    disabledReason={dryRun ? t.error.text('dryRun', {}) : t.settings.running}
                    testid="sign-in-{portal.portal}"
                    onclick={() => void session(true)}
                  />
                {/if}
              </SettingRow>
            {/if}
          {/if}
        </div>
      {/if}
      {#if error}
        <Notice tone="danger" variant="inline" text={error} testid="portal-error" />
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

  /* The rows run edge to edge like the card's dividers (--row-inset); other content keeps
     the card's inset of 20. */
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    padding: 0 var(--space-20) var(--space-4);
    border-top: var(--border-width) solid var(--border);
    --row-inset: var(--space-20);
  }

  .body > :global(:first-child:not(.rows)) {
    margin-top: var(--space-12);
  }

  .body > :global(:last-child:not(.rows)) {
    margin-bottom: var(--space-12);
  }

  /* A card label, not a heading of its own: 15/500 under the 17/600 section heading. */
  .name {
    flex: 1;
    min-width: 0;
    color: var(--text-heading);
    font: var(--type-title);
  }

  .tools {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }

  .quota {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .rows {
    display: flex;
    flex-direction: column;
  }
</style>
