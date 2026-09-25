<!--
  The toast stack, bottom right (mounted once in App and the gallery): short confirmations
  that rise in (150 ms) and slide out sideways (100 ms), at most three, the stack moving up
  as one leaves. A 2 px navy line at the bottom drains over the toast's lifetime and stops
  while the toast is hovered (so does its timer); under reduced motion there is no line.
  The check of a success draws itself once as the toast appears. Closable; an undo of what
  the user just did sits before the close button. A merged toast ("2 Jobs archiviert.")
  cross-fades its sentence (100 ms) and starts its line again.
-->
<script lang="ts">
  import { t } from '$lib/i18n/t';
  import { fade, flip, toastIn, toastOut } from '$lib/motion/transitions';
  import { toasts } from '$lib/state/toasts.svelte';
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';

  let hovered = $state<number | null>(null);
</script>

<div class="stack" role="status" aria-live="polite" data-testid="toasts">
  {#each toasts.items as toast (toast.id)}
    <div
      class="toast {toast.tone}"
      class:paused={hovered === toast.id}
      class:undo={toast.action !== null}
      role="group"
      data-testid="toast"
      animate:flip
      in:toastIn
      out:toastOut
      onpointerenter={() => {
        hovered = toast.id;
        toasts.pause(toast.id);
      }}
      onpointerleave={() => {
        hovered = null;
        toasts.resume(toast.id);
      }}
    >
      <span class="icon"
        ><Icon name={toast.tone === 'success' ? 'circle-check' : 'info'} size="sm" /></span
      >
      {#key toast.text}<span class="text" in:fade>{toast.text}</span>{/key}
      {#if toast.action}
        {@const action = toast.action}
        <Button
          variant="ghost"
          size="sm"
          label={action.label}
          testid="toast-action"
          onclick={() => {
            action.onclick();
            toasts.dismiss(toast.id);
          }}
        />
      {/if}
      <Button
        variant="ghost"
        size="sm"
        icon="x"
        iconOnly
        label={t.common.hide}
        onclick={() => toasts.dismiss(toast.id)}
      />
      <span class="life" aria-hidden="true"></span>
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    right: var(--space-24);
    bottom: var(--space-24);
    z-index: var(--z-overlay);
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-8);
    pointer-events: none;
  }

  .toast {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-8);
    width: var(--toast-width);
    max-width: calc(100vw - 2 * var(--space-24));
    overflow: hidden;
    padding: var(--space-6) var(--space-6) var(--space-6) var(--space-16);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-lg);
    background-color: var(--surface);
    box-shadow: var(--sh-pop);
    color: var(--text);
    font: var(--type-md);
    pointer-events: auto;
  }

  .icon {
    display: inline-flex;
    flex: none;
  }

  .success .icon {
    color: var(--success-strong);
  }

  /* The check draws itself once when the toast appears. */
  .success .icon :global(path) {
    stroke-dasharray: var(--draw-length);
    animation: draw var(--dur-slow) var(--ease-out) var(--dur-instant) both;
  }

  .info .icon {
    color: var(--info);
  }

  .text {
    flex: 1;
    min-width: 0;
  }

  /* The lifetime line: it drains from the right over --dur-toast, paused while hovered. */
  .life {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    height: var(--focus-width);
    background-color: var(--toast-bar);
    transform-origin: left center;
    animation: drain var(--dur-toast) linear forwards;
  }

  /* A toast with an undo stays longer; its line drains as long. */
  .undo .life {
    animation-duration: var(--dur-toast-undo);
  }

  .paused .life,
  :global(:root[data-window='inactive']) .life {
    animation-play-state: paused;
  }

  :global(:root[data-motion='reduce']) .life {
    display: none;
  }
</style>
