<!-- One setting: label and one-sentence hint on the left, badges and the control right. A
     hint that is a value to copy (a path, the address) selects like text (`copy`).
     The row runs edge to edge in its container and pads its content by the container's
     --row-inset, so its divider and its text share the container's grid.
     With `for` (the id of its switch) the row works like a row of the system settings of
     Windows 11 and macOS: its label and hint are a native <label>, so a click on the text
     toggles the switch, and the switch shows its hover while the pointer is anywhere on
     the row. The row itself never gets a background (user decision: only the switch
     reacts). A copyable hint stays outside the label and never toggles. -->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    hint?: string | null;
    /** Badges next to the label (e.g. the risk of a portal switch). */
    badges?: Snippet | null;
    /** The hint is a value a user would copy (a folder path). */
    copy?: boolean;
    /** The id of the switch this row labels (Toggle `id`). */
    for?: string | null;
    testid?: string | null;
    children: Snippet;
  }

  let {
    label,
    hint = null,
    badges = null,
    copy = false,
    for: control = null,
    testid = null,
    children,
  }: Props = $props();
</script>

{#snippet title()}
  <span class="title">
    <span class="label">{label}</span>
    {#if badges}{@render badges()}{/if}
  </span>
{/snippet}

<div
  class="row"
  data-setting-row
  data-toggle-row={control !== null ? '' : undefined}
  data-testid={testid ?? undefined}
>
  {#if control !== null}
    <div class="text">
      <label class="for" for={control}>
        {@render title()}
        {#if hint && !copy}<span class="hint">{hint}</span>{/if}
      </label>
      {#if hint && copy}<p class="hint path" data-copy>{hint}</p>{/if}
    </div>
  {:else}
    <div class="text">
      {@render title()}
      {#if hint}<p class="hint" class:path={copy} data-copy={copy ? '' : undefined}>
          {hint}
        </p>{/if}
    </div>
  {/if}
  <div class="control">{@render children()}</div>
</div>

<style>
  .row {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-24);
    min-height: calc(var(--control-md) + 2 * var(--space-12));
    margin-inline: calc(-1 * var(--row-inset));
    padding: var(--space-12) var(--row-inset);
    border-bottom: var(--border-width) solid var(--border);
    isolation: isolate;
  }

  .row:last-child {
    border-bottom: 0;
  }

  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .for {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
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

  .path {
    overflow-wrap: anywhere;
  }

  .control {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-12);
  }

  /* A ghost button at the end lines its text up with the edge, like toggles and bordered
     buttons (the ghost's own padding would inset it). */
  .control :global(.btn.ghost.sm:last-child) {
    margin-right: calc(-1 * var(--space-12));
  }
</style>
