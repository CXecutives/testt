<!--
  One portal in the settings: its switches (Aktiv, Details holen, for freelance.de Mit
  Anmeldung) with the risk as a badge and one sentence, its health, the quota only from
  80 % or while paused, sign in / sign out (which deletes the session) and the portal in
  the browser. A switch saves at once; a failure puts the switch back and says why here.
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

  interface Props {
    portal: PortalState;
  }
  let { portal }: Props = $props();

  const QUOTA_SHOWN = 0.8;
  const RISK_TONE: Record<Risk, BadgeTone> = { low: 'success', grey: 'warning', account: 'danger' };

  let error = $state<string | null>(null);
  let busy = $state(false);

  const health = $derived(healthText(portal.health));
  const quota = $derived.by(() => {
    const q = portal.quota;
    if (q === null) return null;
    const share = Math.max(q.usedDay / Math.max(q.capDay, 1), q.usedHour / Math.max(q.capHour, 1));
    const paused = portal.health.kind === 'paused' || portal.health.kind === 'quotaReached';
    if (share < QUOTA_SHOWN && !paused) return null;
    return { share, used: q.usedDay, cap: q.capDay };
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

<Card padding="md" testid="portal-{portal.portal}">
  <div class="head">
    <IconTile monogram={PORTAL_MONOGRAM[portal.portal]} tone="coral" size="md" />
    <div class="title">
      <h3 class="name">{de.portal[portal.portal]}</h3>
      <p class="risk">{de.settings.riskText[risk]}</p>
    </div>
    <div class="badges">
      <Badge label={de.settings.risk[risk]} tone={RISK_TONE[risk]} icon="shield" />
      {#if portal.enabled && portal.health.kind !== 'ok'}
        <Badge label={health.label} tone="warning" />
      {/if}
    </div>
  </div>

  {#if portal.enabled && health.text}
    <Notice tone="warning" variant="inline" text={health.text} testid="health-{portal.portal}" />
  {/if}
  {#if quota}
    <div class="quota" data-testid="quota-{portal.portal}">
      <span>{de.settings.quota(quota.used, quota.cap)}</span>
      <Meter
        value={quota.share}
        tone="warning"
        size="sm"
        label={de.settings.quota(quota.used, quota.cap)}
      />
    </div>
  {/if}

  <div class="rows">
    <SettingRow label={de.settings.active}>
      <Toggle
        checked={portal.enabled}
        label={de.settings.active}
        testid="toggle-enabled-{portal.portal}"
        onchange={(on) => void change({ enabled: on })}
      />
    </SettingRow>
    <SettingRow label={de.settings.details}>
      <Toggle
        checked={portal.fetchDetails && portal.enabled}
        label={de.settings.details}
        disabled={!portal.enabled}
        disabledReason={de.settings.needsActive}
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
          disabled={!portal.enabled || !portal.fetchDetails}
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
              disabledReason={de.settings.running}
              testid="sign-in-{portal.portal}"
              onclick={() => void session(true)}
            />
          {/if}
        </SettingRow>
      {/if}
    {/if}
  </div>

  {#if error}
    <Notice tone="danger" variant="inline" text={error} testid="portal-error" />
  {/if}
  <div class="foot">
    <Button
      variant="ghost"
      size="sm"
      icon="external-link"
      label={de.settings.openPortal}
      onclick={openPortal}
    />
  </div>
</Card>

<style>
  .head {
    display: flex;
    align-items: flex-start;
    gap: var(--space-12);
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
    justify-content: flex-end;
    gap: var(--space-6);
  }

  .quota {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    margin: var(--space-8) 0;
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .rows {
    display: flex;
    flex-direction: column;
  }

  .foot {
    display: flex;
    justify-content: flex-end;
    margin-top: var(--space-8);
  }
</style>
