<!--
  Einstellungen (centred 720): Postfach, Abruf, Portale, Dateien, Wartung - each a card of
  setting rows - and "Alles zurücksetzen" alone on the last card, apart from the harmless
  rows. Every action answers where it happened (a note rises in there, and fades when it
  goes); dialogs only to confirm, and a
  confirmed action that fails closes its dialog so the note beside the action can say why.
  Switches move at once and are their own answer (no toast). The dry run changes nothing,
  so what it cannot do is locked with that reason instead of failing.
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
  import { platform } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import MailboxForm from '../shared/MailboxForm.svelte';
  import PortalCard from './PortalCard.svelte';

  type Feedback = { tone: NoticeTone; text: string } | null;

  const id = $props.id();
  const cfg = $derived(app.state);
  let editing = $state(false);
  let mailboxNote = $state<Feedback>(null);
  let fetchNote = $state<Feedback>(null);
  let filesNote = $state<Feedback>(null);
  let careNote = $state<Feedback>(null);
  let resetNote = $state<Feedback>(null);
  let confirmRemove = $state(false);
  let confirmClear = $state(false);
  let confirmFull = $state(false);
  let confirmReset = $state(false);
  let busy = $state<string | null>(null);
  /** Only the answer to the latest save may replace the state (quick double flips). */
  let saves = 0;
  /** What the dry run cannot do, and why (the backend would refuse it). */
  const dryRun = $derived(cfg?.dryRun ?? false);
  const dryRunReason = de.error.text('dryRun', {});
  const lockedReason = $derived(dryRun ? dryRunReason : de.settings.running);

  async function act(
    name: string,
    note: (f: Feedback) => void,
    work: () => Promise<Feedback>,
    close: (() => void) | null = null,
  ): Promise<void> {
    busy = name;
    note(null);
    try {
      note(await work());
    } catch (error) {
      note({ tone: 'danger', text: errorText(error) });
    } finally {
      busy = null;
      close?.();
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
  const setReset = (f: Feedback): void => void (resetNote = f);

  function removeMailbox(): void {
    void act(
      'mailbox',
      setMailbox,
      async () => {
        await invoke('remove_mailbox');
        await app.load();
        return null;
      },
      () => (confirmRemove = false),
    );
  }

  /** The switch moves at once; a failure puts it back (reload) and says why below it. */
  function autoFetch(on: boolean): Promise<void> {
    const save = ++saves;
    if (app.state) app.state.autoFetchOnStart = on;
    return act('fetch', setFetch, async () => {
      try {
        const next = await invoke('save_settings', {
          patch: { portals: [], autoFetchOnStart: on },
        });
        if (save === saves) app.set(next);
      } catch (error) {
        void app.load();
        throw error;
      }
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
    void act(
      'clear',
      setFiles,
      async () => {
        const result = await invoke('clear_txt');
        await app.load();
        if (result.failed.length > 0) {
          return { tone: 'warning', text: de.settings.txtFailed(result.failed.length) };
        }
        toasts.show(de.settings.txtCleared(result.removed));
        return null;
      },
      () => (confirmClear = false),
    );
  }

  function readAll(): void {
    confirmFull = false;
    void run.start({ kind: 'fullMailbox' }).then((started) => {
      if (started) navigation.go('jobs');
      else setCare({ tone: 'danger', text: run.startError ?? de.run.failed });
    });
  }

  /** On success the app restarts empty; a failure closes the dialog and says why here. */
  function reset(): void {
    void act(
      'reset',
      setReset,
      async () => {
        await invoke('reset_all');
        return null;
      },
      () => (confirmReset = false),
    );
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

<!-- A note rises in where its action happened and fades when it goes (Notice, never at mount). -->
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
      <Card padding={cfg.mailbox.user && !editing ? 'rows' : 'md'}>
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
                disabled={dryRun}
                disabledReason={dryRunReason}
                testid="mailbox-change"
                onclick={() => (editing = true)}
              />
              <Button
                variant="ghost"
                size="sm"
                icon="trash-2"
                label={de.common.remove}
                disabled={run.active || dryRun}
                disabledReason={lockedReason}
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
      <Card padding="rows">
        <SettingRow
          label={de.settings.autoFetch}
          hint={de.settings.autoFetchHint}
          for="{id}-auto-fetch"
        >
          <Toggle
            id="{id}-auto-fetch"
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
      <Card padding="rows">
        <SettingRow label={de.settings.workspace} hint={cfg.settings.workspace} copy>
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
            variant="ghost"
            size="sm"
            icon="folder-open"
            label={de.settings.excelShow[platform()]}
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
              disabled={run.active || dryRun}
              disabledReason={lockedReason}
              testid="txt-rewrite"
              onclick={rewrite}
            />
            <Button
              variant="ghost"
              size="sm"
              icon="trash-2"
              label={de.settings.txtClear}
              disabled={cfg.settings.txtFiles === 0 || dryRun}
              disabledReason={dryRun ? dryRunReason : de.settings.txtNone}
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
      <Card padding="rows">
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
        <SettingRow label={de.settings.logs} hint={cfg.logDir} copy>
          <Button
            variant="ghost"
            size="sm"
            icon="folder-open"
            label={de.common.openFolder}
            onclick={() => open({ kind: 'logDir' }, setCare)}
          />
        </SettingRow>
        <SettingRow label={de.settings.data} hint={cfg.dataDir} copy>
          <Button
            variant="ghost"
            size="sm"
            icon="copy"
            label={de.settings.copyPath}
            onclick={() => void copyPath(cfg.dataDir)}
          />
        </SettingRow>
        {@render note(careNote, 'care-note')}
      </Card>
    </section>

    <!-- The one destructive action on its own, last, apart from the harmless rows. -->
    <Card padding="rows" testid="settings-reset">
      {#if cfg.resetReport}
        <Notice
          tone={cfg.resetReport.failed > 0 ? 'warning' : 'success'}
          variant="inline"
          text={cfg.resetReport.failed > 0
            ? de.settings.resetPartly(cfg.resetReport.failed)
            : de.settings.resetDone}
          testid="reset-report"
        />
      {/if}
      <SettingRow label={de.settings.reset} hint={de.settings.resetHint}>
        <Button
          variant="secondary"
          size="sm"
          icon="rotate-ccw"
          label={de.settings.resetAction}
          disabled={run.active || dryRun}
          disabledReason={lockedReason}
          testid="reset"
          onclick={() => (confirmReset = true)}
        />
      </SettingRow>
      {@render note(resetNote, 'reset-note')}
    </Card>
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
    max-width: calc(var(--reader-width) + 2 * var(--pane-padding));
    margin: 0 auto;
    padding: var(--pane-padding) var(--pane-padding) var(--space-64);
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
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
