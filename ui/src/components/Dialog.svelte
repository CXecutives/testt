<!--
  Modal question with at most two actions: confirm | danger. A plain scrim (no blur: a
  blurred backdrop over the whole window drops frames in the web view); the dialog rises in
  180 ms (ease-out) and leaves in 100 ms. It holds the focus like a native one (input.ts):
  Tab and Shift+Tab cycle through its buttons, Esc cancels wherever the focus is, Enter
  presses the focused button or, on the dialog itself, its default button (cancel for
  danger, confirm otherwise). A click on its text keeps the focus inside; on close the
  focus goes back to where it was. A failure of the action shows inside the dialog
  (`error`), never behind the scrim.
  Pressing inside and releasing on the scrim keeps it open; only the left button counts.
  The buttons follow the OS: the action first on Windows, last (right) on macOS.
-->
<script lang="ts">
  import { de } from '$lib/i18n/de';
  import { formKeys } from '$lib/input/input';
  import { primaryFirst } from '$lib/platform';
  import { dialogIn, dialogOut, scrim } from '$lib/motion/transitions';
  import type { Action } from 'svelte/action';
  import Button from './Button.svelte';
  import Notice from './Notice.svelte';

  interface Props {
    open: boolean;
    variant?: 'confirm' | 'danger';
    heading: string;
    text: string;
    confirmLabel: string;
    cancelLabel?: string;
    busy?: boolean;
    /** Why the action failed (shown inside the dialog, which stays open). */
    error?: string | null;
    testid?: string | null;
    onconfirm: () => void;
    oncancel?: () => void;
  }

  let {
    open = $bindable(),
    variant = 'confirm',
    heading,
    text,
    confirmLabel,
    cancelLabel = de.common.cancel,
    busy = false,
    error = null,
    testid = null,
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

  $effect(() => {
    if (open || opener === null) return;
    const back = opener;
    opener = null;
    if (back.isConnected) back.focus();
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
            label={cancelLabel}
            disabled={busy}
            testid="dialog-cancel"
            onclick={cancel}
          />
        {/snippet}
        {#if !actionFirst}{@render dismiss()}{/if}
        <Button
          variant={variant === 'danger' ? 'danger' : 'primary'}
          label={confirmLabel}
          loading={busy}
          testid="dialog-confirm"
          onclick={confirm}
        />
        {#if actionFirst}{@render dismiss()}{/if}
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

  .danger .heading {
    color: var(--danger-strong);
  }

  .text {
    color: var(--text-muted);
    font: var(--type-body);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-12);
    margin-top: var(--space-12);
  }
</style>
