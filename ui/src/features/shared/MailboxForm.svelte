<!--
  Connect a Gmail mailbox: address and app password, Enter saves, Esc cancels. Errors land
  at the field they belong to; the password never leaves this form except to save_mailbox
  (it goes straight into the OS keychain).
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Field from '$components/Field.svelte';
  import Notice from '$components/Notice.svelte';
  import TextField from '$components/TextField.svelte';
  import { de } from '$lib/i18n/de';
  import { errorText } from '$lib/i18n/texts';
  import { formKeys } from '$lib/input/input';
  import { invoke, IpcError } from '$lib/ipc/api';
  import { app } from '$lib/state/app.svelte';
  import { toasts } from '$lib/state/toasts.svelte';

  interface Props {
    /**
     * Label of the save button ("Verbinden" first: the primary while nothing can be fetched;
     * "Speichern" when changing, next to cancel, as a secondary: "Abrufen" is the primary then).
     */
    saveLabel: string;
    /** Only when changing an existing mailbox. */
    oncancel?: (() => void) | null;
    onsaved?: (() => void) | null;
  }
  let { saveLabel, oncancel = null, onsaved = null }: Props = $props();

  const id = $props.id();
  let user = $state(app.state?.mailbox.user ?? '');
  let password = $state('');
  let busy = $state(false);
  let userError = $state<string | null>(null);
  let passwordError = $state<string | null>(null);
  let formError = $state<string | null>(null);

  async function save(): Promise<void> {
    if (busy) return;
    busy = true;
    userError = passwordError = formError = null;
    try {
      await invoke('save_mailbox', { user: user.trim(), password });
      password = '';
      await app.load();
      if (oncancel) toasts.show(de.toast.saved);
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
      } else {
        formError = errorText(error);
      }
    } finally {
      busy = false;
    }
  }

  function openPasswordPage(): void {
    invoke('open_target', { target: { kind: 'appPasswordPage' } }).catch(
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
    <Field label={de.settings.address} for="{id}-user" error={userError}>
      <TextField
        id="{id}-user"
        bind:value={user}
        invalid={userError !== null}
        describedby="{id}-user-message"
        testid="mailbox-user"
      />
    </Field>
    <Field
      label={de.settings.password}
      for="{id}-password"
      hint={de.settings.passwordHint}
      action={{
        label: de.settings.createPassword,
        icon: 'external-link',
        testid: 'create-password',
        onclick: openPasswordPage,
      }}
      error={passwordError}
    >
      <TextField
        id="{id}-password"
        kind="password"
        bind:value={password}
        invalid={passwordError !== null}
        describedby="{id}-password-message"
        testid="mailbox-password"
      />
    </Field>
  </div>
  {#if formError}
    <Notice tone="danger" variant="inline" text={formError} testid="mailbox-error" />
  {/if}
  <div class="actions">
    <Button
      variant={oncancel ? 'secondary' : 'primary'}
      label={saveLabel}
      loading={busy}
      testid="mailbox-save"
      onclick={() => void save()}
    />
    {#if oncancel}
      <Button variant="secondary" label={de.common.cancel} onclick={() => oncancel?.()} />
    {/if}
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
