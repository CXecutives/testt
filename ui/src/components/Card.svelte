<!--
  A white card with a hairline, flat (it sits on the white sheet of the content, no shadow).
  plain | interactive | tinted (a calm muted surface).
  Interactive cards (with onclick) answer like a stat tile: a navy hairline and a soft
  shadow that fades in on hover (no lift), a slight give under the pointer (0.985). Plain
  and tinted cards never react; only their controls do.
-->
<script lang="ts" module>
  export type CardVariant = 'plain' | 'interactive' | 'tinted';
  /** `rows`: for a card of SettingRows (they bring their own vertical padding). */
  export type CardPadding = 'none' | 'rows' | 'md' | 'lg';
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    variant?: CardVariant;
    padding?: CardPadding;
    /** Makes the card one big button (only for interactive cards). */
    onclick?: (() => void) | null;
    label?: string | null;
    testid?: string | null;
    children: Snippet;
  }

  let {
    variant = 'plain',
    padding = 'lg',
    onclick = null,
    label = null,
    testid = null,
    children,
  }: Props = $props();
</script>

{#if variant === 'interactive' && onclick}
  <button
    type="button"
    class="card interactive pad-{padding}"
    aria-label={label ?? undefined}
    data-testid={testid ?? undefined}
    onclick={() => onclick?.()}
  >
    {@render children()}
  </button>
{:else}
  <div class="card {variant} pad-{padding}" data-testid={testid ?? undefined}>
    {@render children()}
  </div>
{/if}

<style>
  .card {
    display: block;
    width: 100%;
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
    background-color: var(--surface);
    text-align: left;
  }

  .tinted {
    background-color: var(--surface-muted);
  }

  .interactive {
    position: relative;
    transition:
      border-color var(--dur-base) var(--ease-standard),
      transform var(--dur-base) var(--ease-emphasized);
  }

  /* The hover shadow, painted once and shown by opacity (no lift, no animated shadow). */
  .interactive::after {
    position: absolute;
    inset: calc(-1 * var(--border-width));
    border-radius: inherit;
    box-shadow: var(--sh-hover);
    content: '';
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  .interactive:hover {
    border-color: var(--border-navy);
    transition-duration: var(--dur-hover), var(--dur-base);
  }

  .interactive:hover::after {
    opacity: 1;
    transition-duration: var(--dur-hover);
  }

  .interactive:active {
    transform: scale(var(--scale-press-soft));
    transition-duration: var(--dur-instant);
  }

  .interactive:active::after {
    opacity: 0;
    transition-duration: var(--dur-instant);
  }

  .interactive:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .pad-none {
    padding: 0;
  }

  .pad-rows {
    padding: var(--space-4) var(--space-20);
  }

  .pad-md {
    padding: var(--space-16) var(--space-20);
  }

  .pad-lg {
    padding: var(--space-24);
  }
</style>
