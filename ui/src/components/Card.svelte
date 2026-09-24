<!--
  A white card with a hairline, flat (it sits on the white sheet of the content: no shadow
  at rest). plain | interactive | tinted.
  Interactive cards (with onclick) lift 1 px on hover: the hairline darkens and a warm
  shadow fades in on ::after. No colour: coral is kept for selection and the primary.
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
    position: relative;
    display: block;
    width: 100%;
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
    background-color: var(--surface);
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

  /* Card shadow, faded in (box-shadow itself never animates). */
  .interactive::after {
    position: absolute;
    z-index: var(--z-below);
    inset: calc(-1 * var(--border-width));
    border-radius: inherit;
    box-shadow: var(--sh-card);
    content: '';
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  .interactive:hover {
    border-color: var(--border-strong);
    transform: translateY(var(--lift));
  }

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
