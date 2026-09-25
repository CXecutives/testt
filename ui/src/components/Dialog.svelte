<!--
  Modal question with at most two actions: confirm | danger, plus an optional third one
  (`altLabel`, a secondary button, as "Verwerfen" in "Änderungen speichern?"). A plain
  scrim (no blur: a
  blurred backdrop over the whole window drops frames in the web view); the dialog rises in
  180 ms (ease-out) and leaves in 100 ms. It holds the focus like a native one (input.ts):
  Tab and Shift+Tab cycle through its buttons, Esc cancels wherever the focus is, Enter
  presses the focused button or, on the dialog itself, its default button (cancel for
  danger, confirm otherwise). The focus starts on that default button; it shows the focus
  ring only when the keyboard opened the dialog or moves in it, never after a click. The
  heading stays ink also in a danger dialog (its red button says it). A click on its text
  keeps the focus inside; on close the focus goes back to where it was. A failure of the
  action shows inside the dialog (`error`), never behind the scrim. The toasts lie below the scrim and wait while it is
  open.
  Pressing inside and releasing on the scrim keeps it open; only the left button counts.
  The buttons follow the OS: the action first on Windows (then the third action, then
  cancel), last (right) on macOS with the third action on the far left.
-->
<script lang="ts">
  import { t } from '$lib/i18n/t';
  import { formKeys } from '$lib/input/input';
  import { primaryFirst } from '$lib/platform';
  import { dialogIn, dialogOut, scrim } from '$lib/motion/transitions';
  import { toasts } from '$lib/state/toasts.svelte';
  import { untrack } from 'svelte';
  import type { Action } from 'svelte/action';
  import Button from './Button.svelte';
  import Notice from './Notice.svelte';

  interface Props {
    open: boolean;
    variant?: 'confirm' | 'danger';
    heading: string;
    text: string;
    /** The bare verb of the heading ("Postfach entfernen?": Entfernen; "Ganzes Postfach
     *  lesen?": Lesen), the same pattern in every dialog. */
    confirmLabel: string;
    cancelLabel?: string;
    busy?: boolean;
    /** Why the action failed (shown inside the dialog, which stays open). */
    error?: string | null;
    testid?: string | null;
    /** A third action next to confirm and cancel (secondary). */
    altLabel?: string | null;
    onalt?: () => void;
    onconfirm: () => void;
    oncancel?: () => void;
  }

  let {
    open = $bindable(),
    variant = 'confirm',
    heading,
    text,
    confirmLabel,
    cancelLabel,
    busy = false,
    error = null,
    testid = null,
    altLabel = null,
    onalt,
    onconfirm,
    oncancel,
  }: Props = $props();

  const id = $props.id();
  let pressedOnScrim = false;

  function cancel(): void {
    if (busy) return;
    open = false;
    oncancel?.();
  }

  function confirm(): void {
    if (!busy) onconfirm();
  }

  const actionFirst = primaryFirst();

  /** Where the focus was before the dialog opened; it goes back there on close. */
  let opener: HTMLElement | null = null;

  /** Danger dialogs start on "cancel", confirm dialogs on the confirm button. */
  const focusFirst: Action<HTMLElement, 'confirm' | 'danger'> = (node, kind) => {
    const before = document.activeElement;
    opener = before instanceof HTMLElement && before !== document.body ? before : null;
    const role = kind === 'danger' ? 'dialog-cancel' : 'dialog-confirm';
    const target = node.querySelector<HTMLButtonElement>(`[data-testid="${role}"]`);
    queueMicrotask(() => target?.focus());
  };

  // While the dialog is open the toasts behind its scrim wait (an undo keeps its time).
  $effect(() => (open ? untrack(() => toasts.hold()) : undefined));

  // On close the focus goes back where it was, unless the action moved it on purpose (a
  // failed save puts the caret into the field it names).
  $effect(() => {
    if (open || opener === null) return;
    const back = opener;
    opener = null;
    const now = document.activeElement;
    const moved =
      now instanceof HTMLElement &&
      now !== document.body &&
      now.closest('[role="alertdialog"]') === null;
    if (back.isConnected && !moved) back.focus();
  });
</script>

{#if open}
  <div
    class="scrim"
    transition:scrim
    onpointerdown={(event) =>
      (pressedOnScrim = event.button === 0 && event.target === event.currentTarget)}
    onclick={(event) => {
      if (pressedOnScrim && event.target === event.currentTarget) cancel();
      pressedOnScrim = false;
    }}
    role="presentation"
  >
    <div
      class="dialog {variant}"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="{id}-heading"
      aria-describedby="{id}-text"
      tabindex="-1"
      data-testid={testid ?? undefined}
      in:dialogIn
      out:dialogOut
      use:formKeys={{ cancel, save: variant === 'danger' ? cancel : confirm }}
      use:focusFirst={variant}
    >
      <h2 class="heading" id="{id}-heading">{heading}</h2>
      <p class="text" id="{id}-text">{text}</p>
      {#if error}
        <Notice tone="danger" variant="inline" text={error} testid="dialog-error" />
      {/if}
      <div class="actions">
        {#snippet dismiss()}
          <Button
            variant="secondary"
            label={cancelLabel ?? t.common.cancel}
            disabled={busy}
            isDefault={variant === 'danger'}
            testid="dialog-cancel"
            onclick={cancel}
          />
        {/snippet}
        {#snippet alt()}
          {#if altLabel}
            <span class:apart={!actionFirst}>
              <Button
                variant="secondary"
                label={altLabel}
                disabled={busy}
                testid="dialog-alt"
                onclick={() => onalt?.()}
              />
            </span>
          {/if}
        {/snippet}
        {#if !actionFirst}{@render alt()}{@render dismiss()}{/if}
        <Button
          variant={variant === 'danger' ? 'danger' : 'primary'}
          label={confirmLabel}
          loading={busy}
          isDefault={variant !== 'danger'}
          testid="dialog-confirm"
          onclick={confirm}
        />
        {#if actionFirst}{@render alt()}{@render dismiss()}{/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    z-index: var(--z-overlay);
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-16);
    background-color: var(--scrim);
  }

  .dialog {
    position: relative;
    z-index: var(--z-dialog);
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    width: var(--dialog-width);
    max-width: 100%;
    padding: var(--space-24);
    border-radius: var(--radius-dialog);
    background-color: var(--surface);
    box-shadow: var(--sh-pop);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-xl);
    letter-spacing: var(--tracking-tight);
  }

  .text {
    color: var(--text-muted);
    font: var(--type-body);
    text-wrap: balance;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-12);
    margin-top: var(--space-12);
  }

  /* On macOS the third action stands apart on the left. */
  .apart {
    margin-right: auto;
  }
</style>
