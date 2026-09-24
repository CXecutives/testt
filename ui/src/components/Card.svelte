<!--
  A white card with a hairline, flat (it sits on the white sheet of the content, no shadow).
  plain | interactive | tinted (a calm muted surface).
  Interactive cards (with onclick) only darken their hairline on hover: no lift, no shadow.
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
    transition: border-color var(--dur-fast) var(--ease-standard);
  }

  .interactive:hover {
    border-color: var(--border-strong);
  }

  .interactive:active {
    border-color: var(--border-input);
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
