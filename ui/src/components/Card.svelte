<!--
  White card on the cream background. plain | interactive | tinted.
  Interactive cards (with onclick) lift 2 px on hover; the card shadow fades in on ::after,
  a coral hairline border and a gradient top edge appear.
-->
<script lang="ts" module>
  export type CardVariant = 'plain' | 'interactive' | 'tinted';
  export type CardPadding = 'none' | 'md' | 'lg';
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
    position: relative;
    display: block;
    width: 100%;
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
    background-color: var(--surface);
    box-shadow: var(--sh-sm);
    text-align: left;
    isolation: isolate;
  }

  .tinted {
    border-color: var(--border-accent);
    background: var(--grad-card);
  }

  .interactive {
    transition:
      transform var(--dur-base) var(--ease-out),
      border-color var(--dur-base) var(--ease-standard);
  }

  .interactive::before,
  .interactive::after {
    position: absolute;
    border-radius: inherit;
    content: '';
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  /* Card shadow, faded in (box-shadow itself never animates). */
  .interactive::after {
    z-index: var(--z-below);
    inset: calc(-1 * var(--border-width));
    box-shadow: var(--sh-card), var(--sh-elegant);
  }

  /* Gradient top edge. */
  .interactive::before {
    z-index: var(--z-raised);
    top: calc(-1 * var(--border-width));
    right: var(--space-12);
    left: var(--space-12);
    height: var(--marker-height);
    border-radius: var(--radius-full);
    background: var(--grad-edge);
  }

  .interactive:hover {
    border-color: var(--border-accent);
    transform: translateY(var(--lift-card));
  }

  .interactive:hover::before,
  .interactive:hover::after {
    opacity: 1;
  }

  .interactive:active {
    transform: translateY(0) scale(var(--scale-press));
    transition-duration: var(--dur-instant);
  }

  .interactive:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .pad-none {
    padding: 0;
  }

  .pad-md {
    padding: var(--space-16);
  }

  .pad-lg {
    padding: var(--space-24);
  }
</style>
