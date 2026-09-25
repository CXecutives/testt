<!--
  Profil (centred 720): the profile itself as a form. Without a profile an empty state with
  the three ways in (a new form, from a CV with an AI, an existing file); a file that no
  longer reads says so in the same place, with its folder at hand. With a profile its head
  (the person, an honest quality, what the app reads, the file actions) and the form with
  the save bar. A chosen file and an AI's answer fill the form for review (an answer for the
  stored profile updates it); nothing is stored before "Speichern". Leaving the view or
  closing the window with unsaved changes asks once. Removing the profile can be taken back
  for a moment (a toast with "Rückgängig").
-->
<script lang="ts">
  import Dialog from '$components/Dialog.svelte';
  import { t } from '$lib/i18n/t';
  import { errorText, warningText } from '$lib/i18n/texts';
  import { IpcError, invoke, onCloseRequested } from '$lib/ipc/api';
  import type { Notice } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { navigation, type ViewId } from '$lib/state/navigation.svelte';
  import {
    editor,
    fieldProblems,
    localQuality,
    sameForm,
    type FieldError,
  } from '$lib/state/profile.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import { onMount, tick, untrack } from 'svelte';
  import ProfileEditor from './ProfileEditor.svelte';
  import ProfileHeader from './ProfileHeader.svelte';
  import ProfilePaste from './ProfilePaste.svelte';
  import ProfileStart from './ProfileStart.svelte';

  const profile = $derived(app.state?.profile ?? null);
  const stored = $derived(profile?.form ?? null);
  const rescoring = $derived((run.active && run.kind === 'rescore') || (profile?.pending ?? 0) > 0);

  /** The prompts for a new profile and for an update of the stored one, loaded ahead so that
   *  copying needs no wait. */
  let prompts = $state<{ create: string | null; update: string | null }>({
    create: null,
    update: null,
  });
  let copied = $state(false);
  let busy = $state<'pick' | 'save' | 'remove' | 'paste' | null>(null);
  /** A failure is said when it shows, so it follows a switch of the language. */
  type Words = () => string;
  let note = $state<Words | null>(null);
  let saveNote = $state<Words | null>(null);
  let pasteError = $state<Words | null>(null);
  /** The steps with an AI update the stored profile (else they make a new one). */
  let updating = $state(false);
  const prompt = $derived(updating ? prompts.update : prompts.create);
  let saved = $state(false);
  /** A value the backend refused on the last save, said at its field. */
  let fieldError = $state<FieldError | null>(null);
  // The outcome of a save stands until the next change.
  $effect(() => {
    if (editor.dirty) saved = false;
  });
  // A refused value is said until the form changes.
  $effect(() => {
    void JSON.stringify(editor.after);
    untrack(() => (fieldError = null));
  });
  const result = $derived(
    !saved
      ? null
      : !rescoring && (app.state?.counts.inbox ?? 0) > 0
        ? t.profile.rescored
        : t.profile.saved,
  );
  /** During setup, a saved profile leads on to the first fetch (once, in the save bar). */
  const next = $derived(
    saved && app.state?.firstRun && app.hasProfile ? () => void onward() : null,
  );

  /** The button goes with the view: the setup page's next action takes the focus. */
  async function onward(): Promise<void> {
    navigation.go('jobs');
    await tick();
    document
      .querySelector<HTMLElement>('[data-testid="first-run"] [aria-current="step"] button')
      ?.focus();
  }
  let confirmRemove = $state(false);
  /** Where the user wanted to go with unsaved changes (a view, or closing the window). */
  let leaving = $state<ViewId | 'close' | null>(null);

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

  // The update prompt carries the stored profile: it is loaded again whenever that changes.
  $effect(() => {
    const form = stored;
    untrack(() => {
      prompts.update = null;
      if (form === null) return;
      invoke('profile_prompt', { update: true })
        .then((text) => {
          if (stored === form) prompts.update = text;
        })
        .catch(() => undefined);
    });
  });

  // The window asks before it closes while the form holds unsaved changes (main.rs).
  const dirty = $derived(editor.dirty);
  $effect(() => {
    const on = dirty;
    untrack(() => void invoke('set_unsaved', { on }).catch(() => undefined));
  });

  /** The window is to close with unsaved changes: the page says it is here, then asks. */
  async function closeRequested(): Promise<void> {
    await invoke('set_unsaved', { on: editor.dirty }).catch(() => undefined);
    if (editor.dirty) leaving = 'close';
    else closeWindow();
  }

  function closeWindow(): void {
    invoke('close_window').catch((error: unknown) => (note = () => errorText(error)));
  }

  onMount(() => {
    invoke('profile_prompt', { update: false })
      .then((text) => (prompts.create = text))
      .catch(() => undefined);
    const release = navigation.guard((next) => {
      if (!editor.dirty) return true;
      leaving = next;
      return false;
    });
    const stopClose = onCloseRequested(() => void closeRequested());
    return () => {
      release();
      stopClose();
      void invoke('set_unsaved', { on: false }).catch(() => undefined);
      // An untouched new form or the steps with an AI start over next time.
      if (!editor.dirty && editor.origin !== 'stored') editor.close();
    };
  });

  /** The state again after a change; `false` when it could not be loaded. */
  async function reload(): Promise<boolean> {
    const loaded = await app.load();
    void jobs.load(true);
    void jobs.loadOverview();
    return loaded !== null;
  }

  async function pick(): Promise<void> {
    busy = 'pick';
    note = null;
    try {
      const draft = await invoke('pick_profile');
      if (draft !== null) editor.take(draft, 'file');
    } catch (error) {
      note = () => errorText(error);
    } finally {
      busy = null;
    }
  }

  function openFolder(): void {
    note = null;
    invoke('open_target', { target: { kind: 'profileDir' } }).catch(
      (error: unknown) => (note = () => errorText(error)),
    );
  }

  async function copyPrompt(): Promise<void> {
    const kind = updating ? 'update' : 'create';
    try {
      const text = prompts[kind] ?? (await invoke('profile_prompt', { update: updating }));
      prompts[kind] = text;
      await navigator.clipboard.writeText(text);
      copied = true;
    } catch {
      copied = false;
    }
  }

  /** The prompt goes to the clipboard first, so the steps show whether it got there; with an
   *  answer pasted earlier the clipboard is left alone ("Erneut kopieren" copies). For the
   *  stored profile it is the update prompt, and the answer updates the profile. */
  async function fromCv(): Promise<void> {
    pasteError = null;
    updating = editor.origin === 'stored' && stored !== null;
    if (editor.answer.trim() === '') await copyPrompt();
    else copied = true;
    editor.pasting = true;
    // The prompt is on the clipboard: the next step is pasting the answer.
    await caretTo('paste-answer');
  }

  /** A new form: the caret goes into its first field. */
  async function create(): Promise<void> {
    editor.create();
    await caretTo('profile-name-field');
  }

  /** The caret into a field that has just appeared. */
  async function caretTo(testid: string): Promise<void> {
    await tick();
    document.querySelector<HTMLElement>(`[data-testid="${testid}"]`)?.focus();
  }

  async function takeAnswer(answer: string): Promise<void> {
    busy = 'paste';
    pasteError = null;
    try {
      const draft = await invoke('parse_profile', { text: answer, update: updating });
      if (updating && stored !== null) editor.update(draft, stored);
      else editor.take(draft, 'answer');
      editor.answer = '';
    } catch (error) {
      pasteError = () => errorText(error);
    } finally {
      busy = null;
    }
  }

  let panel = $state<{
    ready: () => boolean;
    focusField: (field: string) => Promise<boolean>;
  } | null>(null);

  /** A value out of range: the field (and row) it names, and the limit where one is. */
  function refused(
    error: unknown,
  ): { field: string; row: number | null; max: number | null } | null {
    if (!(error instanceof IpcError) || error.params.reason !== 'profileValue') return null;
    const { field, row, max } = error.params;
    return typeof field === 'string'
      ? {
          field,
          row: typeof row === 'number' ? row : null,
          max: typeof max === 'number' ? max : null,
        }
      : null;
  }

  /** `true` when the profile is saved. */
  async function save(): Promise<boolean> {
    busy = 'save';
    saveNote = null;
    fieldError = null;
    try {
      const info = await editor.save();
      // The saved profile is the answer of the save: a state that could not be loaded
      // again never puts the old values back next to "Gespeichert".
      if (!(await reload()) && app.state) app.state.profile = info;
      const form = info.form ?? app.state?.profile?.form;
      if (form) editor.edit(form);
      else editor.close();
      saved = true;
      return true;
    } catch (error) {
      const at = refused(error);
      if (at === null) {
        saveNote = () => errorText(error);
      } else {
        // Said at its field without its name again; the save bar says it where the field
        // is not on the page.
        fieldError = {
          field: at.field,
          row: at.row,
          text: () => (at.max === null ? t.profile.field.refused : t.profile.field.atMost(at.max)),
        };
        if (!(await panel?.focusField(at.field))) saveNote = () => errorText(error);
      }
      return false;
    } finally {
      busy = null;
    }
  }

  /** Back to the stored profile, or (a new form, a draft) to the ways in, whose first one
   *  takes the focus of the gone form. */
  function discard(): void {
    saveNote = null;
    fieldError = null;
    editor.discard(stored);
    if (editor.origin === null) void caretTo('profile-create');
  }

  async function remove(): Promise<void> {
    busy = 'remove';
    note = null;
    try {
      const removed = await invoke('remove_profile');
      editor.close();
      saved = false;
      await reload();
      if (removed) {
        toasts.show(t.profile.removed, 'success', {
          label: t.common.undo,
          onclick: () => void restore(),
        });
      }
    } catch (error) {
      note = () => errorText(error);
    } finally {
      // The dialog closes either way; a failure shows next to the file.
      confirmRemove = false;
      busy = null;
    }
  }

  /** "Rückgängig" of a removal: the backup becomes the profile again. */
  async function restore(): Promise<void> {
    note = null;
    try {
      await invoke('restore_profile');
      await reload();
    } catch (error) {
      note = () => errorText(error);
    }
  }

  /** Leaving without saving. */
  function leave(): void {
    const next = leaving;
    leaving = null;
    editor.discard(stored);
    if (next === 'close') closeWindow();
    else if (next !== null) navigation.go(next, true);
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
    if (!done || next === null) return;
    if (next === 'close') closeWindow();
    else navigation.go(next, true);
  }

  // ------------------------------------------------------------ what the form says

  /** What the engine reads: a draft's own, else the stored profile's (an update from a CV is
   *  saved into it); a new form has none yet. */
  const understood = $derived(
    editor.origin === 'file' || editor.origin === 'answer'
      ? editor.understood
      : editor.origin === 'new'
        ? null
        : (profile?.understood ?? null),
  );
  const warnings = $derived<readonly Notice[]>(understood?.warnings ?? []);
  const problems = $derived(
    editor.origin === null
      ? []
      : fieldProblems(warnings, editor.before, editor.after, editor.cleared),
  );
  /** The quality of the form as it is: the engine's for what it read, followed while the
   *  form changes; a new form says nothing before it has something. */
  const local = $derived(
    editor.origin === null
      ? null
      : localQuality(understood?.competenceCount ?? 0, editor.before, editor.after),
  );
  const quality = $derived.by(() => {
    if (local === null) return null;
    // Unchanged, the engine's own word counts.
    const unchanged = sameForm(editor.before, editor.after);
    if (editor.origin === 'new' && unchanged) return null;
    if (editor.origin === 'stored' && unchanged) return profile?.quality ?? local.quality;
    if ((editor.origin === 'file' || editor.origin === 'answer') && unchanged) {
      return editor.quality ?? local.quality;
    }
    return local.quality;
  });
  const hasCompetences = $derived(editor.after.competences.some((row) => row.name.trim() !== ''));
  /** Terms for the match: the engine's count while nothing changed, else followed. */
  const terms = $derived(
    understood === null || local === null
      ? null
      : sameForm(editor.before, editor.after)
        ? understood.competenceCount
        : local.terms,
  );
  /** The reasons of "Etwas prüfen": each value that does not read, a region rule that stays
   *  off. */
  const checks = $derived(
    [
      ...problems.map((problem) =>
        problem.entry
          ? problem.field === 'focus'
            ? t.profile.field.unreadableFocus(problem.value)
            : t.profile.field.unreadableRole(problem.value)
          : (warningText(problem.notice) ?? ''),
      ),
      ...(warnings.some((w) => w.code === 'regionWithoutPlaces') &&
      editor.after.criteria.permanentPlaces.length === 0 &&
      editor.after.criteria.permanentRemoteMin !== null
        ? [t.profile.warning.regionWithoutPlaces]
        : []),
    ].filter((text) => text !== ''),
  );
  /** Said in the head: what the form cannot change (keys of the file the app does not read). */
  const HEAD = new Set(['ignoredKeys']);
  /** Said elsewhere: the quality at the competences, empty criteria at their section, a
   *  value at its field, the Schwerpunkte taken over at the Schwerpunkte. */
  const ELSEWHERE = new Set([
    'fewCompetences',
    'noCompetences',
    'noCriteria',
    'criterionNotUnderstood',
    'availabilityNotUnderstood',
    'regionWithoutPlaces',
    'focusTrimmed',
  ]);
  const headWarnings = $derived(warnings.filter((w) => HEAD.has(w.code) || !ELSEWHERE.has(w.code)));
