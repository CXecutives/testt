<!--
  Modal question with at most two actions: confirm | danger. A plain scrim (no blur: a
  blurred backdrop over the whole window drops frames in the web view); the dialog rises in
  180 ms (ease-out) and leaves in 100 ms. Esc cancels (formKeys); Tab and Enter work inside.
  Pressing inside and releasing on the scrim keeps it open; only the left button counts.
-->
<script lang="ts">
  import { de } from '$lib/i18n/de';
  import { formKeys } from '$lib/input/input';
  import { dialogIn, dialogOut, scrim } from '$lib/motion/transitions';
  import type { Action } from 'svelte/action';
  import Button from './Button.svelte';

  interface Props {
    open: boolean;
    variant?: 'confirm' | 'danger';
    heading: string;
    text: string;
    confirmLabel: string;
    cancelLabel?: string;
    busy?: boolean;
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

  /** Danger dialogs start on "cancel", confirm dialogs on the confirm button. */
  const focusFirst: Action<HTMLElement, 'confirm' | 'danger'> = (node, kind) => {
    const buttons = node.querySelectorAll<HTMLButtonElement>('.actions button');
    const target = kind === 'danger' ? buttons[0] : buttons[buttons.length - 1];
    queueMicrotask(() => target?.focus());
  };
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
      data-testid={testid ?? undefined}
      in:dialogIn
      out:dialogOut
      use:formKeys={{ cancel }}
      use:focusFirst={variant}
    >
      <h2 class="heading" id="{id}-heading">{heading}</h2>
      <p class="text" id="{id}-text">{text}</p>
      <div class="actions">
        <Button variant="secondary" label={cancelLabel} disabled={busy} onclick={cancel} />
        <Button
          variant={variant === 'danger' ? 'danger' : 'primary'}
          label={confirmLabel}
          loading={busy}
          onclick={onconfirm}
        />
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
