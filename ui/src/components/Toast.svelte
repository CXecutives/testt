<!--
  The toast stack, bottom right (mounted once in App and the gallery): short confirmations
  that rise in (150 ms) and slide out sideways (100 ms), at most three, the stack moving up
  as one leaves. A plain toast stays 4 s, one with an undo 10 s. A 2 px navy line at the
  bottom drains over that time and stops while the toast is hovered (so does its timer),
  and every toast waits while the window is in the back or a modal dialog is open; under
  reduced motion there is no line. The stack lies below a dialog's scrim: dimmed, and its
  undo cannot act behind the dialog.
  The sentence has room for a job's title (520 px) and wraps to at most two lines: the
  title in the catalog's quotes („…“ or “…”) keeps to one line and ends in an ellipsis
  (the full title in a tooltip), the rest of the sentence follows it.
  The check of a success draws itself once as the toast appears. Closable; an undo of what
  the user just did sits before the close button. A merged toast ("2 Jobs archiviert.")
  cross-fades its sentence (100 ms) and starts its line again.
-->
<script lang="ts" module>
  /** A sentence around one quoted name: German „…“, English “…”. */
  const QUOTED = /^(.*?)([„“])([^“”]+)([“”])(.*)$/su;

  interface Quoted {
    before: string;
    open: string;
    name: string;
    close: string;
    after: string;
  }

  /** The quoted name of a toast's sentence (a job's title), or null. */
  export function quoted(text: string): Quoted | null {
    const match = QUOTED.exec(text);
    if (match === null) return null;
    const [, before = '', open = '', name = '', close = '', after = ''] = match;
    return { before, open, name, close, after };
  }
</script>

<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { t } from '$lib/i18n/t';
  import { onWindowFocus } from '$lib/ipc/api';
  import { fade, flip, toastIn, toastOut } from '$lib/motion/transitions';
  import { toasts } from '$lib/state/toasts.svelte';
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';

  let hovered = $state<number | null>(null);

  // The window in the back: every toast waits until it is in front again.
  $effect(() => {
    let release: (() => void) | null = null;
    const stop = onWindowFocus((focused) => {
      if (focused) {
        release?.();
        release = null;
      } else if (release === null) {
        release = toasts.hold();
      }
    });
    return () => {
      stop();
      release?.();
    };
  });
</script>

{#snippet sentence(text: string)}
  {@const split = quoted(text)}
  {#if split}{split.before}<span class="quoted"
      >{split.open}<span class="name" use:tooltip={{ text: split.name, truncated: true }}
        >{split.name}</span
      >{split.close}</span
    >{split.after}{:else}{text}{/if}
{/snippet}

<div class="stack" class:held={toasts.held} role="status" aria-live="polite" data-testid="toasts">
  {#each toasts.items as toast (toast.id)}
    <div
      class="toast {toast.tone}"
      class:paused={hovered === toast.id}
      class:lasting={toast.action !== null}
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
      {#key toast.text}<span class="text" data-testid="toast-text" in:fade
          >{@render sentence(toast.text)}</span
        >{/key}
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
      {#key toast.round}<span class="life" aria-hidden="true"></span>{/key}
    </div>
  {/each}
</div>

<style>
  /* Below the scrim of a dialog (--z-toast < --z-overlay): dimmed with the page. */
  .stack {
    position: fixed;
    right: var(--space-24);
    bottom: var(--space-24);
    z-index: var(--z-toast);
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

  /* One or two lines with room above and below (10 px to the edge with the padding). */
  .text {
    flex: 1;
    min-width: 0;
    padding-block: var(--space-4);
    text-wrap: pretty;
  }

  /* The quoted title keeps to one line; the rest of the sentence follows it. */
  .quoted {
    display: inline-flex;
    max-width: 100%;
    white-space: nowrap;
  }

  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* The lifetime line: it drains from the right over the toast's time, and stops while
     the toast is hovered or every toast waits. */
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

  .lasting .life {
    animation-duration: var(--dur-toast-action);
  }

  .paused .life,
  .held .life {
    animation-play-state: paused;
  }

  :global(:root[data-motion='reduce']) .life {
    display: none;
  }
</style>