</script>

<div class="page" class:editing={editor.origin !== null && !editor.pasting} data-testid="profile">
  {#if app.state === null}
    <!-- The shell shows nothing until the state is known. -->
  {:else if editor.pasting}
    <ProfilePaste
      heading={updating ? t.profile.updateFromCv : t.profile.fromCv}
      {prompt}
      {copied}
      busy={busy === 'paste'}
      error={pasteError?.() ?? null}
      bind:answer={editor.answer}
      oncopy={() => void copyPrompt()}
      ontake={(answer) => void takeAnswer(answer)}
      oncancel={() => (editor.pasting = false)}
    />
  {:else if editor.origin === null}
    <div class="empty">
      <ProfileStart
        heading={profile?.parseError ? t.overview.profileUnreadable : t.profile.none}
        text={profile?.parseError
          ? `${t.error.text(profile.parseError.kind, profile.parseError.params)} ${t.profile.replaces}`
          : t.overview.noProfileText}
        picking={busy === 'pick'}
        unreadable={profile?.parseError !== null && profile?.parseError !== undefined}
        note={note?.() ?? null}
        oncreate={() => void create()}
        onfromcv={() => void fromCv()}
        onpick={() => void pick()}
        onopenfolder={openFolder}
      />
    </div>
  {:else}
    <ProfileHeader
      origin={editor.origin}
      {profile}
      {quality}
      competences={hasCompetences}
      {terms}
      focus={editor.after.focus.length}
      packs={understood?.packs ?? []}
      {checks}
      warnings={headWarnings}
      {rescoring}
      dirty={editor.dirty}
      picking={busy === 'pick'}
      note={note?.() ?? null}
      onpick={() => void pick()}
      onremove={() => (confirmRemove = true)}
      onfromcv={() => void fromCv()}
      onopenfolder={openFolder}
    />
    <ProfileEditor
      bind:this={panel}
      {quality}
      {problems}
      {warnings}
      {understood}
      {fieldError}
      busy={busy === 'save'}
      note={saveNote?.() ?? null}
      {result}
      onnext={next}
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
  /* The same 32 between the head and the first section as between all sections. */
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
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
