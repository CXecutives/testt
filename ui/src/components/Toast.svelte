<!--
  The toast stack, bottom right (mounted once in App and the gallery): short confirmations
  that rise in and fade out, at most three, paused while hovered, closable.
-->
<script lang="ts">
  import { de } from '$lib/i18n/de';
  import { fade, flip, rise } from '$lib/motion/transitions';
  import { toasts } from '$lib/state/toasts.svelte';
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
</script>

<div class="stack" role="status" aria-live="polite" data-testid="toasts">
  {#each toasts.items as toast (toast.id)}
    <div
      class="toast {toast.tone}"
      role="group"
      data-testid="toast"
      animate:flip
      in:rise={{ distance: 'md' }}
      out:fade
      onpointerenter={() => toasts.pause(toast.id)}
      onpointerleave={() => toasts.resume(toast.id)}
    >
      <span class="icon"
        ><Icon name={toast.tone === 'success' ? 'circle-check' : 'info'} size="sm" /></span
      >
      <span class="text">{toast.text}</span>
      <Button
        variant="ghost"
        size="sm"
        icon="x"
        iconOnly
        label={de.common.hide}
        onclick={() => toasts.dismiss(toast.id)}
      />
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
    display: flex;
    align-items: center;
    gap: var(--space-8);
    width: var(--toast-width);
    max-width: calc(100vw - 2 * var(--space-24));
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

  .info .icon {
    color: var(--info);
  }

  .text {
    flex: 1;
    min-width: 0;
  }
</style>
