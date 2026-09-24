<!--
  Einstellungen (centred 720): Postfach, Abruf, Portale, Dateien, Wartung - each a card of
  setting rows. Every action answers where it happened; dialogs only to confirm.
-->
<script lang="ts">
  import Badge from '$components/Badge.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Dialog from '$components/Dialog.svelte';
  import Notice, { type NoticeTone } from '$components/Notice.svelte';
  import SettingRow from '$components/SettingRow.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import Toggle from '$components/Toggle.svelte';
  import { de } from '$lib/i18n/de';
  import { errorText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { OpenTarget } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import MailboxForm from '../shared/MailboxForm.svelte';
  import PortalCard from './PortalCard.svelte';

  type Feedback = { tone: NoticeTone; text: string } | null;

  const cfg = $derived(app.state);
  let editing = $state(false);
  let mailboxNote = $state<Feedback>(null);
  let fetchNote = $state<Feedback>(null);
  let filesNote = $state<Feedback>(null);
  let careNote = $state<Feedback>(null);
  let confirmRemove = $state(false);
  let confirmClear = $state(false);
  let confirmFull = $state(false);
  let confirmReset = $state(false);
  let busy = $state<string | null>(null);

  async function act(
    name: string,
    note: (f: Feedback) => void,
    work: () => Promise<Feedback>,
  ): Promise<void> {
    busy = name;
    note(null);
    try {
      note(await work());
    } catch (error) {
      note({ tone: 'danger', text: errorText(error) });
    } finally {
      busy = null;
    }
  }

  function open(target: OpenTarget, note: (f: Feedback) => void): void {
    invoke('open_target', { target }).catch((error: unknown) =>
      note({ tone: 'danger', text: errorText(error) }),
    );
  }

  const setMailbox = (f: Feedback): void => void (mailboxNote = f);
  const setFetch = (f: Feedback): void => void (fetchNote = f);
  const setFiles = (f: Feedback): void => void (filesNote = f);
  const setCare = (f: Feedback): void => void (careNote = f);

  function removeMailbox(): void {
    void act('mailbox', setMailbox, async () => {
      await invoke('remove_mailbox');
      confirmRemove = false;
      await app.load();
      return null;
    });
  }

  function autoFetch(on: boolean): void {
    void act('fetch', setFetch, async () => {
      app.set(await invoke('save_settings', { patch: { portals: [], autoFetchOnStart: on } }));
      toasts.show(de.toast.saved);
      return null;
    });
  }

  function pickWorkspace(): void {
    void act('workspace', setFiles, async () => {
      const path = await invoke('pick_workspace');
      if (path !== null) await app.load();
      return null;
    });
  }

  function rewrite(): void {
    void act('rewrite', setFiles, async () => {
      const result = await invoke('rewrite_txt');
      await app.load();
      if (result.error)
        return { tone: 'danger', text: de.error.text(result.error.kind, result.error.params) };
      if (result.txtFailed > 0)
        return { tone: 'warning', text: de.settings.txtFailed(result.txtFailed) };
      toasts.show(de.settings.txtWritten(result.txtWritten));
      return null;
    });
  }

  function clear(): void {
    void act('clear', setFiles, async () => {
      const result = await invoke('clear_txt');
      confirmClear = false;
      await app.load();
      if (result.failed.length > 0) {
        return { tone: 'warning', text: de.settings.txtFailed(result.failed.length) };
      }
      toasts.show(de.settings.txtCleared(result.removed));
      return null;
    });
  }

  function readAll(): void {
    confirmFull = false;
    void run.start({ kind: 'fullMailbox' }).then((started) => {
      if (started) navigation.go('jobs');
      else setCare({ tone: 'danger', text: run.startError ?? de.run.failed });
    });
  }

  function reset(): void {
    void act('reset', setCare, async () => {
      await invoke('reset_all');
      return null;
    });
  }

  async function copyPath(path: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(path);
      toasts.show(de.toast.copied);
    } catch (error) {
      setCare({ tone: 'danger', text: errorText(error) });
    }
  }
</script>

