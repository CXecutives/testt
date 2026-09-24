<!--
  Label, control, hint and error. A new error shakes the message once (4 px; no movement
  under reduced motion).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  interface Props {
    label: string;
    /** id of the control inside, for the label. */
    for: string;
    hint?: string | null;
    error?: string | null;
    children: Snippet;
  }

  let { label, for: control, hint = null, error = null, children }: Props = $props();
</script>

<div class="field">
  <label class="label" for={control}>{label}</label>
  {@render children()}
  {#if error}
    {#key error}
      <p class="error" id="{control}-message" role="alert">
        <Icon name="triangle-alert" size="sm" />
        <span>{error}</span>
      </p>
    {/key}
  {:else if hint}
    <p class="hint" id="{control}-message">{hint}</p>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    min-width: 0;
  }

  .label {
    color: var(--text);
    font: var(--type-sm);
    font-weight: var(--weight-semibold);
    cursor: default;
  }

  .hint,
  .error {
    font: var(--type-sm);
  }

  .hint {
    color: var(--text-muted);
  }

  .error {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    color: var(--danger-strong);
    animation: shake var(--dur-slow) var(--ease-standard) 1;
  }
</style>
