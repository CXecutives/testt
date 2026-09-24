<!--
  Label, control, hint (or the error in its place) and the hint's way on, which stays while
  an error shows: it is what helps most then. The way on is a navy link (it underlines on
  hover; one that leaves the app shows the hand) whose text lines up with the edges of the
  field, next to the hint or on a line of its own.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Button from './Button.svelte';
  import Icon, { type IconName } from './Icon.svelte';

  interface Props {
    label: string;
    /** id of the control inside, for the label. */
    for: string;
    hint?: string | null;
    /** A way on that belongs to the hint (opens a page), at the end of the helper line. */
    action?: { label: string; icon?: IconName; testid?: string; onclick: () => void } | null;
    error?: string | null;
    children: Snippet;
  }

  let { label, for: control, hint = null, action = null, error = null, children }: Props = $props();
</script>

<div class="field">
  <label class="label" for={control}>{label}</label>
  {@render children()}
  {#if error || hint || action}
    <div class="help">
      {#if error}
        <p class="error" id="{control}-message" role="alert">
          <Icon name="triangle-alert" size="sm" />
          <span>{error}</span>
        </p>
      {:else if hint}
        <p class="hint" id="{control}-message">{hint}</p>
      {/if}
      {#if action}
        <span class="action">
          <Button
            variant="link"
            size="sm"
            icon={action.icon ?? null}
            external={action.icon === 'external-link'}
            label={action.label}
            testid={action.testid ?? null}
            onclick={action.onclick}
          />
        </span>
      {/if}
    </div>
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
    font-weight: var(--weight-medium);
    cursor: default;
  }

  .hint,
  .error {
    font: var(--type-sm);
  }

  .hint {
    color: var(--text-muted);
  }

  .help {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4) var(--space-12);
    min-height: var(--control-sm);
  }

  .action {
    display: inline-flex;
  }

  /* The icon sits on the first line when a message wraps. */
  .error {
    display: flex;
    align-items: flex-start;
    gap: var(--space-6);
    color: var(--danger-strong);
  }

  .error > :global(:first-child) {
    flex: none;
    margin-top: calc((var(--leading-sm) - var(--icon-sm)) / 2);
  }
</style>
