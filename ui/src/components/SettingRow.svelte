<!-- One setting: label and one-sentence hint on the left, badges and the control right. A
     hint that is a value to copy (a path) selects like text (`copy`), and so does a label
     that is such a value (`copyLabel`, the address of the mailbox).
     The row runs edge to edge in its container and pads its content by the container's
     --row-inset, so its divider and its text share the container's grid.
     With `for` (the id of its switch) the row works like a row of the system settings of
     Windows 11 and macOS: only the switch switches (user decision). The label names the
     switch and the hint describes it (`{for}-label`, and `{for}-hint` while there is a hint,
     read by Toggle), but neither is a click target, and the row never reacts to the pointer. -->
<script lang="ts">
  import { describe } from '$lib/state/described';
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    hint?: string | null;
    /** Badges next to the label (e.g. Verbunden). */
    badges?: Snippet | null;
    /** The hint is a value a user would copy (a folder path). */
    copy?: boolean;
    /** The label is a value a user would copy (the address of the mailbox). */
    copyLabel?: boolean;
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
    copyLabel = false,
    for: control = null,
    testid = null,
    children,
  }: Props = $props();

  describe(() => (control !== null && hint ? `${control}-hint` : null));
</script>

<div
  class="row"
  data-setting-row
  data-toggle-row={control !== null ? '' : undefined}
  data-testid={testid ?? undefined}
>
  <div class="text">
    <span class="title">
      <span
        class="label"
        class:path={copyLabel}
        id={control !== null ? `${control}-label` : undefined}
        data-copy={copyLabel ? '' : undefined}>{label}</span
      >
      {#if badges}{@render badges()}{/if}
    </span>
    {#if hint}<p
        class="hint"
        class:path={copy}
        id={control !== null ? `${control}-hint` : undefined}
        data-copy={copy ? '' : undefined}
      >
        {hint}
      </p>{/if}
  </div>
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
    margin-right: calc(-1 * var(--ghost-inset));
  }

  /* In a narrow container (the settings page at the minimum window) buttons go under the
     text, so a path or a hint keeps the whole width; a switch stays at the right. */
  @container (width < 520px) {
    .row:not([data-toggle-row]) {
      flex-wrap: wrap;
      row-gap: var(--space-8);
    }

    .row:not([data-toggle-row]) .text {
      flex-basis: 100%;
    }

    .row:not([data-toggle-row]) .control {
      margin-left: auto;
    }
  }
</style>
