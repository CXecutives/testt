<!--
  Connect a Gmail mailbox: address and app password, Enter saves, Esc cancels. Errors land
  at the field they belong to; the password never leaves this form except to save_mailbox
  (it goes straight into the OS keychain). Save and cancel follow the OS like the dialogs:
  save first on Windows, cancel first (save last) on macOS; the row stays left-aligned.
  When Gmail refuses the password, its field shakes once and the error rises in below it.
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

  interface Props {
    /**
     * Label of the save button, the primary of the form ("Verbinden" first, "Speichern" when
     * changing, next to cancel). "Abrufen" lives in the Jobs view, so it never competes.
     */
    saveLabel: string;
    /** Only when changing an existing mailbox. */
    oncancel?: (() => void) | null;
    onsaved?: (() => void) | null;
  }
  let { saveLabel, oncancel = null, onsaved = null }: Props = $props();

  const id = $props.id();
  const saveFirst = primaryFirst();
  let user = $state(app.state?.mailbox.user ?? '');
  let password = $state('');
  let busy = $state(false);
  let userError = $state<string | null>(null);
  let passwordError = $state<string | null>(null);
  let formError = $state<string | null>(null);
  let passwordField = $state<TextField | null>(null);

  async function save(): Promise<void> {
    if (busy) return;
    busy = true;
    userError = passwordError = formError = null;
    try {
      await invoke('save_mailbox', { user: user.trim(), password });
      password = '';
      await app.load();
      if (oncancel) toasts.show(t.toast.saved);
      onsaved?.();
    } catch (error) {
      const reason = error instanceof IpcError ? error.params.reason : null;
      if (
        reason === 'mailAddress' ||
        (error instanceof IpcError && error.kind === 'mailNotGmail')
      ) {
        userError = errorText(error);
      } else if (
        reason === 'appPassword' ||
        (error instanceof IpcError && error.kind === 'mailAuth')
      ) {
        passwordError = errorText(error);
        // Gmail refused the password: the field it was typed in shakes once.
        if (error instanceof IpcError && error.kind === 'mailAuth') passwordField?.shake();
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
        invalid={userError !== null}
        describedby="{id}-user-message"
        testid="mailbox-user"
      />
    </Field>
    <Field
      label={t.settings.password}
      for="{id}-password"
      hint={t.settings.passwordHint}
      action={{
        label: t.settings.createPassword,
        icon: 'external-link',
        testid: 'create-password',
        onclick: () => openPage('appPasswordPage'),
      }}
      error={passwordError}
    >
      <TextField
        bind:this={passwordField}
        id="{id}-password"
        kind="password"
        bind:value={password}
        invalid={passwordError !== null}
        describedby="{id}-password-message"
        testid="mailbox-password"
      />
    </Field>
  </div>
  <!-- Before an app password exists, Google wants 2-step verification: said once, with the way there. -->
  <p class="two-step">
    <span>{t.settings.twoStep}</span>
    <Button
      variant="link"
      size="sm"
      icon="external-link"
      external
      label={t.settings.twoStepAction}
      testid="two-step"
      onclick={() => openPage('twoStepPage')}
    />
  </p>
  {#if formError}
    <Notice tone="danger" variant="inline" text={formError} testid="mailbox-error" />
  {/if}
  <div class="actions">
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

  .two-step {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    column-gap: var(--space-8);
    color: var(--text-muted);
    font: var(--type-sm);
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
</style>
