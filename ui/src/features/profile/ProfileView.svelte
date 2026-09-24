<!--
  Profil (centred 720): the profile itself as a form. Without a profile an empty state with
  the three ways in (a new form, from a CV with Claude, an existing file); a file that no
  longer reads says so in the same place. With a profile its head (file, quality, what the
  app understood, file actions) and the form with the save bar. A chosen file and Claude's
  answer fill the form for review; nothing is stored before "Speichern". Leaving the view
  with unsaved changes asks once.
-->
<script lang="ts">
  import Dialog from '$components/Dialog.svelte';
  import { t } from '$lib/i18n/t';
  import { errorText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { navigation, type ViewId } from '$lib/state/navigation.svelte';
  import { editor, sameForm } from '$lib/state/profile.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import { onMount, untrack } from 'svelte';
  import ProfileEditor from './ProfileEditor.svelte';
  import ProfileHeader from './ProfileHeader.svelte';
  import ProfilePaste from './ProfilePaste.svelte';
  import ProfileStart from './ProfileStart.svelte';

  const profile = $derived(app.state?.profile ?? null);
  const stored = $derived(profile?.form ?? null);
  const rescoring = $derived((run.active && run.kind === 'rescore') || (profile?.pending ?? 0) > 0);

  let prompt = $state<string | null>(null);
  let copied = $state(false);
  let busy = $state<'pick' | 'save' | 'remove' | 'paste' | null>(null);
  let note = $state<string | null>(null);
  let saveNote = $state<string | null>(null);
  let pasteError = $state<string | null>(null);
  let saved = $state(false);
  let confirmRemove = $state(false);
  let leaving = $state<ViewId | null>(null);

  // The stored profile fills the form while nothing unsaved is in it (also after a save).
  $effect(() => {
    const form = stored;
    untrack(() => {
      if (editor.dirty || editor.origin === 'new' || editor.pasting) return;
      if (form === null) {
        if (editor.origin === 'stored') editor.close();
      } else if (editor.origin !== 'stored' || !sameForm(editor.before, form)) {
        editor.edit(form);
      }
    });
  });

  onMount(() => {
    invoke('profile_prompt')
      .then((text) => (prompt = text))
      .catch(() => (prompt = null));
    const release = navigation.guard((next) => {
      if (!editor.dirty) return true;
      leaving = next;
      return false;
    });
    return () => {
      release();
      // An untouched new form or the steps with Claude start over next time.
      if (!editor.dirty && editor.origin !== 'stored') editor.close();
    };
  });

  async function reload(): Promise<void> {
    await app.load();
    void jobs.load(true);
    void jobs.loadOverview();
  }

  async function pick(): Promise<void> {
    busy = 'pick';
    note = null;
    try {
      const draft = await invoke('pick_profile');
      if (draft !== null) editor.take(draft, 'file');
    } catch (error) {
      note = errorText(error);
    } finally {
      busy = null;
    }
  }

  async function copyPrompt(): Promise<void> {
    try {
      const text = prompt ?? (await invoke('profile_prompt'));
      prompt = text;
      await navigator.clipboard.writeText(text);
      copied = true;
    } catch {
      copied = false;
    }
  }

  /** The request goes to the clipboard first, so the steps show whether it got there. */
  async function fromCv(): Promise<void> {
    pasteError = null;
    await copyPrompt();
    editor.pasting = true;
  }

  async function takeAnswer(answer: string): Promise<void> {
    busy = 'paste';
    pasteError = null;
    try {
      editor.take(await invoke('parse_profile', { text: answer }), 'answer');
    } catch (error) {
      pasteError = errorText(error);
    } finally {
      busy = null;
    }
  }

  let panel = $state<{ ready: () => boolean } | null>(null);

  /** `true` when the profile is saved. */
  async function save(): Promise<boolean> {
    busy = 'save';
    saveNote = null;
    try {
      const info = await editor.save();
      await reload();
      const form = app.state?.profile?.form ?? info.form;
      if (form) editor.edit(form);
      else editor.close();
      saved = true;
      toasts.show(t.profile.saved);
      return true;
    } catch (error) {
      saveNote = errorText(error);
      return false;
    } finally {
      busy = null;
    }
  }

  function discard(): void {
    saveNote = null;
    editor.discard(stored);
  }

  async function remove(): Promise<void> {
    busy = 'remove';
    note = null;
    try {
      await invoke('remove_profile');
      editor.close();
      saved = false;
      await reload();
    } catch (error) {
      note = errorText(error);
    } finally {
      // The dialog closes either way; a failure shows next to the file.
      confirmRemove = false;
      busy = null;
    }
  }

  /** Leaving without saving. */
  function leave(): void {
    const next = leaving;
    leaving = null;
    editor.discard(stored);
    if (next !== null) navigation.go(next, true);
  }

  /** Saving, then leaving; a date that does not read or a failed save keeps the view. */
  async function saveAndLeave(): Promise<void> {
    const next = leaving;
    if (panel !== null && !panel.ready()) {
      leaving = null;
      return;
    }
    const done = await save();
    leaving = null;
    if (done && next !== null) navigation.go(next, true);
  }

  const quality = $derived(
    editor.origin === 'stored' ? (profile?.quality ?? null) : editor.quality,
  );
</script>

<div class="page" class:editing={editor.origin !== null && !editor.pasting} data-testid="profile">
  {#if app.state === null}
    <!-- The shell shows nothing until the state is known. -->
  {:else if editor.pasting}
    <ProfilePaste
      {copied}
      busy={busy === 'paste'}
      error={pasteError}
      oncopy={() => void copyPrompt()}
      ontake={(answer) => void takeAnswer(answer)}
      oncancel={() => (editor.pasting = false)}
    />
  {:else if editor.origin === null}
    <div class="empty">
      <ProfileStart
        heading={profile?.parseError ? t.profile.parseError : t.profile.none}
        text={profile?.parseError
          ? t.error.text(profile.parseError.kind, profile.parseError.params)
          : t.profile.noneText}
        picking={busy === 'pick'}
        {note}
        oncreate={() => editor.create()}
        onfromcv={() => void fromCv()}
        onpick={() => void pick()}
      />
    </div>
  {:else}
    <ProfileHeader
      origin={editor.origin}
      {profile}
      {quality}
      {rescoring}
      rescored={saved && !rescoring && (app.state?.counts.all ?? 0) > 0}
      dirty={editor.dirty}
      picking={busy === 'pick'}
      {note}
      onpick={() => void pick()}
      onremove={() => (confirmRemove = true)}
      onfromcv={() => void fromCv()}
      onnext={saved && app.state?.firstRun ? () => navigation.go('jobs') : null}
    />
    <ProfileEditor
      bind:this={panel}
      {quality}
      busy={busy === 'save'}
      note={saveNote}
      onsave={() => void save()}
      ondiscard={discard}
    />
  {/if}
</div>

<Dialog
  bind:open={confirmRemove}
  variant="danger"
  heading={t.profile.removeHeading}
  text={t.profile.removeText}
  confirmLabel={t.profile.remove}
  busy={busy === 'remove'}
  testid="dialog-remove-profile"
  onconfirm={() => void remove()}
/>

<Dialog
  open={leaving !== null}
  heading={t.profile.leaveHeading}
  text={t.profile.leaveText}
  confirmLabel={t.profile.save}
  altLabel={t.profile.discard}
  busy={busy === 'save'}
  testid="dialog-leave-profile"
  onconfirm={() => void saveAndLeave()}
  onalt={leave}
  oncancel={() => (leaving = null)}
/>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-24);
    max-width: calc(var(--reader-width) + 2 * var(--pane-padding));
    min-height: 100%;
    margin: 0 auto;
    padding: var(--pane-padding) var(--pane-padding) var(--space-48);
  }

  /* The save bar ends the page at the bottom edge. */
  .editing {
    padding-bottom: 0;
  }

  /* The empty state sits at about 38 % of the height (spacers 38 : 62), not dead centre. */
  .empty {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
  }

  .empty::before,
  .empty::after {
    content: '';
  }

  .empty::before {
    flex: 38;
  }

  .empty::after {
    flex: 62;
  }
</style>
