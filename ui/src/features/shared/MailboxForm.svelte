<!--
  Connect a Gmail mailbox: address and app password, Enter saves, Esc cancels. Errors land
  at the field they belong to (empty fields are said before anything is sent); when Gmail
  refuses the pair, both fields are marked, the password shakes once and the one sentence
  stands above the button. The password never leaves this form except to save_mailbox (it
  goes straight into the OS keychain). Under both fields one line says what an app password
  needs, with the two pages in the order she needs them: the 2-step verification, then the
  app password. Save and cancel follow the OS like the dialogs (save first on Windows, last
  on macOS) and sit at the trailing edge like every save/cancel pair; the single "Verbinden"
  of the first run stays under the fields.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Field from '$components/Field.svelte';
  import Notice from '$components/Notice.svelte';
  import TextField from '$components/TextField.svelte';
  import { t } from '$lib/i18n/t';
  import { errorText } from '$lib/i18n/texts';
  import { formKeys } from '$lib/input/input';
  import { invoke, IpcError } from '$lib/ipc/api';
  import { primaryFirst } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import { onMount } from 'svelte';

  interface Props {
    /**
     * Label of the save button, the primary of the form ("Verbinden" first, "Speichern" when
     * changing, next to cancel). "Abrufen" lives in the Jobs view, so it never competes.
     */
    saveLabel: string;
    /** Only when changing an existing mailbox. */
    oncancel?: (() => void) | null;
    onsaved?: (() => void) | null;
    /** The caret starts in the address (the first run, where this form is the first step). */
    autofocus?: boolean;
  }
  let { saveLabel, oncancel = null, onsaved = null, autofocus = false }: Props = $props();

  const id = $props.id();
  const saveFirst = primaryFirst();
  let user = $state(app.state?.mailbox.user ?? '');
  let password = $state('');
  let busy = $state(false);
  let userError = $state<string | null>(null);
  let passwordError = $state<string | null>(null);
  /** Gmail refused the pair: both fields are marked, the sentence stands once above. */
  let refused = $state(false);
  let formError = $state<string | null>(null);
  let passwordField = $state<TextField | null>(null);

  const focus = (field: 'user' | 'password'): void =>
    document.getElementById(`${id}-${field}`)?.focus();

  onMount(() => {
    if (autofocus && document.activeElement === document.body) focus('user');
  });

  async function save(): Promise<void> {
    if (busy) return;
    userError = passwordError = formError = null;
    refused = false;
    // Empty fields are said at once, both of them, without asking Gmail.
    if (user.trim() === '') userError = t.settings.addressMissing;
    if (password.trim() === '') passwordError = t.settings.passwordMissing;
    if (userError !== null || passwordError !== null) {
      focus(userError !== null ? 'user' : 'password');
      return;
    }
    busy = true;
    try {
      await invoke('save_mailbox', { user: user.trim(), password });
      password = '';
      await app.load();
      if (oncancel) toasts.show(t.toast.mailboxSaved);
      onsaved?.();
    } catch (error) {
      const kind = error instanceof IpcError ? error.kind : null;
      const reason = error instanceof IpcError ? error.params.reason : null;
      if (kind === 'mailAuth') {
        // Gmail refuses address or password: both are marked, the password shakes once.
        refused = true;
        formError = errorText(error);
        passwordField?.shake();
      } else if (reason === 'mailAddress' || kind === 'mailNotGmail') {
        userError = errorText(error);
      } else if (reason === 'appPassword') {
        passwordError = errorText(error);
      } else {
        formError = errorText(error);
      }
    } finally {
      busy = false;
    }
  }

  function openPage(kind: 'appPasswordPage' | 'twoStepPage'): void {
    invoke('open_target', { target: { kind } }).catch(
      (error: unknown) => (formError = errorText(error)),
    );
  }
</script>

<div
  class="form"
  data-testid="mailbox-form"
  use:formKeys={oncancel
    ? { save: () => void save(), cancel: oncancel }
    : { save: () => void save() }}
>
  <div class="fields">
    <Field label={t.settings.address} for="{id}-user" error={userError}>
      <TextField
        id="{id}-user"
        bind:value={user}
        invalid={userError !== null || refused}
        describedby="{id}-user-message"
        testid="mailbox-user"
      />
    </Field>
    <Field
      label={t.settings.password}
      for="{id}-password"
      hint={t.settings.passwordHint}
      error={passwordError}
    >
      <TextField
        bind:this={passwordField}
        id="{id}-password"
        kind="password"
        bind:value={password}
        invalid={passwordError !== null || refused}
        describedby="{id}-password-message"
        testid="mailbox-password"
      />
    </Field>
  </div>
  <!-- What an app password needs, and the two pages in the order she needs them. -->
  <div class="help">
    <p>{t.settings.twoStep}</p>
    <div class="links">
      <Button
        variant="link"
        size="sm"
        icon="external-link"
        external
        label={t.settings.twoStepAction}
        testid="two-step"
        onclick={() => openPage('twoStepPage')}
      />
      <Button
        variant="link"
        size="sm"
        icon="external-link"
        external
        label={t.settings.createPassword}
        testid="create-password"
        onclick={() => openPage('appPasswordPage')}
      />
    </div>
  </div>
  {#if formError}
    <Notice tone="danger" variant="inline" text={formError} testid="mailbox-error" />
  {/if}
  <div class="actions" class:pair={oncancel !== null}>
    {#snippet dismiss()}
      {#if oncancel}
        <Button
          variant="secondary"
          label={t.common.cancel}
          disabled={busy}
          testid="mailbox-cancel"
          onclick={() => oncancel?.()}
        />
      {/if}
    {/snippet}
    {#if !saveFirst}{@render dismiss()}{/if}
    <Button
      variant="primary"
      label={saveLabel}
      loading={busy}
      testid="mailbox-save"
      onclick={() => void save()}
    />
    {#if saveFirst}{@render dismiss()}{/if}
  </div>
</div>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    max-width: var(--form-width);
    container-type: inline-size;
  }

  .help {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .links {
    display: flex;
    flex-wrap: wrap;
    column-gap: var(--space-16);
  }

  /* Address and password side by side where there is room (the first run stays short). */
  .fields {
    display: grid;
    grid-template-columns: 1fr;
    align-items: start;
    gap: var(--space-16);
  }

  @container (width >= 520px) {
    .fields {
      grid-template-columns: 1fr 1fr;
    }
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8);
  }

  /* Save and cancel at the trailing edge, like the profile's save bar and every dialog. */
  .pair {
    justify-content: flex-end;
  }
</style>
