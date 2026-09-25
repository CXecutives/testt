<!--
  On/off switch, coral when on (user decision). The thumb travels in 180 ms (emphasized, no
  bounce) and the track changes colour in 100 ms; while the left button is down the track
  darkens a step (the thumb never changes shape). Disabled switches stay hoverable so the
  tooltip can say why (disabledReason).
  It flips at once, like a native switch: when `onchange` returns a promise (the save),
  the switch shows the new state until it settles, then `checked` again - which is the old
  state if the save failed, so the thumb slides back.
  `id` lets a SettingRow label it: a click on the row's text then toggles it natively, and
  the switch shows its hover while the pointer is anywhere on that row (the row itself never
  changes).
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';

  interface Props {
    checked: boolean;
    label: string;
    /** Show the label next to the switch (otherwise it is only the accessible name). */
    showLabel?: boolean;
    disabled?: boolean;
    disabledReason?: string | null;
    /** For a <label for> around the text of its row (SettingRow `for`). */
    id?: string | null;
    testid?: string | null;
    /** Return the save's promise: the switch shows the new state until it settles. */
    onchange: (checked: boolean) => unknown;
  }

  let {
    checked,
    label,
    showLabel = false,
    disabled = false,
    disabledReason = null,
    id = null,
    testid = null,
    onchange,
  }: Props = $props();

  /** The state shown while a save is on its way (null: show `checked`). */
  let pending = $state<boolean | null>(null);
  let attempt = 0;
  const shown = $derived(pending ?? checked);

  async function toggle(): Promise<void> {
    if (disabled) return;
    const next = !shown;
    const mine = ++attempt;
    pending = next;
    // Settled either way: the caller reports a failure, the switch shows `checked` again.
    const settle = (): void => {
      if (mine === attempt) pending = null;
    };
    await Promise.resolve(onchange(next)).then(settle, settle);
  }
</script>

<button
  type="button"
  role="switch"
  class="toggle"
  id={id ?? undefined}
  aria-checked={shown}
  aria-label={showLabel ? undefined : label}
  aria-disabled={disabled ? 'true' : undefined}
  data-testid={testid ?? undefined}
  use:tooltip={disabled ? disabledReason : null}
  onclick={() => void toggle()}
>
  <span class="track"><span class="thumb"></span></span>
  {#if showLabel}<span class="label">{label}</span>{/if}
</button>

<style>
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--space-12);
    color: var(--text);
    font: var(--type-md);
  }

  .track {
    position: relative;
    display: inline-block;
    flex: none;
    width: var(--toggle-width);
    height: var(--toggle-height);
    border-radius: var(--radius-full);
    background-color: var(--border-strong);
    transition: background-color var(--dur-fast) var(--ease-standard);
  }

  .thumb {
    position: absolute;
    top: calc((var(--toggle-height) - var(--toggle-thumb)) / 2);
    left: calc((var(--toggle-height) - var(--toggle-thumb)) / 2);
    width: var(--toggle-thumb);
    height: var(--toggle-thumb);
    border-radius: var(--radius-full);
    background-color: var(--surface);
    box-shadow: var(--sh-thumb);
    transition: transform var(--dur-slow) var(--ease-emphasized);
  }

  .toggle:not([aria-disabled='true']):hover .track,
  :global([data-toggle-row]:hover) .toggle:not([aria-disabled='true']) .track {
    background-color: var(--border-input);
    transition-duration: var(--dur-hover);
  }

  .toggle[aria-checked='true'] .track {
    background-color: var(--toggle-on);
  }

  .toggle[aria-checked='true']:not([aria-disabled='true']):hover .track,
  :global([data-toggle-row]:hover) .toggle[aria-checked='true']:not([aria-disabled='true']) .track {
    background-color: var(--toggle-on-hover);
  }

  .toggle[aria-checked='true'] .thumb {
    transform: translateX(var(--toggle-travel));
  }

  /* Pressed: the track darkens a step, 60 ms. */
  :global(:where(:root:not([data-aux-press])))
    .toggle:not([aria-disabled='true']):active:hover
    .track {
    background-color: var(--border-input);
    transition-duration: var(--dur-instant);
  }

  :global(:where(:root:not([data-aux-press])))
    .toggle[aria-checked='true']:not([aria-disabled='true']):active:hover
    .track {
    background-color: var(--primary-active);
  }

  .toggle:focus-visible .track {
    box-shadow: var(--focus-ring);
  }

  .toggle[aria-disabled='true'] {
    cursor: default;
    opacity: var(--opacity-disabled);
  }
</style>
