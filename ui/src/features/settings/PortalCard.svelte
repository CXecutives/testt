<!--
  One portal in the settings: the header row carries the portal, its risk (badge and one
  sentence), the portal in the browser and the switch Aktiv. Only an active portal shows
  more: its health, the quota only from 80 % or while paused (a 6 px meter), Details holen,
  for freelance.de Mit Anmeldung and sign in / sign out (which deletes the session).
  A switch saves at once; a failure puts the switch back and says why here.
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
  import { de } from '$lib/i18n/de';
  import { errorText, healthText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { PortalPatch, PortalState, Risk } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';

  interface Props {
    portal: PortalState;
  }
  let { portal }: Props = $props();

  const QUOTA_SHOWN = 0.8;
  const RISK_TONE: Record<Risk, BadgeTone> = { low: 'success', grey: 'warning', account: 'danger' };

  let error = $state<string | null>(null);
  let busy = $state(false);

  const health = $derived(healthText(portal.health));
  // The window that binds (the hour or the day) gives bar and words the same numbers.
  const quota = $derived.by(() => {
    const q = portal.quota;
    if (q === null) return null;
    const day = q.usedDay / Math.max(q.capDay, 1);
    const hour = q.usedHour / Math.max(q.capHour, 1);
    const paused = portal.health.kind === 'paused' || portal.health.kind === 'quotaReached';
    if (Math.max(day, hour) < QUOTA_SHOWN && !paused) return null;
    return hour > day
      ? { share: hour, text: de.settings.quotaHour(q.usedHour, q.capHour) }
      : { share: day, text: de.settings.quota(q.usedDay, q.capDay) };
  });
  const risk = $derived<Risk>(portal.loginEnabled ? 'account' : portal.risk);

  async function change(patch: Partial<Omit<PortalPatch, 'portal'>>): Promise<void> {
    error = null;
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
        },
      });
      app.set(next);
      toasts.show(de.toast.saved);
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
    <IconTile monogram={PORTAL_MONOGRAM[portal.portal]} size="md" />
    <div class="title">
      <h3 class="name">{de.portal[portal.portal]}</h3>
      <p class="risk">{de.settings.riskText[risk]}</p>
    </div>
    <div class="badges">
      <Badge label={de.settings.risk[risk]} tone={RISK_TONE[risk]} icon="shield" />
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon="external-link"
        label={de.settings.openPortal}
        testid="open-portal-{portal.portal}"
        onclick={openPortal}
      />
      <Toggle
        checked={portal.enabled}
        label={de.settings.active}
        testid="toggle-enabled-{portal.portal}"
        onchange={(on) => void change({ enabled: on })}
      />
    </div>
  </div>
  {#if portal.enabled || error}
    <div class="body">
      {#if portal.enabled && health.text}
        <Notice
          tone="warning"
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
          <SettingRow label={de.settings.details}>
            <Toggle
              checked={portal.fetchDetails}
              label={de.settings.details}
              testid="toggle-details-{portal.portal}"
              onchange={(on) => void change({ fetchDetails: on })}
            />
          </SettingRow>
          {#if portal.login === 'optional'}
            <SettingRow label={de.settings.login} hint={de.settings.loginHint}>
              {#snippet badges()}
                <Badge label={de.settings.risk.account} tone="danger" />
              {/snippet}
              <Toggle
                checked={portal.loginEnabled}
                label={de.settings.login}
                disabled={!portal.fetchDetails}
                disabledReason={de.settings.needsDetails}
                testid="toggle-login-{portal.portal}"
                onchange={(on) => void change({ loginEnabled: on })}
              />
            </SettingRow>
            {#if portal.loginEnabled}
              <SettingRow
                label={portal.signedIn ? de.settings.signedIn : de.settings.signedOut}
                hint={run.loginNeeded === portal.portal ? de.settings.signInWaiting : null}
              >
                {#if portal.signedIn}
                  <Button
                    variant="secondary"
                    size="sm"
                    icon="log-out"
                    label={de.settings.signOut}
                    loading={busy}
                    testid="sign-out-{portal.portal}"
                    onclick={() => void session(false)}
                  />
                {:else}
                  <Button
                    variant="secondary"
                    size="sm"
                    icon="log-in"
                    label={de.settings.signIn}
                    loading={busy}
                    disabled={run.active}
                    disabledReason={run.busyText}
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

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    margin: 0 var(--space-20);
    padding-bottom: var(--space-4);
    border-top: var(--border-width) solid var(--border);
  }

  .body > :global(:first-child:not(.rows)) {
    margin-top: var(--space-12);
  }

  .body > :global(:last-child:not(.rows)) {
    margin-bottom: var(--space-12);
  }

  .title {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .name {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .risk {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .badges {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
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
