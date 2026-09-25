<!--
  Einstellungen (centred 720): Postfach, Abruf, Portale, Dateien, Sprache, Wartung - each a
  card of setting rows - and "Alles zurücksetzen" alone on the last card, apart from the
  harmless rows. Sprache switches the whole app at once (Deutsch, English). Every action
  answers where it happened (a note rises in there, and fades when it goes); dialogs only to
  confirm, and a confirmed action that fails closes its dialog so the note beside the action
  can say why.
  Switches move at once and are their own answer (no toast). The dry run changes nothing,
  and a run (a fetch, or the rescore after a profile change) holds the mailbox, the folder
  and the files, so what they cannot do is locked with the reason of that run instead of
  failing. The Postfach says when the last fetch could not reach Gmail or Gmail refused the
  password, instead of "Verbunden": a red badge like the sidebar's status, and a sentence
  under the row only where it adds the cause or the next step.
  Every path row works the same: the path is text to select and copy, the folder opens with
  "Ordner öffnen", the Excel file with "Öffnen".
-->
<script lang="ts">
  import Badge from '$components/Badge.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Dialog from '$components/Dialog.svelte';
  import Notice, { type NoticeTone } from '$components/Notice.svelte';
  import Segmented from '$components/Segmented.svelte';
  import SettingRow from '$components/SettingRow.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import Toggle from '$components/Toggle.svelte';
  import { language } from '$lib/i18n/language.svelte';
  import { t } from '$lib/i18n/t';
  import { errorText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { Language, OpenTarget, SettingsPatch } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import { tick } from 'svelte';
  import MailboxForm from '../shared/MailboxForm.svelte';
  import PortalCard from './PortalCard.svelte';

  type Feedback = { tone: NoticeTone; text: string } | null;

  /** The app's languages, each named in its own words. */
  const LANGUAGES: readonly Language[] = ['de', 'en'];

  const cfg = $derived(app.state);
  let editing = $state(false);
  let mailboxNote = $state<Feedback>(null);
  let fetchNote = $state<Feedback>(null);
  let filesNote = $state<Feedback>(null);
  let languageNote = $state<Feedback>(null);
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
  const dryRunReason = $derived(t.error.text('dryRun', {}));
  /** Why a locked action waits: the dry run, or the run in progress (a fetch or a rescore). */
  const lockedReason = $derived(dryRun ? dryRunReason : run.busyText);

  /** Fetch failures that are about the mailbox itself (not a cancel, not a missing one). */
  const MAIL_FAILURES: readonly string[] = [
    'mailConnect',
    'mailAuth',
    'mailTimeout',
    'mailLost',
    'mailNotGmail',
    'mailServer',
  ];
  /** A mailbox saved here was just checked against Gmail: the old failure is past. */
  let mailboxSaved = $state(false);
  const mailFailure = $derived.by(() => {
    const outcome = cfg?.lastRun?.outcome;
    if (mailboxSaved || outcome?.kind !== 'failed') return null;
    return MAIL_FAILURES.includes(outcome.error.kind) ? outcome.error : null;
  });
  /** The sentence under the row: what to do when Gmail refused the password, else the cause
   *  where it says more than the badge ("Gmail ist nicht erreichbar" is the badge itself). */
  const mailFailureText = $derived(
    mailFailure === null || mailFailure.kind === 'mailConnect'
      ? null
      : mailFailure.kind === 'mailAuth'
        ? t.settings.mailRefused
        : t.error.text(mailFailure.kind, mailFailure.params),
  );

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

  /** Ändern and Entfernen of the connected mailbox. */
  let mailboxButtons = $state<HTMLElement | null>(null);

  /** The change form closes: the focus it held goes back to "Ändern", as a dialog's goes back
   *  to its opener (only when it fell to the page, never taken from elsewhere). */
  async function closeForm(): Promise<void> {
    editing = false;
    await tick();
    if (document.activeElement === document.body) {
      mailboxButtons?.querySelector<HTMLElement>('button')?.focus();
    }
  }

  const setMailbox = (f: Feedback): void => void (mailboxNote = f);
  const setFetch = (f: Feedback): void => void (fetchNote = f);
  const setFiles = (f: Feedback): void => void (filesNote = f);
  const setLanguageNote = (f: Feedback): void => void (languageNote = f);
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

  /** Days after which old jobs archive themselves, and the trash empties itself, when on. */
  const AUTO_ARCHIVE_DAYS = 30;
  const AUTO_EMPTY_TRASH_DAYS = 30;

  /** The switch moves at once; a failure puts it back (reload) and says why below it. */
  function autoFetch(on: boolean): Promise<void> {
    if (app.state) app.state.autoFetchOnStart = on;
    return saveFetch({
      autoFetchOnStart: on,
      autoArchiveDays: null,
      autoEmptyTrashDays: null,
      language: null,
    });
  }

  function autoArchive(on: boolean): Promise<void> {
    const days = on ? AUTO_ARCHIVE_DAYS : 0;
    if (app.state) app.state.autoArchiveDays = days;
    return saveFetch({
      autoFetchOnStart: null,
      autoArchiveDays: days,
      autoEmptyTrashDays: null,
      language: null,
    });
  }

  function autoEmptyTrash(on: boolean): Promise<void> {
    const days = on ? AUTO_EMPTY_TRASH_DAYS : 0;
    if (app.state) app.state.autoEmptyTrashDays = days;
    return saveFetch({
      autoFetchOnStart: null,
      autoArchiveDays: null,
      autoEmptyTrashDays: days,
      language: null,
    });
  }

  /**
   * The language switches the whole page at once, before the backend has stored it; a
   * failure switches back and says why below it. Excel file and overview follow at the next
   * fetch.
   */
  function chooseLanguage(next: Language): Promise<void> {
    const before = language.current;
    language.set(next);
    if (app.state) app.state.language = next;
    return saveFetch(
      { autoFetchOnStart: null, autoArchiveDays: null, autoEmptyTrashDays: null, language: next },
      setLanguageNote,
      () => language.set(before),
    );
  }

  function saveFetch(
    change: Omit<SettingsPatch, 'portals'>,
    note: (f: Feedback) => void = setFetch,
    undo: () => void = () => {},
  ): Promise<void> {
    const save = ++saves;
    return act(change.language === null ? 'fetch' : 'language', note, async () => {
      try {
        const next = await invoke('save_settings', { patch: { portals: [], ...change } });
        if (save === saves) app.set(next);
      } catch (error) {
        undo();
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
        return { tone: 'danger', text: t.error.text(result.error.kind, result.error.params) };
      if (result.txtFailed > 0)
        return { tone: 'warning', text: t.settings.txtFailed(result.txtFailed) };
      return { tone: 'success', text: t.settings.txtWritten(result.txtWritten) };
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
          return { tone: 'warning', text: t.settings.txtFailed(result.failed.length) };
        }
        return { tone: 'success', text: t.settings.txtCleared(result.removed) };
      },
      () => (confirmClear = false),
    );
  }

  function readAll(): void {
    confirmFull = false;
    void run.start({ kind: 'fullMailbox' }).then((started) => {
      if (started) navigation.go('jobs');
      else setCare({ tone: 'danger', text: run.startError ?? t.run.failed });
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
      <Notice tone="info" text={t.settings.dryRun} />
    {/if}

    <section class="section" data-testid="settings-mailbox">
      <h2 class="heading">{t.settings.mailbox}</h2>
      <Card padding={cfg.mailbox.user && !editing ? 'rows' : 'md'}>
        {#if cfg.mailbox.user && !editing}
          <SettingRow label={cfg.mailbox.user} hint={t.settings.vault[cfg.mailbox.vault]}>
            {#snippet badges()}
              {#if mailFailure}
                <Badge
                  label={mailFailure.kind === 'mailAuth'
                    ? t.settings.refused
                    : t.settings.unreachable}
                  tone="danger"
                  icon="triangle-alert"
                />
              {:else}
                <Badge label={t.settings.connected} tone="success" icon="check" />
              {/if}
            {/snippet}
            <div class="buttons" bind:this={mailboxButtons}>
              <Button
                variant="secondary"
                size="sm"
                icon="pencil"
                label={t.common.change}
                disabled={run.active || dryRun}
                disabledReason={lockedReason}
                testid="mailbox-change"
                onclick={() => (editing = true)}
              />
              <Button
                variant="ghost"
                size="sm"
                icon="trash-2"
                label={t.common.remove}
                disabled={run.active || dryRun}
                disabledReason={lockedReason}
                testid="mailbox-remove"
                onclick={() => (confirmRemove = true)}
              />
            </div>
          </SettingRow>
        {:else}
          {#if !cfg.mailbox.user}
            <p class="lead">{t.settings.notConnected}</p>
          {/if}
          <!-- "Ändern" gives way to the form, which takes the caret; closing it gives it back. -->
          <MailboxForm
            saveLabel={cfg.mailbox.user ? t.common.save : t.settings.connect}
            autofocus={editing}
            oncancel={cfg.mailbox.user ? () => void closeForm() : null}
            onsaved={() => {
              mailboxSaved = true;
              void closeForm();
            }}
          />
        {/if}
        {#if mailFailureText && !editing}
          <Notice tone="danger" variant="inline" text={mailFailureText} testid="mailbox-failure" />
        {/if}
        {#if cfg.mailbox.error}
          <Notice
            tone="danger"
            variant="inline"
            text={t.error.text(cfg.mailbox.error.kind, cfg.mailbox.error.params)}
          />
        {/if}
        {@render note(mailboxNote, 'mailbox-note')}
      </Card>
    </section>

    <section class="section" data-testid="settings-fetch">
      <h2 class="heading">{t.settings.fetch}</h2>
      <Card padding="rows">
        <SettingRow
          label={t.settings.autoFetch}
          hint={t.settings.autoFetchHint}
          for="switch-auto-fetch"
        >
          <Toggle
            id="switch-auto-fetch"
            checked={cfg.autoFetchOnStart}
            label={t.settings.autoFetch}
            testid="toggle-auto-fetch"
            onchange={autoFetch}
          />
        </SettingRow>
        <SettingRow
          label={t.settings.autoArchive}
          hint={t.settings.autoArchiveHint}
          for="switch-auto-archive"
        >
          <Toggle
            id="switch-auto-archive"
            checked={cfg.autoArchiveDays > 0}
            label={t.settings.autoArchive}
            testid="toggle-auto-archive"
            onchange={autoArchive}
          />
        </SettingRow>
        <SettingRow
          label={t.settings.autoEmptyTrash}
          hint={t.settings.autoEmptyTrashHint}
          for="switch-auto-empty-trash"
        >
          <Toggle
            id="switch-auto-empty-trash"
            checked={cfg.autoEmptyTrashDays > 0}
            label={t.settings.autoEmptyTrash}
            testid="toggle-auto-empty-trash"
            onchange={autoEmptyTrash}
          />
        </SettingRow>
        {@render note(fetchNote, 'fetch-note')}
      </Card>
    </section>

    <section class="section" data-testid="settings-portals">
      <h2 class="heading">{t.settings.portals}</h2>
      {#each cfg.portals as portal (portal.portal)}
        <PortalCard {portal} />
      {/each}
    </section>

    <section class="section" data-testid="settings-files">
      <h2 class="heading">{t.settings.files}</h2>
      <Card padding="rows">
        <SettingRow label={t.settings.workspace} hint={cfg.settings.workspace} copy>
          {#snippet badges()}
            {#if cfg.settings.workspaceIsDefault}
              <Badge label={t.settings.workspaceDefault} />
            {/if}
          {/snippet}
          <div class="buttons">
            <Button
              variant="secondary"
              size="sm"
              icon="pencil"
              label={t.common.change}
              loading={busy === 'workspace'}
              disabled={run.active || dryRun}
              disabledReason={lockedReason}
              testid="workspace-change"
              onclick={pickWorkspace}
            />
            <Button
              variant="ghost"
              size="sm"
              icon="folder-open"
              label={t.common.openFolder}
              testid="workspace-open"
              onclick={() => open({ kind: 'workspace' }, setFiles)}
            />
          </div>
        </SettingRow>
        <!-- Where the Excel file is (or will be), to find it later or to tell someone. -->
        <SettingRow label={t.settings.excel} hint={cfg.settings.excelPath} copy testid="excel">
          <Button
            variant="ghost"
            size="sm"
            icon="file-spreadsheet"
            label={t.common.open}
            disabled={!cfg.settings.excelExists}
            disabledReason={t.settings.excelMissing}
            testid="excel-open"
            onclick={() => open({ kind: 'excel' }, setFiles)}
          />
        </SettingRow>
        <SettingRow label={t.settings.txt} hint={t.settings.txtCount(cfg.settings.txtFiles)}>
          <div class="buttons">
            <Button
              variant="secondary"
              size="sm"
              icon="refresh-cw"
              label={t.settings.txtRewrite}
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
              label={t.settings.txtClear}
              disabled={run.active || dryRun || cfg.settings.txtFiles === 0}
              disabledReason={run.active || dryRun ? lockedReason : t.settings.txtNone}
              testid="txt-clear"
              onclick={() => (confirmClear = true)}
            />
          </div>
        </SettingRow>
        {@render note(filesNote, 'files-note')}
      </Card>
    </section>

    <section class="section" data-testid="settings-language">
      <h2 class="heading">{t.settings.language}</h2>
      <Card padding="rows">
        <SettingRow label={t.settings.languageLabel} hint={t.settings.languageHint}>
          <Segmented
            size="sm"
            options={LANGUAGES.map((id) => ({ id, label: t.settings.languageName[id] }))}
            value={language.current}
            label={t.settings.language}
            testid="language"
            onchange={(next) => void chooseLanguage(next)}
          />
        </SettingRow>
        {@render note(languageNote, 'language-note')}
      </Card>
    </section>

    <section class="section" data-testid="settings-care">
      <h2 class="heading">{t.settings.maintenance}</h2>
      <Card padding="rows">
        <SettingRow label={t.settings.fullMailbox} hint={t.settings.fullMailboxHint}>
          <Button
            variant="secondary"
            size="sm"
            icon="mail"
            label={t.settings.fullMailboxAction}
            disabled={run.active || !cfg.mailbox.user}
            disabledReason={run.active ? run.busyText : t.toolbar.needsMailbox}
            testid="full-mailbox"
            onclick={() => (confirmFull = true)}
          />
        </SettingRow>
        <SettingRow label={t.settings.logs} hint={cfg.logDir} copy>
          <Button
            variant="ghost"
            size="sm"
            icon="folder-open"
            label={t.common.openFolder}
            testid="logs-open"
            onclick={() => open({ kind: 'logDir' }, setCare)}
          />
        </SettingRow>
        <SettingRow label={t.settings.data} hint={cfg.dataDir} copy>
          <Button
            variant="ghost"
            size="sm"
            icon="folder-open"
            label={t.common.openFolder}
            testid="data-open"
            onclick={() => open({ kind: 'dataDir' }, setCare)}
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
            ? t.settings.resetPartly(cfg.resetReport.failed)
            : t.settings.resetDone}
          testid="reset-report"
        />
      {/if}
      <SettingRow label={t.settings.reset} hint={t.settings.resetHint}>
        <Button
          variant="secondary"
          size="sm"
          icon="rotate-ccw"
          label={t.settings.resetAction}
          warns
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
  heading={t.settings.removeMailbox}
  text={t.settings.removeMailboxText}
  confirmLabel={t.common.remove}
  busy={busy === 'mailbox'}
  testid="dialog-remove-mailbox"
  onconfirm={removeMailbox}
/>
<Dialog
  bind:open={confirmClear}
  variant="danger"
  heading={t.settings.txtClearHeading}
  text={t.settings.txtClearText}
  confirmLabel={t.settings.txtClear}
  busy={busy === 'clear'}
  testid="dialog-clear"
  onconfirm={clear}
/>
<Dialog
  bind:open={confirmFull}
  heading={t.settings.fullMailboxHeading}
  text={t.settings.fullMailboxText}
  confirmLabel={t.settings.fullMailboxAction}
  testid="dialog-full-mailbox"
  onconfirm={readAll}
/>
<Dialog
  bind:open={confirmReset}
  variant="danger"
  heading={t.settings.resetHeading}
  text={t.settings.resetText}
  confirmLabel={t.settings.resetAction}
  busy={busy === 'reset'}
  testid="dialog-reset"
  onconfirm={reset}
/>

<style>
  .page {
    container-type: inline-size;
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