{#snippet note(feedback: Feedback, testid: string)}
  {#if feedback}
    <Notice tone={feedback.tone} variant="inline" text={feedback.text} {testid} />
  {/if}
{/snippet}

<div class="page" data-testid="settings">
  {#if cfg === null}
    {#if app.slow}
      <Card
        ><div class="skeleton">
          <Skeleton width={40} /><Skeleton /><Skeleton width={70} />
        </div></Card
      >
    {/if}
  {:else}
    {#if cfg.dryRun}
      <Notice tone="info" text={de.settings.dryRun} />
    {/if}

    <section class="section" data-testid="settings-mailbox">
      <h2 class="heading">{de.settings.mailbox}</h2>
      <Card padding="md">
        {#if cfg.mailbox.user && !editing}
          <SettingRow label={cfg.mailbox.user} hint={de.settings.vault[cfg.mailbox.vault]}>
            {#snippet badges()}
              <Badge label={de.settings.connected} tone="success" icon="check" />
            {/snippet}
            <div class="buttons">
              <Button
                variant="secondary"
                size="sm"
                label={de.common.change}
                testid="mailbox-change"
                onclick={() => (editing = true)}
              />
              <Button
                variant="ghost"
                size="sm"
                icon="trash-2"
                label={de.common.remove}
                disabled={run.active}
                disabledReason={de.settings.running}
                testid="mailbox-remove"
                onclick={() => (confirmRemove = true)}
              />
            </div>
          </SettingRow>
        {:else}
          {#if !cfg.mailbox.user}
            <p class="lead">{de.settings.notConnected}</p>
          {/if}
          <MailboxForm
            saveLabel={cfg.mailbox.user ? de.common.save : de.settings.connect}
            oncancel={cfg.mailbox.user ? () => (editing = false) : null}
            onsaved={() => (editing = false)}
          />
        {/if}
        {#if cfg.mailbox.error}
          <Notice
            tone="danger"
            variant="inline"
            text={de.error.text(cfg.mailbox.error.kind, cfg.mailbox.error.params)}
          />
        {/if}
        {@render note(mailboxNote, 'mailbox-note')}
      </Card>
    </section>

    <section class="section" data-testid="settings-fetch">
      <h2 class="heading">{de.settings.fetch}</h2>
      <Card padding="md">
        <SettingRow label={de.settings.autoFetch} hint={de.settings.autoFetchHint}>
          <Toggle
            checked={cfg.autoFetchOnStart}
            label={de.settings.autoFetch}
            testid="toggle-auto-fetch"
            onchange={autoFetch}
          />
        </SettingRow>
        {@render note(fetchNote, 'fetch-note')}
      </Card>
    </section>

    <section class="section" data-testid="settings-portals">
      <h2 class="heading">{de.settings.portals}</h2>
      {#each cfg.portals as portal (portal.portal)}
        <PortalCard {portal} />
      {/each}
    </section>

    <section class="section" data-testid="settings-files">
      <h2 class="heading">{de.settings.files}</h2>
      <Card padding="md">
        <SettingRow label={de.settings.workspace} hint={cfg.settings.workspace}>
          {#snippet badges()}
            {#if cfg.settings.workspaceIsDefault}
              <Badge label={de.settings.workspaceDefault} />
            {/if}
          {/snippet}
          <div class="buttons">
            <Button
              variant="secondary"
              size="sm"
              label={de.common.change}
              loading={busy === 'workspace'}
              onclick={pickWorkspace}
            />
            <Button
              variant="ghost"
              size="sm"
              icon="folder-open"
              label={de.common.open}
              onclick={() => open({ kind: 'workspace' }, setFiles)}
            />
          </div>
        </SettingRow>
        <SettingRow label={de.settings.excel}>
          <Button
            variant="secondary"
            size="sm"
            icon="folder-open"
            label={de.settings.excelShow}
            disabled={!cfg.settings.excelExists}
            disabledReason={de.settings.excelMissing}
            testid="excel-show"
            onclick={() => open({ kind: 'excel' }, setFiles)}
          />
        </SettingRow>
        <SettingRow label={de.settings.txt} hint={de.settings.txtCount(cfg.settings.txtFiles)}>
          <div class="buttons">
            <Button
              variant="secondary"
              size="sm"
              icon="refresh-cw"
              label={de.settings.txtRewrite}
              loading={busy === 'rewrite'}
              disabled={run.active}
              disabledReason={de.settings.running}
              testid="txt-rewrite"
              onclick={rewrite}
            />
            <Button
              variant="ghost"
              size="sm"
              icon="trash-2"
              label={de.settings.txtClear}
              disabled={cfg.settings.txtFiles === 0}
              disabledReason={de.settings.txtNone}
              testid="txt-clear"
              onclick={() => (confirmClear = true)}
            />
          </div>
        </SettingRow>
        {@render note(filesNote, 'files-note')}
      </Card>
    </section>

    <section class="section" data-testid="settings-care">
      <h2 class="heading">{de.settings.maintenance}</h2>
      <Card padding="md">
        {#if cfg.resetReport}
          <Notice
            tone={cfg.resetReport.failed > 0 ? 'warning' : 'success'}
            variant="inline"
            text={cfg.resetReport.failed > 0
              ? `${de.settings.resetDone} ${de.settings.resetFailed(cfg.resetReport.failed)}`
              : de.settings.resetDone}
            testid="reset-report"
          />
        {/if}
        <SettingRow label={de.settings.fullMailbox} hint={de.settings.fullMailboxHint}>
          <Button
            variant="secondary"
            size="sm"
            icon="mail"
            label={de.settings.fullMailboxAction}
            disabled={run.active || !cfg.mailbox.user}
            disabledReason={run.active ? de.settings.running : de.toolbar.needsMailbox}
            testid="full-mailbox"
            onclick={() => (confirmFull = true)}
          />
        </SettingRow>
        <SettingRow label={de.settings.logs} hint={cfg.logDir}>
          <Button
            variant="ghost"
            size="sm"
            icon="folder-open"
            label={de.common.openFolder}
            onclick={() => open({ kind: 'logDir' }, setCare)}
          />
        </SettingRow>
        <SettingRow label={de.settings.data} hint={cfg.dataDir}>
          <Button
            variant="ghost"
            size="sm"
            icon="copy"
            label={de.settings.copyPath}
            onclick={() => void copyPath(cfg.dataDir)}
          />
        </SettingRow>
        <SettingRow label={de.settings.reset} hint={de.settings.resetHint}>
          <Button
            variant="danger"
            size="sm"
            icon="rotate-ccw"
            label={de.settings.resetAction}
            disabled={run.active}
            disabledReason={de.settings.running}
            testid="reset"
            onclick={() => (confirmReset = true)}
          />
        </SettingRow>
        {@render note(careNote, 'care-note')}
      </Card>
    </section>
  {/if}
</div>

<Dialog
  bind:open={confirmRemove}
  variant="danger"
  heading={de.settings.removeMailbox}
  text={de.settings.removeMailboxText}
  confirmLabel={de.common.remove}
  busy={busy === 'mailbox'}
  testid="dialog-remove-mailbox"
  onconfirm={removeMailbox}
/>
<Dialog
  bind:open={confirmClear}
  variant="danger"
  heading={de.settings.txtClearHeading}
  text={de.settings.txtClearText}
  confirmLabel={de.settings.txtClear}
  busy={busy === 'clear'}
  testid="dialog-clear"
  onconfirm={clear}
/>
<Dialog
  bind:open={confirmFull}
  heading={de.settings.fullMailboxHeading}
  text={de.settings.fullMailboxText}
  confirmLabel={de.settings.fullMailboxAction}
  testid="dialog-full-mailbox"
  onconfirm={readAll}
/>
<Dialog
  bind:open={confirmReset}
  variant="danger"
  heading={de.settings.resetHeading}
  text={de.settings.resetText}
  confirmLabel={de.settings.resetAction}
  busy={busy === 'reset'}
  testid="dialog-reset"
  onconfirm={reset}
/>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
    max-width: var(--reader-width);
    margin: 0 auto;
    padding: var(--space-32) var(--space-32) var(--space-64);
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-xl);
  }

  .lead {
    margin-bottom: var(--space-16);
    color: var(--text-muted);
    font: var(--type-md);
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-8);
  }

  .skeleton {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
</style>
