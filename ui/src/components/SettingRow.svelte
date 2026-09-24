<!-- One setting: label and one-sentence hint on the left, badges and the control right. -->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    hint?: string | null;
    /** Badges next to the label (e.g. the risk of a portal switch). */
    badges?: Snippet | null;
    testid?: string | null;
    children: Snippet;
  }

  let { label, hint = null, badges = null, testid = null, children }: Props = $props();
</script>

<div class="row" data-testid={testid ?? undefined}>
  <div class="text">
    <div class="title">
      <span class="label">{label}</span>
      {#if badges}{@render badges()}{/if}
    </div>
    {#if hint}<p class="hint">{hint}</p>{/if}
  </div>
  <div class="control">{@render children()}</div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-24);
    padding: var(--space-16) 0;
    border-bottom: var(--border-width) solid var(--border);
  }

  .row:last-child {
    border-bottom: 0;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8);
  }

  .label {
    color: var(--text);
    font: var(--type-md);
    font-weight: var(--weight-medium);
  }

  .hint {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .control {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-12);
  }
</style>
